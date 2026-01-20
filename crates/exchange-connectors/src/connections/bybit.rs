use crate::connector::{
    AssetBalance, Balance, CancelResponse, ConnectorConfig, ConnectorStats, ExchangeConnector,
    FundingRate, HealthStatus, OrderRequest, OrderResponse, OrderSide, OrderStatus,
    OrderStatusType, OrderType, TickerData,
};
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{
    format_symbol, parse_decimal, parse_symbol, parse_timestamp, ExponentialBackoff, SymbolFormat,
};
use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};

/// ByBit WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BybitSubscription {
    op: String,
    args: Vec<String>,
}

/// ByBit WebSocket response message
#[derive(Debug, Clone, Deserialize)]
struct BybitWsResponse {
    success: Option<bool>,
    ret_msg: Option<String>,
    conn_id: Option<String>,
    op: Option<String>,
}

/// ByBit WebSocket market data message
#[derive(Debug, Clone, Deserialize)]
struct BybitMarketData {
    topic: String,
    #[serde(rename = "type")]
    data_type: String,
    ts: u64,
    data: Value,
    cts: Option<u64>,
}

#[derive(Clone)]
pub struct BybitConnector {
    pub config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl BybitConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::ByBit,
            ws_url: "wss://stream.bybit.com/v5/public/spot".to_string(),
            rest_url: "https://api.bybit.com".to_string(),
            rate_limit_per_second: 60,
            rate_limit_burst: 120,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::ByBit;

        Self {
            config,
            client,
            event_sender,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn symbol_to_bybit(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    pub fn symbol_from_bybit(&self, bybit_symbol: &str) -> Result<Symbol> {
        parse_symbol(bybit_symbol, SymbolFormat::NoSeparator)
    }
}

#[async_trait]
impl ExchangeConnector for BybitConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::ByBit
    }

    fn status(&self) -> ConnectionStatus {
        match self.status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ConnectionStatus::Disconnected,
        }
    }

    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_sender.subscribe()
    }

    async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook> {
        let bybit_symbol = self.symbol_to_bybit(symbol);
        let url = format!(
            "{}/v5/market/orderbook?category=spot&symbol={}&limit={}",
            self.config.rest_url, bybit_symbol, self.config.order_book_depth
        );

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        if let Some(result) = data["result"].as_object() {
            return self.parse_order_book(result, symbol);
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "No order book data".to_string(),
        ))
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!(
            "{}/v5/market/instruments-info?category=spot",
            self.config.rest_url
        );
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(result) = data["result"].as_object() {
            if let Some(list) = result["list"].as_array() {
                for item in list {
                    if let Some(symbol_str) = item["symbol"].as_str() {
                        if let Ok(symbol) = self.symbol_from_bybit(symbol_str) {
                            symbols.push(symbol);
                        }
                    }
                }
            }
        }
        Ok(symbols)
    }

    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
        let mut tickers = HashMap::new();
        for symbol in symbols {
            let bybit_symbol = self.symbol_to_bybit(symbol);
            let url = format!(
                "{}/v5/market/tickers?category=spot&symbol={}",
                self.config.rest_url, bybit_symbol
            );

            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(result) = data["result"].as_object() {
                        if let Some(list) = result["list"].as_array() {
                            if let Some(ticker_data) = list.first() {
                                if let Ok(ticker) = self.parse_ticker(ticker_data) {
                                    tickers.insert(symbol.clone(), ticker);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(tickers)
    }

    async fn fetch_funding_rates(
        &self,
        symbols: &[Symbol],
    ) -> Result<HashMap<Symbol, FundingRate>> {
        let mut funding_rates = HashMap::new();
        for symbol in symbols {
            let bybit_symbol = self.symbol_to_bybit(symbol);
            let url = format!(
                "{}/v5/market/funding/history?category=linear&symbol={}&limit=1",
                self.config.rest_url, bybit_symbol
            );

            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(result) = data["result"].as_object() {
                        if let Some(list) = result["list"].as_array() {
                            if let Some(funding_data) = list.first() {
                                if let Ok(funding_rate) =
                                    self.parse_funding_rate(funding_data, symbol)
                                {
                                    funding_rates.insert(symbol.clone(), funding_rate);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(funding_rates)
    }

    async fn connect(&mut self) -> Result<()> {
        // Check if already connected
        {
            let status = self.status.read().await;
            if *status == ConnectionStatus::Connected {
                return Ok(());
            }
        }

        info!("Connecting to ByBit WebSocket: {}", self.config.ws_url);

        // Update status to connecting
        *self.status.write().await = ConnectionStatus::Connecting;

        // Start WebSocket connection task
        let ws_url = self.config.ws_url.clone();
        let event_sender = self.event_sender.clone();
        let status = self.status.clone();
        let stats = self.stats.clone();
        let subscribed_symbols = self.subscribed_symbols.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            Self::websocket_task(
                ws_url,
                event_sender,
                status,
                stats,
                subscribed_symbols,
                config,
            )
            .await;
        });

        // Store the handle
        *self.ws_handle.lock().await = Some(handle);

        // Wait for connection to establish (increased from 100ms for reliability)
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from ByBit WebSocket");

        // Update status
        *self.status.write().await = ConnectionStatus::Disconnected;

        // Cancel WebSocket task if running
        if let Some(handle) = self.ws_handle.lock().await.take() {
            handle.abort();
        }

        // Clear subscribed symbols
        self.subscribed_symbols.write().await.clear();

        Ok(())
    }

    async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        // Store symbols for reconnection
        let mut subscribed = self.subscribed_symbols.write().await;
        for symbol in symbols {
            if !subscribed.contains(symbol) {
                subscribed.push(symbol.clone());
            }
        }
        Ok(())
    }

    async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        // Remove symbols from subscription list
        let mut subscribed = self.subscribed_symbols.write().await;
        subscribed.retain(|s| !symbols.contains(s));
        Ok(())
    }

    async fn subscribe_tickers(&mut self, _symbols: &[Symbol]) -> Result<()> {
        // ByBit tickers are included in order book subscriptions
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!(
            "Subscribing to ByBit order books for {} symbols",
            symbols.len()
        );

        // Add symbols to subscription list
        self.subscribe_symbols(symbols).await?;

        // Send subscription message if connected
        let status = self.status.read().await;
        if *status == ConnectionStatus::Connected {
            // The actual subscription will be handled by the WebSocket task
            // when it detects new symbols in the subscribed_symbols list
        }

        Ok(())
    }

    async fn subscribe_trades(&mut self, _symbols: &[Symbol]) -> Result<()> {
        Ok(())
    }

    async fn subscribe_funding_rates(&mut self, _symbols: &[Symbol]) -> Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let status = self.status.read().await.clone();
        let stats = self.stats.lock().await;

        Ok(HealthStatus {
            is_connected: status == ConnectionStatus::Connected,
            last_message_time: Some(stats.last_update),
            websocket_status: status,
            rest_api_status: ConnectionStatus::Connected,
            error_count: stats.errors_count,
            reconnect_count: stats.reconnections,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        match self.stats.try_lock() {
            Ok(stats) => stats.clone(),
            Err(_) => ConnectorStats::default(),
        }
    }

    async fn force_reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    // === Trading Methods ===

    async fn place_order(
        &self,
        order: &crate::connector::OrderRequest,
    ) -> Result<crate::connector::OrderResponse> {
        use crate::connector::{OrderResponse, OrderStatusType};

        // ByBit API endpoint for placing orders
        let url = format!("{}/v5/order/create", self.config.rest_url);

        // Convert our order request to ByBit format
        let bybit_order = serde_json::json!({
            "category": "spot", // Trading category: spot for spot trading
            "symbol": format!("{}{}", order.symbol.base, order.symbol.quote),
            "side": match order.side {
                crate::connector::OrderSide::Buy => "Buy",
                crate::connector::OrderSide::Sell => "Sell",
            },
            "orderType": match order.order_type {
                crate::connector::OrderType::Market => "Market",
                crate::connector::OrderType::Limit => "Limit",
                _ => "Limit", // Default to limit for other types
            },
            "qty": order.quantity.to_string(),
            "price": order.price.map(|p| p.to_string()).unwrap_or_default(),
            "orderLinkId": order.client_order_id.as_deref().unwrap_or(""),
        });

        // Make authenticated request to ByBit
        let response = self
            .client
            .post(&url)
            .json(&bybit_order)
            .send()
            .await
            .map_err(|e| {
                arbitrage_core::ArbitrageError::Network(format!(
                    "ByBit place order request failed: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "ByBit place order failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        // Parse ByBit response
        if let Some(result) = response_json.get("result") {
            let order_id = result
                .get("orderId")
                .and_then(|id| id.as_str())
                .unwrap_or("unknown")
                .to_string();

            let status = match response_json.get("retCode").and_then(|c| c.as_u64()) {
                Some(0) => OrderStatusType::New, // Success
                _ => OrderStatusType::Rejected,
            };

            Ok(OrderResponse {
                order_id,
                client_order_id: order.client_order_id.clone(),
                symbol: order.symbol.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                quantity: order.quantity,
                price: order.price,
                status,
                timestamp: chrono::Utc::now(),
            })
        } else {
            Err(arbitrage_core::ArbitrageError::ExchangeConnection(
                "Invalid response format from ByBit".to_string(),
            ))
        }
    }

    async fn cancel_order(&self, order_id: &str) -> Result<crate::connector::CancelResponse> {
        use crate::connector::{CancelResponse, OrderStatusType};

        let url = format!("{}/v5/order/cancel", self.config.rest_url);

        let cancel_request = serde_json::json!({
            "category": "spot",
            "symbol": "BTCUSDT", // This should be dynamic based on the order
            "orderId": order_id,
        });

        let response = self
            .client
            .post(&url)
            .json(&cancel_request)
            .send()
            .await
            .map_err(|e| {
                arbitrage_core::ArbitrageError::Network(format!(
                    "ByBit cancel order request failed: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "ByBit cancel order failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let status = if response_json.get("retCode").and_then(|c| c.as_u64()) == Some(0) {
            OrderStatusType::Cancelled
        } else {
            OrderStatusType::Rejected
        };

        Ok(CancelResponse {
            order_id: order_id.to_string(),
            client_order_id: None,
            status,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_order_status(&self, order_id: &str) -> Result<crate::connector::OrderStatus> {
        use crate::connector::{OrderSide, OrderStatus, OrderStatusType, OrderType};

        let url = format!(
            "{}/v5/order/realtime?category=spot&orderId={}",
            self.config.rest_url, order_id
        );

        let response = self.client.get(&url).send().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!(
                "ByBit get order status request failed: {}",
                e
            ))
        })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "ByBit get order status failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        if let Some(result) = response_json
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .and_then(|arr| arr.first())
        {
            let symbol_str = result
                .get("symbol")
                .and_then(|s| s.as_str())
                .unwrap_or("BTCUSDT");
            // ByBit uses concatenated symbols like BTCUSDT, need to parse
            let symbol = if symbol_str.ends_with("USDT") {
                let base = &symbol_str[..symbol_str.len() - 4];
                arbitrage_core::types::Symbol::new(base, "USDT")
            } else {
                arbitrage_core::types::Symbol::new("BTC", "USDT")
            };

            let side = match result.get("side").and_then(|s| s.as_str()) {
                Some("Buy") => OrderSide::Buy,
                Some("Sell") => OrderSide::Sell,
                _ => OrderSide::Buy,
            };

            let order_type = match result.get("orderType").and_then(|t| t.as_str()) {
                Some("Market") => OrderType::Market,
                Some("Limit") => OrderType::Limit,
                _ => OrderType::Limit,
            };

            let quantity = result
                .get("qty")
                .and_then(|q| q.as_str())
                .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                .unwrap_or_default();

            let price = result
                .get("price")
                .and_then(|p| p.as_str())
                .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

            let filled_quantity = result
                .get("cumExecQty")
                .and_then(|f| f.as_str())
                .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                .unwrap_or_default();

            let status = match result.get("orderStatus").and_then(|s| s.as_str()) {
                Some("New") => OrderStatusType::New,
                Some("PartiallyFilled") => OrderStatusType::PartiallyFilled,
                Some("Filled") => OrderStatusType::Filled,
                Some("Cancelled") => OrderStatusType::Cancelled,
                _ => OrderStatusType::New,
            };

            Ok(OrderStatus {
                order_id: order_id.to_string(),
                client_order_id: result
                    .get("orderLinkId")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string()),
                symbol,
                side,
                order_type,
                quantity,
                price,
                filled_quantity,
                remaining_quantity: quantity - filled_quantity,
                average_price: price,
                status,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
        } else {
            Err(arbitrage_core::ArbitrageError::ExchangeConnection(
                "Invalid response format from ByBit".to_string(),
            ))
        }
    }

    async fn get_balance(&self) -> Result<crate::connector::Balance> {
        use crate::connector::{AssetBalance, Balance};

        let url = format!(
            "{}/v5/account/wallet-balance?accountType=SPOT",
            self.config.rest_url
        );

        let response = self.client.get(&url).send().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!(
                "ByBit get balance request failed: {}",
                e
            ))
        })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "ByBit get balance failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let mut balances = std::collections::HashMap::new();

        if let Some(result) = response_json
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .and_then(|arr| arr.first())
        {
            if let Some(coins) = result.get("coin").and_then(|c| c.as_array()) {
                for coin in coins {
                    if let Some(currency) = coin.get("coin").and_then(|c| c.as_str()) {
                        let available = coin
                            .get("walletBalance")
                            .and_then(|a| a.as_str())
                            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                            .unwrap_or_default();

                        let locked = coin
                            .get("locked")
                            .and_then(|f| f.as_str())
                            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                            .unwrap_or_default();

                        balances.insert(
                            currency.to_string(),
                            AssetBalance {
                                asset: currency.to_string(),
                                free: available - locked,
                                locked,
                                total: available,
                            },
                        );
                    }
                }
            }
        }

        Ok(Balance {
            exchange: ExchangeId::ByBit,
            balances,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_open_orders(
        &self,
        symbol: Option<&Symbol>,
    ) -> Result<Vec<crate::connector::OrderStatus>> {
        use crate::connector::{OrderSide, OrderStatus, OrderStatusType, OrderType};

        let mut url = format!("{}/v5/order/realtime?category=spot", self.config.rest_url);

        if let Some(sym) = symbol {
            url.push_str(&format!("&symbol={}{}", sym.base, sym.quote));
        }

        let response = self.client.get(&url).send().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!(
                "ByBit get open orders request failed: {}",
                e
            ))
        })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "ByBit get open orders failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let mut orders = Vec::new();

        if let Some(list) = response_json
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
        {
            for order_data in list {
                let order_id = order_data
                    .get("orderId")
                    .and_then(|id| id.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let symbol_str = order_data
                    .get("symbol")
                    .and_then(|s| s.as_str())
                    .unwrap_or("BTCUSDT");
                let order_symbol = if symbol_str.ends_with("USDT") {
                    let base = &symbol_str[..symbol_str.len() - 4];
                    arbitrage_core::types::Symbol::new(base, "USDT")
                } else {
                    arbitrage_core::types::Symbol::new("BTC", "USDT")
                };

                let side = match order_data.get("side").and_then(|s| s.as_str()) {
                    Some("Buy") => OrderSide::Buy,
                    Some("Sell") => OrderSide::Sell,
                    _ => OrderSide::Buy,
                };

                let order_type = match order_data.get("orderType").and_then(|t| t.as_str()) {
                    Some("Market") => OrderType::Market,
                    Some("Limit") => OrderType::Limit,
                    _ => OrderType::Limit,
                };

                let quantity = order_data
                    .get("qty")
                    .and_then(|q| q.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                let price = order_data
                    .get("price")
                    .and_then(|p| p.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

                let filled_quantity = order_data
                    .get("cumExecQty")
                    .and_then(|f| f.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                orders.push(OrderStatus {
                    order_id,
                    client_order_id: order_data
                        .get("orderLinkId")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string()),
                    symbol: order_symbol,
                    side,
                    order_type,
                    quantity,
                    price,
                    filled_quantity,
                    remaining_quantity: quantity - filled_quantity,
                    average_price: price,
                    status: OrderStatusType::New, // Open orders are "New"
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }

        Ok(orders)
    }
}

impl BybitConnector {
    /// Main WebSocket connection task with reconnection logic
    async fn websocket_task(
        ws_url: String,
        event_sender: broadcast::Sender<ConnectionEvent>,
        status: Arc<RwLock<ConnectionStatus>>,
        stats: Arc<Mutex<ConnectorStats>>,
        subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
        config: ConnectorConfig,
    ) {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_millis(1000), Duration::from_millis(30000));

        loop {
            match Self::connect_websocket(&ws_url).await {
                Ok((ws_stream, _)) => {
                    info!("ByBit WebSocket connected successfully");
                    backoff.reset();

                    // Update status to connected
                    *status.write().await = ConnectionStatus::Connected;

                    // Send status change event
                    let _ = event_sender.send(ConnectionEvent::StatusChange {
                        exchange: ExchangeId::ByBit,
                        old_status: ConnectionStatus::Connecting,
                        new_status: ConnectionStatus::Connected,
                        timestamp: chrono::Utc::now(),
                    });

                    // Handle WebSocket messages
                    if let Err(e) = Self::handle_websocket_connection(
                        ws_stream,
                        &event_sender,
                        &status,
                        &stats,
                        &subscribed_symbols,
                        &config,
                    )
                    .await
                    {
                        error!("ByBit WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to ByBit WebSocket: {}", e);

                    // Update status to error
                    *status.write().await =
                        ConnectionStatus::Error("WebSocket connection failed".to_string());

                    // Send error event
                    let _ = event_sender.send(ConnectionEvent::Error {
                        exchange: ExchangeId::ByBit,
                        error: format!("WebSocket connection failed: {}", e),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }

            // Check if we should continue reconnecting
            let current_status = status.read().await;
            if *current_status == ConnectionStatus::Disconnected {
                break;
            }

            // Wait before reconnecting
            let delay = backoff.next_delay();
            warn!("ByBit WebSocket reconnecting in {:?}", delay);
            tokio::time::sleep(delay).await;
        }
    }

    /// Establish WebSocket connection
    async fn connect_websocket(
        ws_url: &str,
    ) -> Result<(
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::http::Response<Option<Vec<u8>>>,
    )> {
        let (ws_stream, response) = connect_async(ws_url).await.map_err(|e| {
            arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "WebSocket connection failed: {}",
                e
            ))
        })?;

        Ok((ws_stream, response))
    }

    /// Handle WebSocket connection and messages
    async fn handle_websocket_connection(
        mut ws_stream: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        event_sender: &broadcast::Sender<ConnectionEvent>,
        status: &Arc<RwLock<ConnectionStatus>>,
        stats: &Arc<Mutex<ConnectorStats>>,
        subscribed_symbols: &Arc<RwLock<Vec<Symbol>>>,
        _config: &ConnectorConfig,
    ) -> Result<()> {
        let mut last_subscription_check = std::time::Instant::now();
        let mut current_subscriptions: Vec<String> = Vec::new();

        loop {
            // Check for new subscriptions every 5 seconds
            if last_subscription_check.elapsed() > Duration::from_secs(5) {
                let symbols = subscribed_symbols.read().await;
                let mut new_topics = Vec::new();

                for symbol in symbols.iter() {
                    let bybit_symbol = Self::symbol_to_bybit_static(symbol);
                    let topic = format!("orderbook.1.{}", bybit_symbol);

                    if !current_subscriptions.contains(&topic) {
                        new_topics.push(topic.clone());
                        current_subscriptions.push(topic);
                    }
                }

                if !new_topics.is_empty() {
                    let subscription = BybitSubscription {
                        op: "subscribe".to_string(),
                        args: new_topics.clone(),
                    };

                    let msg = serde_json::to_string(&subscription).map_err(|e| {
                        arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                            "Failed to serialize subscription: {}",
                            e
                        ))
                    })?;

                    debug!("Sending ByBit subscription: {}", msg);

                    ws_stream
                        .send(Message::Text(msg.into()))
                        .await
                        .map_err(|e| {
                            arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                                "Failed to send subscription: {}",
                                e
                            ))
                        })?;
                }

                last_subscription_check = std::time::Instant::now();
            }

            // Handle incoming messages with timeout
            match tokio::time::timeout(Duration::from_secs(30), ws_stream.next()).await {
                Ok(Some(Ok(message))) => {
                    // Update stats
                    {
                        let mut stats_guard = stats.lock().await;
                        stats_guard.messages_received += 1;
                        stats_guard.last_update = chrono::Utc::now();
                    }

                    match message {
                        Message::Text(text) => {
                            if let Err(e) = Self::handle_text_message(&text, event_sender).await {
                                warn!("Failed to handle ByBit message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            // Respond to ping with pong
                            ws_stream.send(Message::Pong(data)).await.map_err(|e| {
                                arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                                    "Failed to send pong: {}",
                                    e
                                ))
                            })?;
                        }
                        Message::Close(_) => {
                            info!("ByBit WebSocket connection closed by server");
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("ByBit WebSocket error: {}", e);
                    break;
                }
                Ok(None) => {
                    info!("ByBit WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    warn!("ByBit WebSocket timeout, sending ping");
                    // Send ping to keep connection alive
                    ws_stream
                        .send(Message::Ping(vec![].into()))
                        .await
                        .map_err(|e| {
                            arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                                "Failed to send ping: {}",
                                e
                            ))
                        })?;
                }
            }

            // Check if we should disconnect
            let current_status = status.read().await;
            if *current_status == ConnectionStatus::Disconnected {
                break;
            }
        }

        Ok(())
    }

    /// Handle incoming text messages
    async fn handle_text_message(
        text: &str,
        event_sender: &broadcast::Sender<ConnectionEvent>,
    ) -> Result<()> {
        debug!("Received ByBit message: {}", text);

        // Try to parse as response first
        if let Ok(response) = serde_json::from_str::<BybitWsResponse>(text) {
            if let Some(success) = response.success {
                if success {
                    debug!("ByBit subscription successful: {:?}", response);
                } else {
                    warn!("ByBit subscription failed: {:?}", response);
                }
            }
            return Ok(());
        }

        // Try to parse as market data
        if let Ok(market_data) = serde_json::from_str::<BybitMarketData>(text) {
            if market_data.topic.starts_with("orderbook.") {
                if let Ok(order_book) = Self::parse_orderbook_message(&market_data) {
                    let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                        exchange: ExchangeId::ByBit,
                        order_book,
                        timestamp: chrono::Utc::now(),
                    });

                    let _ = event_sender.send(event);
                }
            }
        }

        Ok(())
    }

    /// Parse ByBit orderbook message into OrderBook
    fn parse_orderbook_message(market_data: &BybitMarketData) -> Result<OrderBook> {
        let data = &market_data.data;

        // Extract symbol from topic (e.g., "orderbook.50.BTCUSDT" -> "BTCUSDT")
        let topic_parts: Vec<&str> = market_data.topic.split('.').collect();
        if topic_parts.len() < 3 {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(
                "Invalid topic format".to_string(),
            ));
        }
        let bybit_symbol = topic_parts[2];
        let symbol = Self::symbol_from_bybit_static(bybit_symbol)?;

        let asks_data = data["a"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string())
        })?;
        let bids_data = data["b"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bids data".to_string())
        })?;

        let mut asks = Vec::new();
        for ask in asks_data.iter().take(50) {
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price = parse_decimal(&ask_array[0])?;
                    let quantity = parse_decimal(&ask_array[1])?;
                    asks.push(OrderBookLevel { price, quantity });
                }
            }
        }

        let mut bids = Vec::new();
        for bid in bids_data.iter().take(50) {
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price = parse_decimal(&bid_array[0])?;
                    let quantity = parse_decimal(&bid_array[1])?;
                    bids.push(OrderBookLevel { price, quantity });
                }
            }
        }

        let timestamp = if let Some(cts) = market_data.cts {
            chrono::DateTime::from_timestamp_millis(cts as i64)
                .unwrap_or_else(|| chrono::Utc::now())
        } else {
            chrono::DateTime::from_timestamp_millis(market_data.ts as i64)
                .unwrap_or_else(|| chrono::Utc::now())
        };

        Ok(OrderBook {
            exchange: ExchangeId::ByBit,
            symbol,
            bids,
            asks,
            timestamp,
            sequence: data["u"].as_u64(),
        })
    }

    /// Static version of symbol conversion for use in async contexts
    fn symbol_to_bybit_static(symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    /// Static version of symbol parsing for use in async contexts
    fn symbol_from_bybit_static(bybit_symbol: &str) -> Result<Symbol> {
        parse_symbol(bybit_symbol, SymbolFormat::NoSeparator)
    }

    pub fn parse_order_book(
        &self,
        data: &serde_json::Map<String, serde_json::Value>,
        symbol: &Symbol,
    ) -> Result<OrderBook> {
        let asks_data = data["a"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string())
        })?;
        let bids_data = data["b"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bids data".to_string())
        })?;

        let mut asks = Vec::new();
        for ask in asks_data.iter().take(self.config.order_book_depth as usize) {
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price = parse_decimal(&ask_array[0])?;
                    let quantity = parse_decimal(&ask_array[1])?;
                    asks.push(OrderBookLevel { price, quantity });
                }
            }
        }

        let mut bids = Vec::new();
        for bid in bids_data.iter().take(self.config.order_book_depth as usize) {
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price = parse_decimal(&bid_array[0])?;
                    let quantity = parse_decimal(&bid_array[1])?;
                    bids.push(OrderBookLevel { price, quantity });
                }
            }
        }

        Ok(OrderBook {
            exchange: ExchangeId::ByBit,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    pub fn parse_ticker(&self, data: &serde_json::Value) -> Result<TickerData> {
        let symbol_str = data["symbol"].as_str().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing symbol".to_string())
        })?;
        let symbol = self.symbol_from_bybit(symbol_str)?;

        Ok(TickerData {
            symbol,
            exchange: ExchangeId::ByBit,
            last_price: parse_decimal(&data["lastPrice"])?,
            bid_price: parse_decimal(&data["bid1Price"])?,
            ask_price: parse_decimal(&data["ask1Price"])?,
            volume_24h: parse_decimal(&data["volume24h"]).unwrap_or_default(),
            price_change_24h: parse_decimal(&data["price24hPcnt"]).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }

    pub fn parse_funding_rate(
        &self,
        data: &serde_json::Value,
        symbol: &Symbol,
    ) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["fundingRate"])?;
        let funding_time = parse_timestamp(&data["fundingRateTimestamp"])?;

        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::ByBit,
            funding_rate,
            predicted_rate: None,
            funding_time,
            timestamp: chrono::Utc::now(),
        })
    }
}
