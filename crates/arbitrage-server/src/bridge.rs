//! # Arbitrage Bridge
//!
//! Connects exchange data feeds to the arbitrage engine.
//! Handles WebSocket connections, market data processing, and signal detection.

use crate::audit_logger::{AuditDecision, AuditEntry, AuditLogger};
use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    config::Config,
    strategies::{FundingRate, StrategyRegistry, Ticker},
    symbol_discovery::{MarketInfo, OrderBookDepth, SymbolSelectionCriteria},
    symbol_manager::{SymbolManager, SymbolManagerConfig},
    types::Symbol,
    Result,
};
use exchange_connectors::{
    events::{ConnectionEvent, MarketDataEvent},
    ExchangeManager, ExchangeManagerConfig,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Bridge service statistics
#[derive(Debug, Clone, Default)]
pub struct BridgeStats {
    pub connected_exchanges: usize,
    pub total_exchanges: usize,
    pub total_messages_received: u64,
    pub total_errors: u64,
    pub active_symbols: usize,
    pub cached_signals: usize,
    pub order_books_cached: usize,
}

/// Bridge service connecting exchanges to arbitrage engine
pub struct ArbitrageBridge {
    exchange_manager: Arc<Mutex<ExchangeManager>>,
    arbitrage_engine: Arc<ArbitrageEngine>,
    strategy_registry: Arc<StrategyRegistry>,
    symbol_manager: Arc<Mutex<SymbolManager>>,
    audit_logger: Arc<AuditLogger>,
    config: Config,
}

impl ArbitrageBridge {
    /// Create new bridge service
    pub async fn new(
        config: Config,
        arbitrage_engine: Arc<ArbitrageEngine>,
        strategy_registry: Arc<StrategyRegistry>,
        audit_logger: Arc<AuditLogger>,
    ) -> Result<Self> {
        let exchange_config = ExchangeManagerConfig {
            enabled_exchanges: config.exchanges.keys().cloned().collect(),
            health_check_interval_seconds: 30,
            max_reconnect_attempts: 5,
            event_buffer_size: 10000,
            rate_limits: Default::default(),
            auto_reconnect: true,
            max_parallel_connections: 10,
        };

        let exchange_manager = Arc::new(Mutex::new(ExchangeManager::new(exchange_config)));
        exchange_manager.lock().await.initialize().await?;

        // Initialize symbol manager with config symbols as core
        let core_symbols = Self::collect_config_symbols(&config);
        let symbol_manager_config = SymbolManagerConfig {
            core_symbols,
            enable_discovery: true,
            discovery_refresh_interval: 3600, // 1 hour
            discovery_criteria: SymbolSelectionCriteria::default(),
        };
        let symbol_manager = Arc::new(Mutex::new(SymbolManager::new(symbol_manager_config)));

        info!(
            "📋 Symbol Manager initialized with {} core symbols",
            symbol_manager.lock().await.get_active_symbols().len()
        );

        Ok(Self {
            exchange_manager,
            arbitrage_engine,
            strategy_registry,
            symbol_manager,
            audit_logger,
            config,
        })
    }

    /// Collect all symbols from config
    fn collect_config_symbols(config: &Config) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        for exchange_config in config.exchanges.values() {
            for symbol in &exchange_config.symbols {
                if !symbols.contains(symbol) {
                    symbols.push(symbol.clone());
                }
            }
        }
        if symbols.is_empty() {
            symbols = vec![
                Symbol::new("BTC", "USDT"),
                Symbol::new("ETH", "USDT"),
                Symbol::new("SOL", "USDT"),
            ];
        }
        symbols
    }

    /// Start the bridge service
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting Bridge Service");

        // Connect to exchanges
        self.exchange_manager.lock().await.connect_all().await?;

        // Get symbols to subscribe from symbol manager
        let symbols = self.symbol_manager.lock().await.get_active_symbols();
        info!("📊 Subscribing to {} symbols", symbols.len());

        // Subscribe to market data
        self.exchange_manager
            .lock()
            .await
            .subscribe_symbols(&symbols)
            .await?;

        // Start processing loops
        self.start_market_data_processor().await;
        self.start_health_monitor().await;
        self.start_symbol_discovery_refresh().await;

        info!("✅ Bridge Service started");
        Ok(())
    }

    /// Periodically refresh discovered symbols
    async fn start_symbol_discovery_refresh(&self) {
        let symbol_manager = Arc::clone(&self.symbol_manager);
        let exchange_manager = Arc::clone(&self.exchange_manager);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // 1 hour

            loop {
                interval.tick().await;

                info!("🔍 Refreshing symbol discovery...");

                match symbol_manager
                    .lock()
                    .await
                    .refresh_discovered_symbols()
                    .await
                {
                    Ok(_) => {
                        let active_symbols = symbol_manager.lock().await.get_active_symbols();
                        info!("📊 Now monitoring {} symbols", active_symbols.len());

                        // Resubscribe to updated symbol list
                        if let Err(e) = exchange_manager
                            .lock()
                            .await
                            .subscribe_symbols(&active_symbols)
                            .await
                        {
                            warn!("Failed to resubscribe to symbols: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Symbol discovery refresh failed: {}", e);
                    }
                }
            }
        });
    }

    /// Process incoming market data
    async fn start_market_data_processor(&mut self) {
        let exchange_manager = Arc::clone(&self.exchange_manager);
        let arbitrage_engine = Arc::clone(&self.arbitrage_engine);
        let strategy_registry = Arc::clone(&self.strategy_registry);
        let symbol_manager = Arc::clone(&self.symbol_manager);
        let audit_logger = Arc::clone(&self.audit_logger);

        tokio::spawn(async move {
            info!("📡 Market data processor started");

            let mut event_receiver = exchange_manager.lock().await.get_event_receiver();

            while let Ok(event) = event_receiver.recv().await {
                if let Err(e) = Self::handle_event(
                    &event,
                    &arbitrage_engine,
                    &strategy_registry,
                    &symbol_manager,
                    &audit_logger,
                )
                .await
                {
                    warn!("Event handling error: {}", e);
                }
            }
        });
    }

    /// Handle a single connection event
    async fn handle_event(
        event: &ConnectionEvent,
        engine: &ArbitrageEngine,
        registry: &StrategyRegistry,
        symbol_manager: &Arc<Mutex<SymbolManager>>,
        audit_logger: &Arc<AuditLogger>,
    ) -> Result<()> {
        match event {
            ConnectionEvent::MarketData(market_event) => {
                Self::handle_market_data(
                    market_event,
                    engine,
                    registry,
                    symbol_manager,
                    audit_logger,
                )
                .await
            }
            ConnectionEvent::StatusChange {
                exchange,
                new_status,
                ..
            } => {
                info!("🔄 {} status: {:?}", exchange, new_status);
                Ok(())
            }
            ConnectionEvent::Error {
                exchange, error, ..
            } => {
                error!("❌ {} error: {}", exchange, error);

                // Log error to audit
                let audit_entry = AuditEntry::new(
                    AuditDecision::ErrorOccurred,
                    "error",
                    &format!("Exchange {} error: {}", exchange, error),
                );
                audit_logger.log(audit_entry).await;

                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Handle market data events
    async fn handle_market_data(
        event: &MarketDataEvent,
        engine: &ArbitrageEngine,
        registry: &StrategyRegistry,
        symbol_manager: &Arc<Mutex<SymbolManager>>,
        _audit_logger: &Arc<AuditLogger>,
    ) -> Result<()> {
        match event {
            MarketDataEvent::OrderBook { order_book, .. } => {
                debug!(
                    "📖 Order book: {} on {}",
                    order_book.symbol, order_book.exchange
                );
                engine.update_order_book(order_book.clone()).await?;

                // Update symbol manager with market info for discovery
                let market_info = MarketInfo {
                    symbol: order_book.symbol.clone(),
                    exchange: order_book.exchange,
                    volume_24h_usd: Decimal::ZERO, // Would need ticker data
                    price_usd: order_book.mid_price().unwrap_or(Decimal::ZERO),
                    spread_bps: {
                        let best_bid = order_book
                            .best_bid()
                            .map(|l| l.price)
                            .unwrap_or(Decimal::ZERO);
                        let best_ask = order_book
                            .best_ask()
                            .map(|l| l.price)
                            .unwrap_or(Decimal::ZERO);
                        let mid = order_book.mid_price().unwrap_or(Decimal::ONE);
                        ((best_ask - best_bid) / mid * Decimal::from(10000))
                            .to_i64()
                            .unwrap_or(0) as u32
                    },
                    is_active: true,
                    timestamp: order_book.timestamp,
                    depth_analysis: OrderBookDepth {
                        level_1_volume_usd: {
                            let bid_size = order_book
                                .best_bid()
                                .map(|l| l.quantity)
                                .unwrap_or(Decimal::ZERO);
                            let ask_size = order_book
                                .best_ask()
                                .map(|l| l.quantity)
                                .unwrap_or(Decimal::ZERO);
                            (bid_size + ask_size) * order_book.mid_price().unwrap_or(Decimal::ZERO)
                        },
                        depth_01_percent_usd: Decimal::ZERO, // Would need full book
                        depth_05_percent_usd: Decimal::ZERO,
                        max_order_size_usd: {
                            let bid_size = order_book
                                .best_bid()
                                .map(|l| l.quantity)
                                .unwrap_or(Decimal::ZERO);
                            let ask_size = order_book
                                .best_ask()
                                .map(|l| l.quantity)
                                .unwrap_or(Decimal::ZERO);
                            bid_size.max(ask_size) * order_book.mid_price().unwrap_or(Decimal::ZERO)
                        },
                    },
                };

                if let Err(e) = symbol_manager
                    .lock()
                    .await
                    .update_market_data(market_info)
                    .await
                {
                    debug!("Symbol manager update error: {}", e);
                }

                // Run detection after order book update
                if let Err(e) = engine.detect_opportunities(registry).await {
                    debug!("Detection error: {}", e);
                }
            }

            MarketDataEvent::Ticker { ticker, .. } => {
                debug!("📊 Ticker: {} on {}", ticker.symbol, ticker.exchange);

                let engine_ticker = Ticker {
                    symbol: ticker.symbol.clone(),
                    exchange: ticker.exchange,
                    last: ticker.last_price,
                    bid: ticker.bid_price,
                    ask: ticker.ask_price,
                    volume_24h: ticker.volume_24h,
                    change_24h: Decimal::ZERO,
                    timestamp: ticker.timestamp,
                };
                engine.update_ticker(engine_ticker).await?;

                // Update symbol manager with ticker data
                let market_info = MarketInfo {
                    symbol: ticker.symbol.clone(),
                    exchange: ticker.exchange,
                    volume_24h_usd: ticker.volume_24h * ticker.last_price,
                    price_usd: ticker.last_price,
                    spread_bps: ((ticker.ask_price - ticker.bid_price) / ticker.last_price
                        * Decimal::from(10000))
                    .to_i64()
                    .unwrap_or(0) as u32,
                    is_active: true,
                    timestamp: ticker.timestamp,
                    depth_analysis: OrderBookDepth {
                        level_1_volume_usd: Decimal::ZERO,
                        depth_01_percent_usd: Decimal::ZERO,
                        depth_05_percent_usd: Decimal::ZERO,
                        max_order_size_usd: Decimal::ZERO,
                    },
                };

                if let Err(e) = symbol_manager
                    .lock()
                    .await
                    .update_market_data(market_info)
                    .await
                {
                    debug!("Symbol manager update error: {}", e);
                }
            }

            MarketDataEvent::FundingRate { funding_rate, .. } => {
                debug!(
                    "💸 Funding rate: {} on {}",
                    funding_rate.symbol, funding_rate.exchange
                );

                let engine_funding = FundingRate {
                    symbol: funding_rate.symbol.clone(),
                    exchange: funding_rate.exchange,
                    rate: funding_rate.funding_rate,
                    predicted_rate: funding_rate.predicted_rate,
                    next_funding: funding_rate.funding_time,
                    timestamp: funding_rate.timestamp,
                };
                engine.update_funding_rate(engine_funding).await?;
            }

            MarketDataEvent::Raw { .. } => {
                debug!("📦 Raw market data received");
            }

            _ => {}
        }

        Ok(())
    }

    /// Monitor exchange health
    async fn start_health_monitor(&self) {
        let exchange_manager = Arc::clone(&self.exchange_manager);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                let health = exchange_manager.lock().await.get_health_status().await;
                let connected = health.values().filter(|h| h.is_connected).count();

                info!(
                    "💓 Health: {}/{} exchanges connected",
                    connected,
                    health.len()
                );

                if connected == 0 {
                    error!("🚨 No exchanges connected!");
                }
            }
        });
    }

    /// Get bridge statistics
    pub async fn get_stats(&self) -> BridgeStats {
        let exchange_stats = self.exchange_manager.lock().await.get_stats().await;
        let health = self.exchange_manager.lock().await.get_health_status().await;
        let engine_stats = self.arbitrage_engine.get_stats();
        let active_symbols_count = self.symbol_manager.lock().await.get_active_symbols().len();

        BridgeStats {
            connected_exchanges: health.values().filter(|h| h.is_connected).count(),
            total_exchanges: health.len(),
            total_messages_received: exchange_stats.values().map(|s| s.messages_received).sum(),
            total_errors: exchange_stats.values().map(|s| s.errors_count).sum(),
            active_symbols: active_symbols_count,
            cached_signals: engine_stats.cached_signals_count,
            order_books_cached: engine_stats.order_books_count,
        }
    }

    /// Force reconnect all exchanges
    pub async fn force_reconnect_all(&mut self) -> Result<()> {
        info!("🔄 Force reconnecting all exchanges");

        for exchange in self.config.exchanges.keys() {
            if let Err(e) = self
                .exchange_manager
                .lock()
                .await
                .force_reconnect(*exchange)
                .await
            {
                warn!("Reconnect failed for {}: {}", exchange, e);
            }
        }

        Ok(())
    }
}
