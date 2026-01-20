use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, Symbol},
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tracing::{error, info, warn};

use crate::connector::{ConnectorStats, ExchangeConnector, HealthStatus};
use crate::events::{ConnectionEvent, EventStats, MarketDataEvent};
use crate::rate_limiter::{RateLimitConfig, UnifiedRateLimitManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeManagerConfig {
    pub enabled_exchanges: Vec<ExchangeId>,
    pub health_check_interval_seconds: u64,
    pub max_reconnect_attempts: u32,
    pub event_buffer_size: usize,
    pub rate_limits: HashMap<ExchangeId, RateLimitConfig>,
    pub auto_reconnect: bool,
    pub max_parallel_connections: usize,
}

impl Default for ExchangeManagerConfig {
    fn default() -> Self {
        let mut rate_limits = HashMap::new();
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

pub struct ExchangeManager {
    config: ExchangeManagerConfig,
    connectors: HashMap<ExchangeId, Arc<RwLock<Box<dyn ExchangeConnector + Send + Sync>>>>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    event_receiver: broadcast::Receiver<ConnectionEvent>,
    subscribed_symbols: HashMap<ExchangeId, Vec<Symbol>>,
    event_stats: HashMap<ExchangeId, EventStats>,
    rate_limiters: HashMap<ExchangeId, UnifiedRateLimitManager>,
}

impl ExchangeManager {
    pub fn new(config: ExchangeManagerConfig) -> Self {
        let (event_sender, event_receiver) = broadcast::channel(config.event_buffer_size);
        Self {
            config,
            connectors: HashMap::new(),
            event_sender,
            event_receiver,
            subscribed_symbols: HashMap::new(),
            event_stats: HashMap::new(),
            rate_limiters: HashMap::new(),
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        for exchange_id in &self.config.enabled_exchanges {
            let connector = crate::create_connector(*exchange_id)?;
            let rate_limit_config = self
                .config
                .rate_limits
                .get(exchange_id)
                .cloned()
                .unwrap_or_default();
            let mut rate_limiter = UnifiedRateLimitManager::new();
            rate_limiter.add_limiter("rest".to_string(), rate_limit_config.clone());
            rate_limiter.add_limiter("websocket".to_string(), rate_limit_config);
            let mut stats = EventStats::default();
            stats.exchange = *exchange_id;
            self.connectors
                .insert(*exchange_id, Arc::new(RwLock::new(connector)));
            self.rate_limiters.insert(*exchange_id, rate_limiter);
            self.event_stats.insert(*exchange_id, stats);
        }
        self.start_health_monitor().await;
        self.start_event_processor().await;
        Ok(())
    }

    pub async fn connect_all(&mut self) -> Result<()> {
        for (exchange, connector) in self.connectors.iter_mut() {
            let exchange = *exchange;
            let mut connector = connector.write().await;
            info!("Connecting to {}", exchange);
            match connector.connect().await {
                Ok(()) => {
                    let _ = self.event_sender.send(ConnectionEvent::StatusChange {
                        exchange,
                        old_status: ConnectionStatus::Disconnected,
                        new_status: ConnectionStatus::Connected,
                        timestamp: chrono::Utc::now(),
                    });
                }
                Err(e) => {
                    error!("Failed to connect to {}: {}", exchange, e);
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

    pub async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        for symbol in symbols {
            self.subscribed_symbols
                .values_mut()
                .for_each(|s| s.push(symbol.clone()));
        }
        for (exchange, connector) in self.connectors.iter_mut() {
            let exchange = *exchange;
            let mut connector = connector.write().await;
            if let Err(e) = connector.subscribe_symbols(symbols).await {
                warn!("Failed to subscribe on {}: {}", exchange, e);
            }
        }
        Ok(())
    }

    pub async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        for symbol in symbols {
            self.subscribed_symbols.values_mut().for_each(|s| {
                s.retain(|sym| sym != symbol);
            });
        }
        for (exchange, connector) in self.connectors.iter_mut() {
            let exchange = *exchange;
            let mut connector = connector.write().await;
            if let Err(e) = connector.unsubscribe_symbols(symbols).await {
                warn!("Failed to unsubscribe on {}: {}", exchange, e);
            }
        }
        Ok(())
    }

    pub fn get_event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_receiver.resubscribe()
    }

    pub async fn get_order_book(&self, exchange: ExchangeId, symbol: &Symbol) -> Result<OrderBook> {
        if let Some(connector) = self.connectors.get(&exchange) {
            let connector = connector.read().await;
            connector.fetch_order_book(symbol).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    pub async fn get_order_books_all(
        &self,
        symbol: &Symbol,
    ) -> HashMap<ExchangeId, Result<OrderBook>> {
        let mut results = HashMap::new();
        for (exchange, connector) in self.connectors.iter() {
            let exchange = *exchange;
            let connector = connector.read().await;
            results.insert(exchange, connector.fetch_order_book(symbol).await);
        }
        results
    }

    pub async fn get_health_status(&self) -> HashMap<ExchangeId, HealthStatus> {
        let mut status_map = HashMap::new();
        for (exchange, connector) in self.connectors.iter() {
            let exchange = *exchange;
            let connector = connector.read().await;
            match connector.health_check().await {
                Ok(status) => {
                    status_map.insert(exchange, status);
                }
                Err(_) => {
                    status_map.insert(
                        exchange,
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

    pub async fn get_stats(&self) -> HashMap<ExchangeId, ConnectorStats> {
        let mut stats_map = HashMap::new();
        for (exchange, connector) in self.connectors.iter() {
            let exchange = *exchange;
            let connector = connector.read().await;
            stats_map.insert(exchange, connector.get_stats());
        }
        stats_map
    }

    pub async fn get_event_stats(&self) -> HashMap<ExchangeId, EventStats> {
        self.event_stats
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    pub async fn force_reconnect(&mut self, exchange: ExchangeId) -> Result<()> {
        if let Some(connector) = self.connectors.get_mut(&exchange) {
            let mut connector = connector.write().await;
            connector.force_reconnect().await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not found",
                exchange
            )))
        }
    }

    async fn start_health_monitor(&self) {
        let connectors = self.connectors.clone();
        let event_sender = self.event_sender.clone();
        let interval_seconds = self.config.health_check_interval_seconds;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
            loop {
                interval.tick().await;
                for (exchange, connector) in connectors.iter() {
                    let exchange = *exchange;
                    let connector = connector.read().await;
                    let sender = event_sender.clone();
                    if let Ok(health) = connector.health_check().await {
                        if !health.is_connected {
                            let _ = sender.send(ConnectionEvent::StatusChange {
                                exchange,
                                old_status: ConnectionStatus::Connected,
                                new_status: ConnectionStatus::Disconnected,
                                timestamp: chrono::Utc::now(),
                            });
                        }
                    }
                }
            }
        });
    }

    async fn start_event_processor(&self) {
        let mut event_stats = self.event_stats.clone();
        let mut event_receiver = self.event_receiver.resubscribe();
        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                if let ConnectionEvent::MarketData(ref market_event) = event {
                    let exchange = match market_event {
                        MarketDataEvent::OrderBook { exchange, .. } => *exchange,
                        MarketDataEvent::Ticker { exchange, .. } => *exchange,
                        MarketDataEvent::Trade { exchange, .. } => *exchange,
                        MarketDataEvent::FundingRate { exchange, .. } => *exchange,
                        MarketDataEvent::Statistics { exchange, .. } => *exchange,
                        MarketDataEvent::Raw { exchange, .. } => *exchange,
                    };
                    if let Some(exchange_stats) = event_stats.get_mut(&exchange) {
                        exchange_stats.update(&event);
                    }
                }
            }
        });
    }

    pub async fn add_connector(&mut self, connector: Box<dyn ExchangeConnector>) -> Result<()> {
        let exchange_id = connector.exchange_id();
        self.connectors
            .insert(exchange_id, Arc::new(RwLock::new(connector)));
        Ok(())
    }

    pub async fn get_connector(
        &self,
        exchange: &ExchangeId,
    ) -> Option<Arc<RwLock<Box<dyn ExchangeConnector + Send + Sync>>>> {
        self.connectors.get(exchange).map(|e| Arc::clone(e))
    }

    pub async fn is_exchange_connected(&self, exchange: &ExchangeId) -> bool {
        if let Some(connector) = self.connectors.get(exchange) {
            let connector = connector.read().await;
            connector
                .health_check()
                .await
                .map_or(false, |h| h.is_connected)
        } else {
            false
        }
    }

    pub async fn place_order(
        &self,
        exchange: &ExchangeId,
        order: &crate::connector::OrderRequest,
    ) -> Result<crate::connector::OrderResponse> {
        if let Some(connector) = self.connectors.get(exchange) {
            let connector = connector.read().await;
            connector.place_order(order).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    pub async fn cancel_order(
        &self,
        exchange: &ExchangeId,
        order_id: &str,
    ) -> Result<crate::connector::CancelResponse> {
        if let Some(connector) = self.connectors.get(exchange) {
            let connector = connector.read().await;
            connector.cancel_order(order_id).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    pub async fn get_order_status(
        &self,
        exchange: &ExchangeId,
        order_id: &str,
    ) -> Result<crate::connector::OrderStatus> {
        if let Some(connector) = self.connectors.get(exchange) {
            let connector = connector.read().await;
            connector.get_order_status(order_id).await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }

    pub async fn get_balance(&self, exchange: &ExchangeId) -> Result<crate::connector::Balance> {
        if let Some(connector) = self.connectors.get(exchange) {
            let connector = connector.read().await;
            connector.get_balance().await
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(format!(
                "Exchange {} not available",
                exchange
            )))
        }
    }
}
