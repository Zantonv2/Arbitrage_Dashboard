use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    strategies::StrategyRegistry,
    config::Config,
    types::{Symbol},
    Result,
};
use exchange_connectors::{
    ExchangeManager, ExchangeManagerConfig,
    events::{ConnectionEvent, MarketDataEvent},
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn, error, debug};

/// Bridge service that connects exchange data to arbitrage engine
pub struct ArbitrageBridge {
    exchange_manager: Arc<Mutex<ExchangeManager>>,
    arbitrage_engine: Arc<ArbitrageEngine>,
    strategy_registry: Arc<StrategyRegistry>,
    config: Config,
    signal_receiver: broadcast::Receiver<arbitrage_core::types::Signal>,
}

impl ArbitrageBridge {
    /// Create new arbitrage bridge
    pub async fn new(
        config: Config,
        arbitrage_engine: Arc<ArbitrageEngine>,
        strategy_registry: Arc<StrategyRegistry>,
    ) -> Result<Self> {
        // Create exchange manager configuration
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
        
        let signal_receiver = arbitrage_engine.subscribe();
        
        Ok(Self {
            exchange_manager,
            arbitrage_engine,
            strategy_registry,
            config,
            signal_receiver,
        })
    }
    
    /// Start the bridge service
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting Arbitrage Bridge Service");
        
        // Connect to all exchanges
        self.exchange_manager.lock().await.connect_all().await?;
        
        // Get symbols from configuration
        let symbols = self.get_trading_symbols();
        info!("📊 Subscribing to {} symbols across all exchanges", symbols.len());
        
        // Subscribe to symbols on all exchanges
        self.exchange_manager.lock().await.subscribe_symbols(&symbols).await?;
        
        // Start market data processing loop
        self.start_market_data_processor().await;
        
        // Start signal monitoring
        self.start_signal_monitor().await;
        
        // Start health monitoring
        self.start_health_monitor().await;
        
        info!("✅ Arbitrage Bridge Service started successfully");
        Ok(())
    }
    
    /// Get trading symbols from configuration
    fn get_trading_symbols(&self) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        
        // Add symbols from exchange configurations
        for exchange_config in self.config.exchanges.values() {
            for symbol in &exchange_config.symbols {
                if !symbols.contains(symbol) {
                    symbols.push(symbol.clone());
                }
            }
        }
        
        // Add default symbols if none configured
        if symbols.is_empty() {
            symbols = vec![
                Symbol::new("BTC", "USDT"),
                Symbol::new("ETH", "USDT"),
                Symbol::new("SOL", "USDT"),
                Symbol::new("BNB", "USDT"),
                Symbol::new("XRP", "USDT"),
            ];
        }
        
        symbols
    }
    
    /// Start market data processing background task
    async fn start_market_data_processor(&mut self) {
        let exchange_manager = Arc::clone(&self.exchange_manager);
        let arbitrage_engine = Arc::clone(&self.arbitrage_engine);
        let strategy_registry = Arc::clone(&self.strategy_registry);
        
        tokio::spawn(async move {
            info!("📡 Market data processor started");
            
            let mut event_receiver = exchange_manager.lock().await.get_event_receiver();
            
            while let Ok(event) = event_receiver.recv().await {
                match event {
                    ConnectionEvent::MarketData(market_event) => {
                        match market_event {
                            MarketDataEvent::OrderBook { order_book, .. } => {
                                debug!("📖 Received order book for {} on {}", 
                                       order_book.symbol, order_book.exchange);
                                
                                // Update arbitrage engine with new order book
                                if let Err(e) = arbitrage_engine.update_order_book(order_book).await {
                                    warn!("Failed to update order book: {}", e);
                                    continue;
                                }
                                
                                // Run strategy detection
                                match arbitrage_engine.detect_opportunities(&strategy_registry).await {
                                    Ok(signals) => {
                                        if !signals.is_empty() {
                                            info!("🎯 Detected {} arbitrage opportunities", signals.len());
                                            for signal in &signals {
                                                info!("💰 {} arbitrage: {:.2}% profit on {} ({} -> {})",
                                                      "CEX", // TODO: Get strategy name from signal
                                                      signal.net_profit_percent * rust_decimal::Decimal::from(100),
                                                      signal.symbol,
                                                      signal.buy_exchange,
                                                      signal.sell_exchange);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Strategy detection failed: {}", e);
                                    }
                                }
                            }
                            
                            MarketDataEvent::Ticker { ticker, .. } => {
                                debug!("📊 Received ticker for {} on {}", 
                                       ticker.symbol, ticker.exchange);
                                
                                // Convert TickerData to Ticker for arbitrage engine
                                let engine_ticker = arbitrage_core::strategies::Ticker {
                                    symbol: ticker.symbol.clone(),
                                    exchange: ticker.exchange,
                                    last: ticker.last_price,
                                    bid: ticker.bid_price,
                                    ask: ticker.ask_price,
                                    volume_24h: ticker.volume_24h,
                                    change_24h: Decimal::ZERO, // Default value since not provided
                                    timestamp: ticker.timestamp,
                                };
                                
                                // Update arbitrage engine with ticker
                                if let Err(e) = arbitrage_engine.update_ticker(engine_ticker).await {
                                    warn!("Failed to update ticker: {}", e);
                                }
                            }
                            
                            MarketDataEvent::FundingRate { funding_rate, .. } => {
                                debug!("💸 Received funding rate for {} on {}", 
                                       funding_rate.symbol, funding_rate.exchange);
                                
                                // Convert connector FundingRate to strategies FundingRate
                                let engine_funding_rate = arbitrage_core::strategies::FundingRate {
                                    symbol: funding_rate.symbol.clone(),
                                    exchange: funding_rate.exchange,
                                    rate: funding_rate.funding_rate,
                                    predicted_rate: funding_rate.predicted_rate,
                                    next_funding: funding_rate.funding_time,
                                    timestamp: funding_rate.timestamp,
                                };
                                
                                // Update arbitrage engine with funding rate
                                if let Err(e) = arbitrage_engine.update_funding_rate(engine_funding_rate).await {
                                    warn!("Failed to update funding rate: {}", e);
                                }
                            }
                            
                            _ => {
                                // Handle other market data events
                                debug!("📈 Received other market data event");
                            }
                        }
                    }
                    
                    ConnectionEvent::StatusChange { exchange, new_status, .. } => {
                        info!("🔄 Exchange {} status changed to {:?}", exchange, new_status);
                    }
                    
                    ConnectionEvent::Error { exchange, error, .. } => {
                        error!("❌ Exchange {} error: {}", exchange, error);
                    }
                    
                    _ => {
                        debug!("📡 Received other connection event");
                    }
                }
            }
        });
    }
    
    /// Start signal monitoring background task
    async fn start_signal_monitor(&mut self) {
        let mut signal_receiver = self.signal_receiver.resubscribe();
        
        tokio::spawn(async move {
            info!("🎯 Signal monitor started");
            
            while let Ok(signal) = signal_receiver.recv().await {
                info!("🚨 ARBITRAGE SIGNAL DETECTED!");
                info!("   Symbol: {}", signal.symbol);
                info!("   Buy:  {} @ {}", signal.buy_exchange, signal.buy_price);
                info!("   Sell: {} @ {}", signal.sell_exchange, signal.sell_price);
                info!("   Profit: {:.4}%", signal.net_profit_percent * rust_decimal::Decimal::from(100));
                info!("   Signal ID: {}", signal.id);
                
                // TODO: Send to execution engine
                // TODO: Send desktop notification
                // TODO: Log to audit trail
            }
        });
    }
    
    /// Start health monitoring background task
    async fn start_health_monitor(&self) {
        let exchange_manager = Arc::clone(&self.exchange_manager);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // Get health status of all exchanges
                let health_status = exchange_manager.lock().await.get_health_status().await;
                let stats = exchange_manager.lock().await.get_stats().await;
                
                let mut connected_count = 0;
                let mut total_count = 0;
                
                for (exchange, health) in &health_status {
                    total_count += 1;
                    if health.is_connected {
                        connected_count += 1;
                    }
                    
                    if let Some(exchange_stats) = stats.get(exchange) {
                        debug!("📊 {} - Connected: {}, Messages: {}, Errors: {}", 
                               exchange, 
                               health.is_connected,
                               exchange_stats.messages_received,
                               exchange_stats.errors_count);
                    }
                }
                
                info!("💓 Health Check: {}/{} exchanges connected", connected_count, total_count);
                
                if connected_count == 0 {
                    error!("🚨 NO EXCHANGES CONNECTED - Trading halted!");
                } else if connected_count < total_count {
                    warn!("⚠️  Some exchanges disconnected - Reduced opportunities");
                }
            }
        });
    }
    
    /// Get current bridge statistics
    pub async fn get_stats(&self) -> BridgeStats {
        let exchange_stats = self.exchange_manager.lock().await.get_stats().await;
        let health_status = self.exchange_manager.lock().await.get_health_status().await;
        let engine_stats = self.arbitrage_engine.get_stats();
        
        let connected_exchanges = health_status.values()
            .filter(|h| h.is_connected)
            .count();
            
        let total_messages = exchange_stats.values()
            .map(|s| s.messages_received)
            .sum();
            
        let total_errors = exchange_stats.values()
            .map(|s| s.errors_count)
            .sum();
        
        BridgeStats {
            connected_exchanges,
            total_exchanges: health_status.len(),
            total_messages_received: total_messages,
            total_errors,
            active_symbols: engine_stats.active_symbols_count,
            cached_signals: engine_stats.cached_signals_count,
            order_books_cached: engine_stats.order_books_count,
        }
    }
    
    /// Force reconnect to all exchanges
    pub async fn force_reconnect_all(&mut self) -> Result<()> {
        info!("🔄 Force reconnecting to all exchanges");
        
        for exchange in self.config.exchanges.keys() {
            if let Err(e) = self.exchange_manager.lock().await.force_reconnect(*exchange).await {
                warn!("Failed to reconnect to {}: {}", exchange, e);
            }
        }
        
        Ok(())
    }
}

/// Bridge service statistics
#[derive(Debug, Clone)]
pub struct BridgeStats {
    pub connected_exchanges: usize,
    pub total_exchanges: usize,
    pub total_messages_received: u64,
    pub total_errors: u64,
    pub active_symbols: usize,
    pub cached_signals: usize,
    pub order_books_cached: usize,
}