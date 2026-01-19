//! # Arbitrage Bridge
//!
//! Connects exchange data feeds to the arbitrage engine.
//! Handles WebSocket connections, market data processing, and signal detection.

use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    config::Config,
    strategies::{FundingRate, StrategyRegistry, Ticker},
    types::Symbol,
    Result,
};
use exchange_connectors::{
    events::{ConnectionEvent, MarketDataEvent},
    ExchangeManager, ExchangeManagerConfig,
};
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
    config: Config,
}

impl ArbitrageBridge {
    /// Create new bridge service
    pub async fn new(
        config: Config,
        arbitrage_engine: Arc<ArbitrageEngine>,
        strategy_registry: Arc<StrategyRegistry>,
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

        Ok(Self {
            exchange_manager,
            arbitrage_engine,
            strategy_registry,
            config,
        })
    }

    /// Start the bridge service
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting Bridge Service");

        // Connect to exchanges
        self.exchange_manager.lock().await.connect_all().await?;

        // Get symbols to subscribe
        let symbols = self.get_trading_symbols();
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

        info!("✅ Bridge Service started");
        Ok(())
    }

    /// Get trading symbols from config or defaults
    fn get_trading_symbols(&self) -> Vec<Symbol> {
        let mut symbols = Vec::new();

        for exchange_config in self.config.exchanges.values() {
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

    /// Process incoming market data
    async fn start_market_data_processor(&mut self) {
        let exchange_manager = Arc::clone(&self.exchange_manager);
        let arbitrage_engine = Arc::clone(&self.arbitrage_engine);
        let strategy_registry = Arc::clone(&self.strategy_registry);

        tokio::spawn(async move {
            info!("📡 Market data processor started");

            let mut event_receiver = exchange_manager.lock().await.get_event_receiver();

            while let Ok(event) = event_receiver.recv().await {
                if let Err(e) =
                    Self::handle_event(&event, &arbitrage_engine, &strategy_registry).await
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
    ) -> Result<()> {
        match event {
            ConnectionEvent::MarketData(market_event) => {
                Self::handle_market_data(market_event, engine, registry).await
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
    ) -> Result<()> {
        match event {
            MarketDataEvent::OrderBook { order_book, .. } => {
                debug!(
                    "📖 Order book: {} on {}",
                    order_book.symbol, order_book.exchange
                );
                engine.update_order_book(order_book.clone()).await?;

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

        BridgeStats {
            connected_exchanges: health.values().filter(|h| h.is_connected).count(),
            total_exchanges: health.len(),
            total_messages_received: exchange_stats.values().map(|s| s.messages_received).sum(),
            total_errors: exchange_stats.values().map(|s| s.errors_count).sum(),
            active_symbols: engine_stats.active_symbols_count,
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
