use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, Symbol},
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::connector::{ConnectorStats, ExchangeConnector, HealthStatus};
use crate::events::{ConnectionEvent, EventStats, MarketDataEvent};
use crate::rate_limiter::{RateLimitConfig, UnifiedRateLimitManager};

/// Configuration for exchange manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeManagerConfig {
    /// Enabled exchanges
    pub enabled_exchanges: Vec<ExchangeId>,

    /// Health check interval in seconds
    pub health_check_interval_seconds: u64,

    /// Maximum reconnection attempts per exchange
    pub max_reconnect_attempts: u32,

    /// Event buffer size
    pub event_buffer_size: usize,

    /// Rate limiting configurations per exchange
    pub rate_limits: HashMap<ExchangeId, RateLimitConfig>,

    /// Auto-reconnect on failures
    pub auto_reconnect: bool,

    /// Parallel connection limit
    pub max_parallel_connections: usize,
}

impl Default for ExchangeManagerConfig {
    fn default() -> Self {
        let mut rate_limits = HashMap::new();

        // Default rate limits for each exchange
        rate_limits.insert(
            ExchangeId::OKX,
            RateLimitConfig {
                requests_per_second: 40,
                burst_capacity: 80,
                window_seconds: 60,
            },
        );

        rate_limits.insert(
            ExchangeId::ByBit,
            RateLimitConfig {
                requests_per_second: 60,
                burst_capacity: 120,
                window_seconds: 60,
            },
        );

        rate_limits.insert(
            ExchangeId::MEXC,
            RateLimitConfig {
                requests_per_second: 20,
                burst_capacity: 40,
                window_seconds: 60,
            },
        );

        rate_limits.insert(
            ExchangeId::GateIo,
            RateLimitConfig {
                requests_per_second: 30,
                burst_capacity: 60,
                window_seconds: 60,
            },
        );

        rate_limits.insert(
            ExchangeId::Bitstamp,
            RateLimitConfig {
                requests_per_second: 8000,
                burst_capacity: 16000,
                window_seconds: 60,
            },
        );

        rate_limits.insert(
            ExchangeId::Kraken,
            RateLimitConfig {
                requests_per_second: 1,
                burst_capacity: 2,
                window_seconds: 60,
            },
        );

        Self {
            enabled_exchanges: vec![
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::MEXC,
                ExchangeId::GateIo,
                ExchangeId::Bitstamp,
                ExchangeId::Kraken,
            ],
            health_check_interval_seconds: 30,
            max_reconnect_attempts: 5,
            event_buffer_size: 10000,
            rate_limits,
            auto_reconnect: true,
            max_parallel_connections: 10,
        }
    }
}

/// Manages all exchange connectors and provides unified interface
pub struct ExchangeManager {
    config: ExchangeManagerConfig,
    connectors: Arc<RwLock<HashMap<ExchangeId, Box<dyn ExchangeConnector>>>>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    event_receiver: broadcast::Receiver<ConnectionEvent>,
    subscribed_symbols: Arc<RwLock<HashSet<Symbol>>>,
    event_stats: Arc<RwLock<HashMap<ExchangeId, EventStats>>>,
    rate_limiters: Arc<RwLock<HashMap<ExchangeId, UnifiedRateLimitManager>>>,
}

impl ExchangeManager {
    /// Create new exchange manager
    pub fn new(config: ExchangeManagerConfig) -> Self {
        let (event_sender, event_receiver) = broadcast::channel(config.event_buffer_size);

        Self {
            config,
            connectors: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver,
            subscribed_symbols: Arc::new(RwLock::new(HashSet::new())),
            event_stats: Arc::new(RwLock::new(HashMap::new())),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize all enabled exchange connectors
    pub async fn initialize(&mut self) -> Result<()> {
        info!(
            "Initializing exchange manager with {} exchanges",
            self.config.enabled_exchanges.len()
        );

        let mut connectors = self.connectors.write().await;
        let mut rate_limiters = self.rate_limiters.write().await;
        let mut event_stats = self.event_stats.write().await;

        for exchange_id in &self.config.enabled_exchanges {
            // Create connector
            let connector = crate::create_connector(*exchange_id)?;

            // Initialize rate limiter
            let rate_limit_config = self
                .config
                .rate_limits
                .get(exchange_id)
                .cloned()
                .unwrap_or_default();

            let mut rate_limiter = UnifiedRateLimitManager::new();
            rate_limiter.add_limiter("rest".to_string(), rate_limit_config.clone());
            rate_limiter.add_limiter("websocket".to_string(), rate_limit_config);

            // Initialize event stats
            let mut stats = EventStats::default();
            stats.exchange = *exchange_id;

            connectors.insert(*exchange_id, connector);
            rate_limiters.insert(*exchange_id, rate_limiter);
            event_stats.insert(*exchange_id, stats);

            info!("Initialized connector for {}", exchange_id);
        }

        // Start background tasks
        self.start_health_monitor().await;
        self.start_event_processor().await;

        Ok(())
    }

    /// Connect all exchange connectors
    pub async fn connect_all(&mut self) -> Result<()> {
        info!("Connecting to all exchanges");

        let mut connectors = self.connectors.write().await;

        for (exchange_id, connector) in connectors.iter_mut() {
            let exchange = *exchange_id;
            info!("Connecting to {}", exchange);

            match connector.connect().await {
                Ok(()) => {
                    info!("Successfully connected to {}", exchange);

                    // Send connection event
                    let _ = self.event_sender.send(ConnectionEvent::StatusChange {
                        exchange,
                        old_status: ConnectionStatus::Disconnected,
                        new_status: ConnectionStatus::Connected,
                        timestamp: chrono::Utc::now(),
                    });
                }
                Err(e) => {
                    error!("Failed to connect to {}: {}", exchange, e);

                    // Send error event
                    let _ = self.event_sender.send(ConnectionEvent::Error {
                        exchange,
                        error: format!("Health check failed: {}", e),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Subscribe to symbols across all exchanges
    pub async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!(
            "Subscribing to {} symbols across all exchanges",
            symbols.len()
        );

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write().await;
            for symbol in symbols {
                subscribed.insert(symbol.clone());
            }
        }

        let mut connectors = self.connectors.write().await;

        for (exchange_id, connector) in connectors.iter_mut() {
            let exchange = *exchange_id;

            match connector.subscribe_symbols(symbols).await {
                Ok(()) => {
                    info!("Successfully subscribed to symbols on {}", exchange);

                    // Send subscription confirmation
                    let _ = self
                        .event_sender
                        .send(ConnectionEvent::SubscriptionConfirmed {
                            exchange,
                            symbols: symbols.to_vec(),
                            data_type: "symbols".to_string(),
                            timestamp: chrono::Utc::now(),
                        });
                }
                Err(e) => {
                    warn!("Failed to subscribe to symbols on {}: {}", exchange, e);
                }
            }
        }

        Ok(())
    }

    /// Unsubscribe from symbols across all exchanges
    pub async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!(
            "Unsubscribing from {} symbols across all exchanges",
            symbols.len()
        );

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write().await;
            for symbol in symbols {
                subscribed.remove(symbol);
            }
        }

        let mut connectors = self.connectors.write().await;

        for (exchange_id, connector) in connectors.iter_mut() {
            let exchange = *exchange_id;

            match connector.unsubscribe_symbols(symbols).await {
                Ok(()) => {
                    info!("Successfully unsubscribed from symbols on {}", exchange);
                }
                Err(e) => {
                    warn!("Failed to unsubscribe from symbols on {}: {}", exchange, e);
                }
            }
        }

        Ok(())
    }

    /// Get unified event receiver
    pub fn get_event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_receiver.resubscribe()
    }

    /// Get order book from specific exchange
    pub async fn get_order_book(&self, exchange: ExchangeId, symbol: &Symbol) -> Result<OrderBook> {
        let connectors = self.connectors.read().await;

        if let Some(connector) = connectors.get(&exchange) {
            // Apply rate limiting
            if let Some(rate_limiter) = self.rate_limiters.read().await.get(&exchange) {
                rate_limiter.acquire("rest").await?;
            }

            connector.fetch_order_book(symbol).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    /// Get order books from all exchanges for a symbol
    pub async fn get_order_books_all(
        &self,
        symbol: &Symbol,
    ) -> HashMap<ExchangeId, Result<OrderBook>> {
        let connectors = self.connectors.read().await;
        let mut results = HashMap::new();

        for (exchange_id, connector) in connectors.iter() {
            let exchange = *exchange_id;

            // Apply rate limiting
            let rate_limit_result =
                if let Some(rate_limiter) = self.rate_limiters.read().await.get(&exchange) {
                    rate_limiter.try_acquire("rest")
                } else {
                    Ok(())
                };

            let result = match rate_limit_result {
                Ok(()) => connector.fetch_order_book(symbol).await,
                Err(e) => Err(e.into()),
            };

            results.insert(exchange, result);
        }

        results
    }

    /// Get health status of all exchanges
    pub async fn get_health_status(&self) -> HashMap<ExchangeId, HealthStatus> {
        let connectors = self.connectors.read().await;
        let mut status_map = HashMap::new();

        for (exchange_id, connector) in connectors.iter() {
            match connector.health_check().await {
                Ok(status) => {
                    status_map.insert(*exchange_id, status);
                }
                Err(e) => {
                    warn!("Health check failed for {}: {}", exchange_id, e);
                    status_map.insert(
                        *exchange_id,
                        HealthStatus {
                            is_connected: false,
                            last_message_time: None,
                            websocket_status: ConnectionStatus::Disconnected,
                            rest_api_status: ConnectionStatus::Disconnected,
                            error_count: 1,
                            reconnect_count: 0,
                        },
                    );
                }
            }
        }

        status_map
    }

    /// Get statistics for all exchanges
    pub async fn get_stats(&self) -> HashMap<ExchangeId, ConnectorStats> {
        let connectors = self.connectors.read().await;
        let mut stats_map = HashMap::new();

        for (exchange_id, connector) in connectors.iter() {
            let stats = connector.get_stats();
            stats_map.insert(*exchange_id, stats);
        }

        stats_map
    }

    /// Get event statistics
    pub async fn get_event_stats(&self) -> HashMap<ExchangeId, EventStats> {
        self.event_stats.read().await.clone()
    }

    /// Force reconnection for specific exchange
    pub async fn force_reconnect(&mut self, exchange: ExchangeId) -> Result<()> {
        let mut connectors = self.connectors.write().await;

        if let Some(connector) = connectors.get_mut(&exchange) {
            info!("Force reconnecting to {}", exchange);
            connector.force_reconnect().await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not found",
                exchange
            )))
        }
    }

    /// Start health monitoring background task
    async fn start_health_monitor(&self) {
        let connectors = Arc::clone(&self.connectors);
        let event_sender = self.event_sender.clone();
        let interval_seconds = self.config.health_check_interval_seconds;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_seconds));

            loop {
                interval.tick().await;

                let connectors_read = connectors.read().await;
                for (exchange_id, connector) in connectors_read.iter() {
                    let exchange = *exchange_id;

                    match connector.health_check().await {
                        Ok(health) => {
                            if !health.is_connected {
                                warn!("Exchange {} is not connected", exchange);

                                let _ = event_sender.send(ConnectionEvent::StatusChange {
                                    exchange,
                                    old_status: ConnectionStatus::Connected,
                                    new_status: ConnectionStatus::Disconnected,
                                    timestamp: chrono::Utc::now(),
                                });
                            }
                        }
                        Err(e) => {
                            error!("Health check failed for {}: {}", exchange, e);

                            let _ = event_sender.send(ConnectionEvent::Error {
                                exchange,
                                error: format!("Health check failed: {}", e),
                                timestamp: chrono::Utc::now(),
                            });
                        }
                    }
                }
            }
        });
    }

    /// Start event processing background task
    async fn start_event_processor(&self) {
        let event_stats = Arc::clone(&self.event_stats);
        let mut event_receiver = self.event_receiver.resubscribe();

        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                // Update event statistics
                if let ConnectionEvent::MarketData(ref market_event) = event {
                    let exchange = match market_event {
                        MarketDataEvent::OrderBook { exchange, .. } => *exchange,
                        MarketDataEvent::Ticker { exchange, .. } => *exchange,
                        MarketDataEvent::Trade { exchange, .. } => *exchange,
                        MarketDataEvent::FundingRate { exchange, .. } => *exchange,
                        MarketDataEvent::Statistics { exchange, .. } => *exchange,
                        MarketDataEvent::Raw { exchange, .. } => *exchange,
                    };

                    if let Ok(mut stats) = event_stats.try_write() {
                        if let Some(exchange_stats) = stats.get_mut(&exchange) {
                            exchange_stats.update(&event);
                        }
                    }
                }

                debug!("Processed event: {:?}", event);
            }
        });
    }

    /// Add a connector to the manager
    pub async fn add_connector(&mut self, connector: Box<dyn ExchangeConnector>) -> Result<()> {
        let exchange_id = connector.exchange_id();
        let mut connectors = self.connectors.write().await;
        connectors.insert(exchange_id, connector);

        info!("Added connector for {}", exchange_id);
        Ok(())
    }

    /// Get a connector for a specific exchange
    pub async fn get_connector(
        &self,
        exchange: &ExchangeId,
    ) -> Option<Box<dyn ExchangeConnector + '_>> {
        // This is a simplified version - in practice we'd need to handle the lifetime properly
        // For now, we'll return None and handle this in the OrderExecutor differently
        None
    }

    /// Check if an exchange is connected
    pub async fn is_exchange_connected(&self, exchange: &ExchangeId) -> bool {
        let connectors = self.connectors.read().await;
        if let Some(connector) = connectors.get(exchange) {
            match connector.health_check().await {
                Ok(health) => health.is_connected,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Place order on specific exchange
    pub async fn place_order(
        &self,
        exchange: &ExchangeId,
        order: &crate::connector::OrderRequest,
    ) -> Result<crate::connector::OrderResponse> {
        let connectors = self.connectors.read().await;

        if let Some(connector) = connectors.get(exchange) {
            // Apply rate limiting
            if let Some(rate_limiter) = self.rate_limiters.read().await.get(exchange) {
                rate_limiter.acquire("rest").await?;
            }

            connector.place_order(order).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    /// Cancel order on specific exchange
    pub async fn cancel_order(
        &self,
        exchange: &ExchangeId,
        order_id: &str,
    ) -> Result<crate::connector::CancelResponse> {
        let connectors = self.connectors.read().await;

        if let Some(connector) = connectors.get(exchange) {
            // Apply rate limiting
            if let Some(rate_limiter) = self.rate_limiters.read().await.get(exchange) {
                rate_limiter.acquire("rest").await?;
            }

            connector.cancel_order(order_id).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    /// Get order status from specific exchange
    pub async fn get_order_status(
        &self,
        exchange: &ExchangeId,
        order_id: &str,
    ) -> Result<crate::connector::OrderStatus> {
        let connectors = self.connectors.read().await;

        if let Some(connector) = connectors.get(exchange) {
            // Apply rate limiting
            if let Some(rate_limiter) = self.rate_limiters.read().await.get(exchange) {
                rate_limiter.acquire("rest").await?;
            }

            connector.get_order_status(order_id).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    /// Get balance from specific exchange
    pub async fn get_balance(&self, exchange: &ExchangeId) -> Result<crate::connector::Balance> {
        let connectors = self.connectors.read().await;

        if let Some(connector) = connectors.get(exchange) {
            // Apply rate limiting
            if let Some(rate_limiter) = self.rate_limiters.read().await.get(exchange) {
                rate_limiter.acquire("rest").await?;
            }

            connector.get_balance().await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }
}
