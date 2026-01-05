use arbitrage_core::types::{ExchangeId, OrderBook, Symbol, ConnectionStatus};
use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Error, Debug)]
pub enum ConnectorError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Authentication error: {0}")]
    Authentication(String),
    
    #[error("Rate limit error: {0}")]
    RateLimit(String),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),
}

/// Events emitted by exchange connectors
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    Connected(ExchangeId),
    Disconnected(ExchangeId),
    Reconnecting(ExchangeId),
    Error(ExchangeId, String),
    OrderBookUpdate(OrderBook),
    RateLimit(ExchangeId, u64), // seconds until reset
}

/// Configuration for exchange connector
#[derive(Debug, Clone)]
pub struct ConnectorConfig {
    pub exchange: ExchangeId,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub testnet: bool,
    pub symbols: Vec<Symbol>,
    pub rate_limit_per_second: u32,
    pub reconnect_delay_ms: u64,
    pub max_reconnect_attempts: u32,
    pub heartbeat_interval_ms: u64,
}

/// Trait for exchange connectors
#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    /// Get exchange ID
    fn exchange_id(&self) -> ExchangeId;

    /// Connect to exchange
    async fn connect(&mut self) -> Result<(), ConnectorError>;

    /// Disconnect from exchange
    async fn disconnect(&mut self) -> Result<(), ConnectorError>;

    /// Subscribe to symbols
    async fn subscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError>;

    /// Unsubscribe from symbols
    async fn unsubscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError>;

    /// Get current connection status
    fn status(&self) -> ConnectionStatus;

    /// Get event receiver
    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent>;

    /// Get subscribed symbols
    fn subscribed_symbols(&self) -> Vec<Symbol>;
}