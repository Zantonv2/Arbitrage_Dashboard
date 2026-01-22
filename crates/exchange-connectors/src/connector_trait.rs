use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::{broadcast, RwLock};

use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol},
    ArbitrageError, Result,
};

use crate::connections::constants::BROADCAST_CHANNEL_CAPACITY;
use crate::connector::{
    Balance, ConnectorConfig, ConnectorStats, FundingRate, HealthStatus, OrderRequest,
    OrderResponse, OrderStatus, TickerData,
};
use crate::events::ConnectionEvent;

/// Base structure shared by all exchange connectors.
///
/// Contains common state and functionality that is inherited by all
/// concrete connector implementations. Provides thread-safe status
/// tracking, statistics collection, and event broadcasting capabilities.
///
/// # Example
///
/// ```rust
/// use exchange_connectors::{connector::ConnectorConfig, connector_trait::ConnectorBase};
///
/// let config = ConnectorConfig {
///     exchange_id: arbitrage_core::types::ExchangeId::OKX,
///     order_book_depth: 10,
///     ..Default::default()
/// };
///
/// let base = ConnectorBase::new(config);
/// assert_eq!(base.config.exchange_id, arbitrage_core::types::ExchangeId::OKX);
/// ```
#[derive(Clone)]
pub struct ConnectorBase {
    /// Connector configuration
    pub config: ConnectorConfig,
    /// Connection status (thread-safe)
    pub status: Arc<RwLock<ConnectionStatus>>,
    /// Connector statistics (thread-safe)
    pub stats: Arc<Mutex<ConnectorStats>>,
    /// Event broadcast sender for connection events
    pub event_sender: broadcast::Sender<ConnectionEvent>,
}

impl ConnectorBase {
    /// Creates a new ConnectorBase with the given configuration.
    ///
    /// Initializes the connector with default statistics and creates
    /// an event broadcast channel with a buffer size of 1000.
    ///
    /// # Arguments
    ///
    /// * `config` - The connector configuration
    ///
    /// # Example
    ///
    /// ```rust
    /// use exchange_connectors::{connector::ConnectorConfig, connector_trait::ConnectorBase};
    ///
    /// let config = ConnectorConfig {
    ///     exchange_id: arbitrage_core::types::ExchangeId::ByBit,
    ///     order_book_depth: 20,
    ///     ..Default::default()
    /// };
    ///
    /// let base = ConnectorBase::new(config);
    /// ```
    pub fn new(config: ConnectorConfig) -> Self {
        let (event_sender, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        let stats = ConnectorStats {
            exchange: config.exchange_id,
            ..Default::default()
        };

        Self {
            config,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
            event_sender,
        }
    }

    /// Establishes a connection to the exchange.
    ///
    /// This is a no-op in the base implementation. Concrete connectors
    /// should override this to perform actual connection logic.
    ///
    /// # Returns
    ///
    /// `Ok(())` if already connected, otherwise transitions to connecting state.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection attempt fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut base = ConnectorBase::new(config);
    /// base.connect().await?;
    /// ```
    pub async fn connect(&mut self) -> Result<()> {
        {
            let status = self.status.read().await;
            if *status == ConnectionStatus::Connected {
                return Ok(());
            }
        }

        *self.status.write().await = ConnectionStatus::Connecting;

        Ok(())
    }

    /// Disconnects from the exchange.
    ///
    /// Transitions the connection status to Disconnected.
    /// Concrete connectors should override this to perform cleanup.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful disconnect.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut base = ConnectorBase::new(config);
    /// base.disconnect().await?;
    /// ```
    pub async fn disconnect(&mut self) -> Result<()> {
        *self.status.write().await = ConnectionStatus::Disconnected;
        Ok(())
    }

    /// Retrieves connector statistics without locking.
    ///
    /// Attempts to acquire the stats lock and return a clone.
    /// Returns default stats if the lock is already held.
    ///
    /// # Returns
    ///
    /// Current connector statistics.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stats = base.get_stats();
    /// println!("Errors: {}", stats.errors_count);
    /// ```
    pub fn get_stats(&self) -> ConnectorStats {
        match self.stats.try_lock() {
            Ok(stats) => stats.clone(),
            Err(_) => ConnectorStats::default(),
        }
    }

    /// Retrieves connector statistics with a blocking lock.
    ///
    /// Blocks until the lock is available, then returns a guard
    /// allowing access to the stats. The guard automatically releases
    /// the lock when dropped.
    ///
    /// # Returns
    ///
    /// A mutex guard providing access to the statistics.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stats = base.stats();
    /// println!("Last update: {:?}", stats.last_update);
    /// ```
    pub fn stats(&self) -> MutexGuard<'_, ConnectorStats> {
        self.stats.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Asynchronous trait defining the interface for exchange connectors.
///
/// This trait provides a unified interface for interacting with cryptocurrency
/// exchanges. All exchange-specific connectors implement this trait to provide
/// common functionality for market data retrieval, order management, and
/// connection handling.
///
/// # Implementors
///
/// Implementors must provide implementations for:
/// - `id()`: Return the exchange identifier
/// - `config()`: Return the connector configuration
/// - `base()`: Return reference to the connector base
/// - `base_mut()`: Return mutable reference to the connector base
/// - `event_receiver()`: Return an event receiver for connection events
/// - `fetch_order_book()`: Fetch order book for a symbol
/// - `fetch_symbols()`: Fetch available trading symbols
/// - `fetch_tickers()`: Fetch ticker data for symbols
/// - `fetch_funding_rates()`: Fetch funding rates for perpetual symbols
/// - Subscription methods: `subscribe_symbols`, `unsubscribe_symbols`, etc.
///
/// # Market Data Methods
///
/// Market data methods (`fetch_*` and `subscribe_*`) are required to be
/// implemented by all connectors. Trading methods (`place_order`, `cancel_order`,
/// etc.) provide default implementations that return `NotImplemented`.
///
/// # Example
///
/// ```ignore
/// use async_trait::async_trait;
/// use exchange_connectors::connector_trait::{ConnectorBase, ExchangeConnector};
/// use arbitrage_core::types::{ExchangeId, Symbol};
///
/// struct MyConnector {
///     base: ConnectorBase,
/// }
///
/// #[async_trait]
/// impl ExchangeConnector for MyConnector {
///     fn id(&self) -> ExchangeId {
///         ExchangeId::OKX
///     }
///
///     fn config(&self) -> &ConnectorConfig {
///         &self.base.config
///     }
///
///     fn base(&self) -> &ConnectorBase {
///         &self.base
///     }
///
///     fn base_mut(&mut self) -> &mut ConnectorBase {
///         &mut self.base
///     }
///
///     fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
///         self.base.event_sender.subscribe()
///     }
///
///     async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook> {
///         // Implementation specific to OKX
///         unimplemented!()
///     }
///
///     async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
///         // Implementation specific to OKX
///         unimplemented!()
///     }
///
///     async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
///         // Implementation specific to OKX
///         unimplemented!()
///     }
///
///     async fn fetch_funding_rates(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
///         // Implementation specific to OKX
///         unimplemented!()
///     }
///
///     async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
///         // Implementation specific to OKX
///         Ok(())
///     }
///
///     async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
///         // Implementation specific to OKX
///         Ok(())
///     }
///
///     async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()> {
///         // Implementation specific to OKX
///         Ok(())
///     }
///
///     async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
///         // Implementation specific to OKX
///         Ok(())
///     }
///
///     async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> Result<()> {
///         // Implementation specific to OKX
///         Ok(())
///     }
///
///     async fn subscribe_funding_rates(&mut self, symbols: &[Symbol]) -> Result<()> {
///         // Implementation specific to OKX
///         Ok(())
///     }
/// }
/// ```
///
/// # Error Handling
///
/// All methods return `Result<T>` where T is the expected return type.
/// Errors are wrapped in `ArbitrageError::Exchange` with descriptive messages.
#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    /// Returns the exchange identifier for this connector.
    ///
    /// # Returns
    ///
    /// The unique identifier of the exchange this connector interfaces with.
    fn id(&self) -> ExchangeId;

    /// Returns a reference to the connector configuration.
    ///
    /// # Returns
    ///
    /// Immutable reference to the connector's configuration.
    fn config(&self) -> &ConnectorConfig;

    /// Returns a reference to the connector base.
    ///
    /// # Returns
    ///
    /// Immutable reference to the shared connector base.
    fn base(&self) -> &ConnectorBase;

    /// Returns a mutable reference to the connector base.
    ///
    /// # Returns
    ///
    /// Mutable reference to the shared connector base.
    fn base_mut(&mut self) -> &mut ConnectorBase;

    /// Returns an event receiver for connection events.
    ///
    /// Allows consumers to subscribe to connection-related events
    /// such as connect, disconnect, and error events.
    ///
    /// # Returns
    ///
    /// A receiver for connection events.
    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent>;

    /// Fetches the current order book for a trading symbol.
    ///
    /// Retrieves the order book snapshot from the exchange, including
    /// bid and ask levels up to the configured depth.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol to fetch order book for
    ///
    /// # Returns
    ///
    /// The order book snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails or the exchange returns invalid data.
    async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook>;

    /// Fetches all available trading symbols from the exchange.
    ///
    /// Returns a list of all symbols that are available for trading
    /// on the connected exchange.
    ///
    /// # Returns
    ///
    /// A vector of available trading symbols.
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails.
    async fn fetch_symbols(&self) -> Result<Vec<Symbol>>;

    /// Fetches ticker data for the specified symbols.
    ///
    /// Retrieves 24-hour ticker statistics including price, volume,
    /// and change data for each requested symbol.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The symbols to fetch ticker data for
    ///
    /// # Returns
    ///
    /// A hashmap mapping symbols to their ticker data.
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails.
    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>>;

    /// Fetches funding rates for perpetual contracts.
    ///
    /// Retrieves current funding rates and next funding time for
    /// the specified perpetual contract symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The perpetual symbols to fetch funding rates for
    ///
    /// # Returns
    ///
    /// A hashmap mapping symbols to their funding rate data.
    ///
    /// # Errors
    ///
    /// Returns an error if the fetch fails.
    async fn fetch_funding_rates(&self, symbols: &[Symbol])
        -> Result<HashMap<Symbol, FundingRate>>;

    /// Establishes a connection to the exchange.
    ///
    /// Performs the necessary handshake and authentication to establish
    /// a connection. For WebSocket-based connectors, this typically
    /// involves the initial connection and authentication.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful connection.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    async fn connect(&mut self) -> Result<()> {
        self.base_mut()
            .connect()
            .await
            .map_err(|e| ArbitrageError::Exchange(format!("connect failed: {}", e)))
    }

    /// Closes the connection to the exchange.
    ///
    /// Performs graceful disconnection, including any necessary cleanup
    /// such as closing WebSocket connections or canceling subscriptions.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful disconnect.
    ///
    /// # Errors
    ///
    /// Returns an error if the disconnect fails.
    async fn disconnect(&mut self) -> Result<()> {
        self.base_mut().disconnect().await
    }

    /// Subscribes to real-time updates for the specified symbols.
    ///
    /// Registers interest in receiving updates for the given trading symbols.
    /// The exact update mechanism (WebSocket, polling, etc.) is connector-specific.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The symbols to subscribe to
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful subscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Unsubscribes from updates for the specified symbols.
    ///
    /// Removes interest in receiving updates for the given trading symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The symbols to unsubscribe from
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful unsubscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the unsubscription fails.
    async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribes to real-time ticker updates for the specified symbols.
    ///
    /// Registers interest in receiving 24-hour ticker statistics updates
    /// for the given symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The symbols to subscribe to
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful subscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribes to real-time order book updates for the specified symbols.
    ///
    /// Registers interest in receiving order book snapshots or deltas
    /// for the given symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The symbols to subscribe to
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful subscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribes to real-time trade updates for the specified symbols.
    ///
    /// Registers interest in receiving individual trade executions
    /// for the given symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The symbols to subscribe to
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful subscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Subscribes to real-time funding rate updates for the specified symbols.
    ///
    /// Registers interest in receiving funding rate updates for
    /// perpetual contract symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - The perpetual symbols to subscribe to
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful subscription.
    ///
    /// # Errors
    ///
    /// Returns an error if the subscription fails.
    async fn subscribe_funding_rates(&mut self, symbols: &[Symbol]) -> Result<()>;

    /// Performs a health check on the connector.
    ///
    /// Checks the connection status, last message time, and various
    /// health indicators to determine if the connector is functioning
    /// properly.
    ///
    /// # Returns
    ///
    /// A health status structure indicating the connector's health.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let health = connector.health_check().await?;
    /// if !health.is_connected {
    ///     println!("Connector is not connected!");
    /// }
    /// ```
    async fn health_check(&self) -> Result<HealthStatus> {
        let status = self.base().status.read().await.clone();
        let stats = self.base().stats();
        Ok(HealthStatus {
            is_connected: status == ConnectionStatus::Connected,
            last_message_time: Some(stats.last_update),
            websocket_status: status,
            rest_api_status: ConnectionStatus::Connected,
            error_count: stats.errors_count,
            reconnect_count: stats.reconnections,
        })
    }

    /// Returns the current connection status.
    ///
    /// # Returns
    ///
    /// The current connection status.
    fn status(&self) -> ConnectionStatus {
        match self.base().status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ConnectionStatus::Disconnected,
        }
    }

    /// Returns the connector statistics.
    ///
    /// # Returns
    ///
    /// Current connector statistics.
    fn get_stats(&self) -> ConnectorStats {
        self.base().get_stats()
    }

    /// Forces a reconnection to the exchange.
    ///
    /// Disconnects and then reconnects to the exchange. Useful for
    /// recovering from error states or refreshing connections.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful reconnection.
    ///
    /// # Errors
    ///
    /// Returns an error if either disconnect or connect fails.
    async fn force_reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    /// Places a new order on the exchange.
    ///
    /// # Arguments
    ///
    /// * `request` - The order request containing order details
    ///
    /// # Returns
    ///
    /// The order response from the exchange.
    ///
    /// # Errors
    ///
    /// Returns `NotImplemented` by default. Implementors should override
    /// this method to provide actual order placement.
    async fn place_order(&self, _request: &OrderRequest) -> Result<OrderResponse> {
        Err(ArbitrageError::NotImplemented(
            "place_order not implemented".to_string(),
        ))
    }

    /// Cancels an existing order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - The ID of the order to cancel
    /// * `symbol` - The symbol the order was for
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful cancellation.
    ///
    /// # Errors
    ///
    /// Returns `NotImplemented` by default.
    async fn cancel_order(&self, _order_id: &str, _symbol: &Symbol) -> Result<()> {
        Err(ArbitrageError::NotImplemented(
            "cancel_order not implemented".to_string(),
        ))
    }

    /// Gets the status of an existing order.
    ///
    /// # Arguments
    ///
    /// * `order_id` - The ID of the order to check
    /// * `symbol` - The symbol the order was for
    ///
    /// # Returns
    ///
    /// The current order status.
    ///
    /// # Errors
    ///
    /// Returns `NotImplemented` by default.
    async fn get_order_status(&self, _order_id: &str, _symbol: &Symbol) -> Result<OrderStatus> {
        Err(ArbitrageError::NotImplemented(
            "get_order_status not implemented".to_string(),
        ))
    }

    /// Gets the account balance.
    ///
    /// # Returns
    ///
    /// The current account balance across all assets.
    ///
    /// # Errors
    ///
    /// Returns `NotImplemented` by default.
    async fn get_balance(&self) -> Result<Balance> {
        Err(ArbitrageError::NotImplemented(
            "get_balance not implemented".to_string(),
        ))
    }

    /// Gets all open orders, optionally filtered by symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Optional symbol filter
    ///
    /// # Returns
    ///
    /// A vector of open order statuses.
    ///
    /// # Errors
    ///
    /// Returns `NotImplemented` by default.
    async fn get_open_orders(&self, _symbol: Option<&Symbol>) -> Result<Vec<OrderStatus>> {
        Err(ArbitrageError::NotImplemented(
            "get_open_orders not implemented".to_string(),
        ))
    }

    /// Parses an order book from nested JSON structure.
    ///
    /// Helper method for implementing connectors. Parses order book data
    /// from a nested JSON structure using specified JSON paths.
    ///
    /// # Arguments
    ///
    /// * `data` - The JSON data to parse
    /// * `symbol` - The symbol this order book is for
    /// * `bids_path` - JSON path segments to the bids array
    /// * `asks_path` - JSON path segments to the asks array
    ///
    /// # Returns
    ///
    /// The parsed order book.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let order_book = connector.parse_order_book(
    ///     &data,
    ///     &symbol,
    ///     &["data", "bids"],
    ///     &["data", "asks"],
    /// )?;
    /// ```
    fn parse_order_book(
        &self,
        data: &Value,
        symbol: &Symbol,
        bids_path: &[&str],
        asks_path: &[&str],
    ) -> Result<OrderBook> {
        let mut bids = Vec::new();
        let mut asks = Vec::new();

        let depth = self.config().order_book_depth as usize;

        for path_segment in bids_path {
            if let Some(obj) = data.pointer(path_segment) {
                if let Some(bids_array) = obj.as_array() {
                    for item in bids_array.iter().take(depth) {
                        if let Some(level) = self.parse_order_book_level(item) {
                            bids.push(level);
                        }
                    }
                }
                break;
            }
        }

        for path_segment in asks_path {
            if let Some(obj) = data.pointer(path_segment) {
                if let Some(asks_array) = obj.as_array() {
                    for item in asks_array.iter().take(depth) {
                        if let Some(level) = self.parse_order_book_level(item) {
                            asks.push(level);
                        }
                    }
                }
                break;
            }
        }

        Ok(OrderBook::new(self.id(), symbol.clone(), bids, asks))
    }

    /// Parses a single order book level from JSON.
    ///
    /// Helper method for implementing connectors. Handles various JSON
    /// formats for price and quantity fields.
    ///
    /// # Arguments
    ///
    /// * `data` - The JSON data for a single level
    ///
    /// # Returns
    ///
    /// Some(OrderBookLevel) if parsing succeeds, None otherwise.
    fn parse_order_book_level(&self, data: &Value) -> Option<OrderBookLevel> {
        let price = data
            .get("price")
            .or(data.get(0))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())?;

        let quantity = data
            .get("size")
            .or(data.get("quantity"))
            .or(data.get(1))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())?;

        Some(OrderBookLevel { price, quantity })
    }

    /// Parses an order book from flat JSON structure.
    ///
    /// Helper method for implementing connectors. Parses order book data
    /// from a flat JSON structure with specified keys for bids and asks.
    ///
    /// # Arguments
    ///
    /// * `data` - The JSON data to parse
    /// * `symbol` - The symbol this order book is for
    /// * `bids_key` - JSON key for the bids array
    /// * `asks_key` - JSON key for the asks array
    ///
    /// # Returns
    ///
    /// The parsed order book.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let order_book = connector.parse_order_book_from_array(
    ///     &data,
    ///     &symbol,
    ///     "bids",
    ///     "asks",
    /// )?;
    /// ```
    fn parse_order_book_from_array(
        &self,
        data: &Value,
        symbol: &Symbol,
        bids_key: &str,
        asks_key: &str,
    ) -> Result<OrderBook> {
        let empty_vec: Vec<Value> = Vec::new();
        let bids_array = data
            .get(bids_key)
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);
        let asks_array = data
            .get(asks_key)
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);

        let depth = self.config().order_book_depth as usize;
        let mut bids = Vec::with_capacity(depth);
        let mut asks = Vec::with_capacity(depth);

        for item in bids_array.iter().take(depth) {
            if let Some(level) = self.parse_order_book_level(item) {
                bids.push(level);
            }
        }

        for item in asks_array.iter().take(depth) {
            if let Some(level) = self.parse_order_book_level(item) {
                asks.push(level);
            }
        }

        Ok(OrderBook::new(self.id(), symbol.clone(), bids, asks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn create_test_connector_base() -> ConnectorBase {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::OKX,
            order_book_depth: 10,
            ..Default::default()
        };
        ConnectorBase::new(config)
    }

    fn create_test_value() -> Value {
        serde_json::json!({
            "bids": [
                {"price": "50000.00", "size": "1.5"},
                {"price": "49999.50", "size": "2.0"},
                {"price": "49999.00", "size": "0.5"}
            ],
            "asks": [
                {"price": "50001.00", "size": "1.0"},
                {"price": "50001.50", "size": "2.5"},
                {"price": "50002.00", "size": "1.0"}
            ]
        })
    }

    struct TestConnector {
        base: ConnectorBase,
    }

    #[async_trait]
    impl ExchangeConnector for TestConnector {
        fn id(&self) -> ExchangeId {
            ExchangeId::OKX
        }

        fn config(&self) -> &ConnectorConfig {
            &self.base.config
        }

        fn base(&self) -> &ConnectorBase {
            &self.base
        }

        fn base_mut(&mut self) -> &mut ConnectorBase {
            &mut self.base
        }

        fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
            self.base.event_sender.subscribe()
        }

        async fn fetch_order_book(&self, _symbol: &Symbol) -> Result<OrderBook> {
            unimplemented!()
        }

        async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
            unimplemented!()
        }

        async fn fetch_tickers(&self, _symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
            unimplemented!()
        }

        async fn fetch_funding_rates(
            &self,
            _symbols: &[Symbol],
        ) -> Result<HashMap<Symbol, FundingRate>> {
            unimplemented!()
        }

        async fn subscribe_symbols(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn unsubscribe_symbols(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_tickers(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_order_books(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_trades(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_funding_rates(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }
    }

    impl TestConnector {
        fn new() -> Self {
            Self {
                base: create_test_connector_base(),
            }
        }
    }

    #[tokio::test]
    async fn test_parse_order_book() {
        let connector = TestConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_test_value();

        let order_book = connector
            .parse_order_book_from_array(&data, &symbol, "bids", "asks")
            .unwrap();

        assert_eq!(order_book.exchange, ExchangeId::OKX);
        assert_eq!(order_book.symbol.base, "BTC");
        assert_eq!(order_book.symbol.quote, "USDT");
        assert_eq!(order_book.bids.len(), 3);
        assert_eq!(order_book.asks.len(), 3);

        assert_eq!(
            order_book.bids[0].price,
            Decimal::from_str("50000.00").unwrap()
        );
        assert_eq!(
            order_book.bids[0].quantity,
            Decimal::from_str("1.5").unwrap()
        );
    }

    #[tokio::test]
    async fn test_parse_order_book_with_depth_limit() {
        let mut base = create_test_connector_base();
        base.config.order_book_depth = 2;
        let connector = TestConnector { base };
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_test_value();

        let order_book = connector
            .parse_order_book_from_array(&data, &symbol, "bids", "asks")
            .unwrap();

        assert_eq!(order_book.bids.len(), 2);
        assert_eq!(order_book.asks.len(), 2);
    }

    #[tokio::test]
    async fn test_status() {
        let connector = TestConnector::new();
        let status = connector.status();
        assert_eq!(status, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let connector = TestConnector::new();
        let stats = connector.get_stats();
        assert_eq!(stats.exchange, ExchangeId::OKX);
    }
}
