use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, Symbol},
    Result,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::broadcast;

use crate::events::ConnectionEvent;

/// Configuration for exchange connector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    /// Exchange identifier
    pub exchange_id: ExchangeId,

    /// WebSocket URL
    pub ws_url: String,

    /// REST API base URL
    pub rest_url: String,

    /// API credentials (optional for public data)
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,

    /// Rate limiting configuration
    pub rate_limit_per_second: u32,
    pub rate_limit_burst: u32,

    /// Connection settings
    pub reconnect_interval_ms: u64,
    pub max_reconnect_attempts: u32,
    pub heartbeat_interval_ms: u64,

    /// Data settings
    pub order_book_depth: u32,
    pub enable_trades: bool,
    pub enable_tickers: bool,
    pub enable_funding_rates: bool,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            exchange_id: ExchangeId::OKX,
            ws_url: String::new(),
            rest_url: String::new(),
            api_key: None,
            api_secret: None,
            passphrase: None,
            rate_limit_per_second: 10,
            rate_limit_burst: 20,
            reconnect_interval_ms: 5000,
            max_reconnect_attempts: 10,
            heartbeat_interval_ms: 30000,
            order_book_depth: 20,
            enable_trades: true,
            enable_tickers: true,
            enable_funding_rates: false,
        }
    }
}

/// Unified interface for all exchange connectors
#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    /// Get exchange identifier
    fn exchange_id(&self) -> ExchangeId;

    /// Get current connection status
    fn status(&self) -> ConnectionStatus;

    /// Get event receiver for this connector
    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent>;

    // === REST API Methods ===

    /// Fetch current order book for a symbol
    async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook>;

    /// Fetch all available trading symbols
    async fn fetch_symbols(&self) -> Result<Vec<Symbol>>;

    /// Fetch ticker data for symbols
    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>>;

    /// Fetch funding rates (for perpetual contracts)
    async fn fetch_funding_rates(&self, symbols: &[Symbol])
        -> Result<HashMap<Symbol, FundingRate>>;

    // === WebSocket Methods ===

    /// Connect to WebSocket streams
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from WebSocket streams
    async fn disconnect(&mut self) -> Result<()>;

    /// Subscribe to symbols for real-time data
    async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Unsubscribe from symbols
    async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribe to ticker updates
    async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribe to order book updates
    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribe to trade updates
    async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribe to funding rate updates (for perpetual contracts)
    async fn subscribe_funding_rates(&mut self, symbols: &[Symbol]) -> Result<()>;

    // === Health & Monitoring ===

    /// Check if connector is healthy
    async fn health_check(&self) -> Result<HealthStatus>;

    /// Get connector statistics
    fn get_stats(&self) -> ConnectorStats;

    /// Force reconnection
    async fn force_reconnect(&mut self) -> Result<()>;

    // === Trading Methods (Phase 4) ===

    /// Place a new order
    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse>;

    /// Cancel an existing order
    async fn cancel_order(&self, order_id: &str) -> Result<CancelResponse>;

    /// Get order status
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus>;

    /// Get account balance
    async fn get_balance(&self) -> Result<Balance>;

    /// Get open orders
    async fn get_open_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<OrderStatus>>;
}

/// Ticker data from exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerData {
    pub symbol: Symbol,
    pub exchange: ExchangeId,
    pub last_price: rust_decimal::Decimal,
    pub bid_price: rust_decimal::Decimal,
    pub ask_price: rust_decimal::Decimal,
    pub volume_24h: rust_decimal::Decimal,
    pub price_change_24h: rust_decimal::Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Funding rate data for perpetual contracts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: Symbol,
    pub exchange: ExchangeId,
    pub funding_rate: rust_decimal::Decimal,
    pub predicted_rate: Option<rust_decimal::Decimal>,
    pub funding_time: chrono::DateTime<chrono::Utc>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Health status of connector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub is_connected: bool,
    pub last_message_time: Option<chrono::DateTime<chrono::Utc>>,
    pub websocket_status: ConnectionStatus,
    pub rest_api_status: ConnectionStatus,
    pub error_count: u64,
    pub reconnect_count: u32,
}

/// Connector performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorStats {
    pub exchange: ExchangeId,
    pub uptime_seconds: u64,
    pub messages_received: u64,
    pub messages_sent: u64,
    pub errors_count: u64,
    pub reconnections: u32,
    pub avg_latency_ms: f64,
    pub subscribed_symbols: usize,
    pub rate_limit_hits: u64,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

impl Default for ConnectorStats {
    fn default() -> Self {
        Self {
            exchange: ExchangeId::OKX,
            uptime_seconds: 0,
            messages_received: 0,
            messages_sent: 0,
            errors_count: 0,
            reconnections: 0,
            avg_latency_ms: 0.0,
            subscribed_symbols: 0,
            rate_limit_hits: 0,
            last_update: chrono::Utc::now(),
        }
    }
}

/// Order request for placing trades
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: rust_decimal::Decimal,
    pub price: Option<rust_decimal::Decimal>,
    pub time_in_force: TimeInForce,
    pub client_order_id: Option<String>,
}

/// Order side (buy/sell)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    StopLossLimit,
    TakeProfit,
    TakeProfitLimit,
}

/// Time in force for orders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC, // Good Till Cancelled
    IOC, // Immediate Or Cancel
    FOK, // Fill Or Kill
    GTD, // Good Till Date
}

/// Order response after placement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: rust_decimal::Decimal,
    pub price: Option<rust_decimal::Decimal>,
    pub status: OrderStatusType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Order cancellation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelResponse {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub status: OrderStatusType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Order status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatus {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: rust_decimal::Decimal,
    pub price: Option<rust_decimal::Decimal>,
    pub filled_quantity: rust_decimal::Decimal,
    pub remaining_quantity: rust_decimal::Decimal,
    pub average_price: Option<rust_decimal::Decimal>,
    pub status: OrderStatusType,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Order status types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderStatusType {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Account balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub exchange: ExchangeId,
    pub balances: HashMap<String, AssetBalance>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Individual asset balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetBalance {
    pub asset: String,
    pub free: rust_decimal::Decimal,
    pub locked: rust_decimal::Decimal,
    pub total: rust_decimal::Decimal,
}
