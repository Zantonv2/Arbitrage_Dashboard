use crate::connector::{
    AssetBalance, Balance, CancelResponse, ConnectorConfig, ConnectorStats, ExchangeConnector,
    FundingRate, HealthStatus, OrderRequest, OrderResponse, OrderSide, OrderStatus,
    OrderStatusType, OrderType, TickerData,
};
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{format_symbol, parse_decimal, parse_symbol, ExponentialBackoff, SymbolFormat};
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

/// Gate.io WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GateioSubscription {
    method: String,
    params: Vec<String>,
    id: u64,
}

/// Gate.io WebSocket response message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct GateioWsResponse {
    method: Option<String>,
    params: Option<Value>,
    id: Option<u64>,
    error: Option<Value>,
}

pub struct GateioConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl GateioConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::GateIo,
            ws_url: "wss://api.gateio.ws/ws/v4/".to_string(),
            rest_url: "https://api.gateio.ws".to_string(),
            rate_limit_per_second: 10,
            rate_limit_burst: 20,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::GateIo;

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

    fn symbol_to_gateio(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Underscore)
    }

    fn symbol_from_gateio(&self, gateio_symbol: &str) -> Result<Symbol> {
        parse_symbol(gateio_symbol, SymbolFormat::Underscore)
    }
}

#[async_trait]
impl ExchangeConnector for GateioConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::GateIo
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
        let gateio_symbol = self.symbol_to_gateio(symbol);
        let url = format!(
            "{}/api/v4/spot/order_book?currency_pair={}&limit={}",
            self.config.rest_url, gateio_symbol, self.config.order_book_depth
        );

        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        self.parse_order_book(&data, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/api/v4/spot/currency_pairs", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(pairs) = data.as_array() {
            for pair in pairs {
                if let Some(id) = pair["id"].as_str() {
                    if let Ok(symbol) = self.symbol_from_gateio(id) {
                        symbols.push(symbol);
                    }
                }
            }
        }
        Ok(symbols)
    }

    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
        let url = format!("{}/api/v4/spot/tickers", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut tickers = HashMap::new();
        if let Some(ticker_array) = data.as_array() {
            for ticker_data in ticker_array {
                if let Some(currency_pair) = ticker_data["currency_pair"].as_str() {
                    if let Ok(symbol) = self.symbol_from_gateio(currency_pair) {
                        if symbols.contains(&symbol) {
                            if let Ok(ticker) = self.parse_ticker(ticker_data, &symbol) {
                                tickers.insert(symbol, ticker);
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
        _symbols: &[Symbol],
    ) -> Result<HashMap<Symbol, FundingRate>> {
        // Gate.io spot doesn't have funding rates
        Ok(HashMap::new())
    }

    async fn connect(&mut self) -> Result<()> {
        {
            let status = self.status.read().await;
            if *status == ConnectionStatus::Connected {
                return Ok(());
            }
        }

        info!("Connecting to Gate.io WebSocket: {}", self.config.ws_url);
        *self.status.write().await = ConnectionStatus::Connecting;

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

        *self.ws_handle.lock().await = Some(handle);
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Gate.io WebSocket");
        *self.status.write().await = ConnectionStatus::Disconnected;

        if let Some(handle) = self.ws_handle.lock().await.take() {
            handle.abort();
        }

        self.subscribed_symbols.write().await.clear();
        Ok(())
    }

    async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        let mut subscribed = self.subscribed_symbols.write().await;
        for symbol in symbols {
            if !subscribed.contains(symbol) {
                subscribed.push(symbol.clone());
            }
        }
        Ok(())
    }

    async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        let mut subscribed = self.subscribed_symbols.write().await;
        subscribed.retain(|s| !symbols.contains(s));
        Ok(())
    }

    async fn subscribe_tickers(&mut self, _symbols: &[Symbol]) -> Result<()> {
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!(
            "Subscribing to Gate.io order books for {} symbols",
            symbols.len()
        );
        self.subscribe_symbols(symbols).await?;
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

    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse> {
        let gateio_symbol = self.symbol_to_gateio(&order.symbol);
        let url = format!("{}/api/v4/spot/orders", self.config.rest_url);

        let side = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let order_type = match order.order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            _ => "limit",
        };

        let mut body = serde_json::json!({
            "currency_pair": gateio_symbol,
            "side": side,
            "type": order_type,
            "amount": order.quantity.to_string(),
        });

        if let Some(price) = order.price {
            body["price"] = serde_json::Value::String(price.to_string());
        }

        if let Some(client_id) = &order.client_order_id {
            body["text"] = serde_json::Value::String(client_id.clone());
        }

        let response = self.client.post(&url).json(&body).send().await?;

        let response_json: Value = response.json().await?;

        let order_id = response_json["id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let status = match response_json["status"].as_str() {
            Some("open") => OrderStatusType::New,
            Some("filled") => OrderStatusType::Filled,
            Some("cancelled") => OrderStatusType::Cancelled,
            _ => OrderStatusType::New,
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
    }

    async fn cancel_order(&self, order_id: &str) -> Result<CancelResponse> {
        let url = format!("{}/api/v4/spot/orders/{}", self.config.rest_url, order_id);

        let response = self.client.delete(&url).send().await?;

        let response_json: Value = response.json().await?;

        let status = match response_json["status"].as_str() {
            Some("cancelled") => OrderStatusType::Cancelled,
            _ => OrderStatusType::Rejected,
        };

        Ok(CancelResponse {
            order_id: order_id.to_string(),
            client_order_id: response_json["text"].as_str().map(|s| s.to_string()),
            status,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        let url = format!("{}/api/v4/spot/orders/{}", self.config.rest_url, order_id);

        let response = self.client.get(&url).send().await?;
        let order_data: Value = response.json().await?;

        let currency_pair = order_data["currency_pair"].as_str().unwrap_or("BTC_USDT");
        let order_symbol = self.symbol_from_gateio(currency_pair)?;

        let side = match order_data["side"].as_str() {
            Some("buy") => OrderSide::Buy,
            Some("sell") => OrderSide::Sell,
            _ => OrderSide::Buy,
        };

        let order_type = match order_data["type"].as_str() {
            Some("market") => OrderType::Market,
            Some("limit") => OrderType::Limit,
            _ => OrderType::Limit,
        };

        let quantity = order_data["amount"]
            .as_str()
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
            .unwrap_or_default();

        let price = order_data["price"]
            .as_str()
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

        let filled_quantity = order_data["filled_total"]
            .as_str()
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
            .unwrap_or_default();

        let status = match order_data["status"].as_str() {
            Some("open") => OrderStatusType::New,
            Some("filled") => OrderStatusType::Filled,
            Some("cancelled") => OrderStatusType::Cancelled,
            _ => OrderStatusType::New,
        };

        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: order_data["text"].as_str().map(|s| s.to_string()),
            symbol: order_symbol,
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
    }

    async fn get_balance(&self) -> Result<Balance> {
        let url = format!("{}/api/v4/spot/accounts", self.config.rest_url);

        let response = self.client.get(&url).send().await?;
        let accounts: Value = response.json().await?;

        let mut balances = HashMap::new();

        if let Some(account_array) = accounts.as_array() {
            for account in account_array {
                if let Some(currency) = account["currency"].as_str() {
                    let available = account["available"]
                        .as_str()
                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                        .unwrap_or_default();

                    let locked = account["locked"]
                        .as_str()
                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                        .unwrap_or_default();

                    balances.insert(
                        currency.to_string(),
                        AssetBalance {
                            asset: currency.to_string(),
                            free: available,
                            locked,
                            total: available + locked,
                        },
                    );
                }
            }
        }

        Ok(Balance {
            exchange: ExchangeId::GateIo,
            balances,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_open_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<OrderStatus>> {
        let mut url = format!("{}/api/v4/spot/orders?status=open", self.config.rest_url);

        if let Some(sym) = symbol {
            let gateio_symbol = self.symbol_to_gateio(sym);
            url.push_str(&format!("&currency_pair={}", gateio_symbol));
        }

        let response = self.client.get(&url).send().await?;
        let response_json: Value = response.json().await?;

        let mut orders = Vec::new();

        if let Some(order_array) = response_json.as_array() {
            for order_data in order_array {
                let order_id = order_data["id"].as_str().unwrap_or("unknown").to_string();

                let currency_pair = order_data["currency_pair"].as_str().unwrap_or("BTC_USDT");
                let order_symbol = self.symbol_from_gateio(currency_pair)?;

                let side = match order_data["side"].as_str() {
                    Some("buy") => OrderSide::Buy,
                    Some("sell") => OrderSide::Sell,
                    _ => OrderSide::Buy,
                };

                let order_type = match order_data["type"].as_str() {
                    Some("market") => OrderType::Market,
                    Some("limit") => OrderType::Limit,
                    _ => OrderType::Limit,
                };

                let quantity = order_data["amount"]
                    .as_str()
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                let price = order_data["price"]
                    .as_str()
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

                let filled_quantity = order_data["filled_total"]
                    .as_str()
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                orders.push(OrderStatus {
                    order_id,
                    client_order_id: order_data["text"].as_str().map(|s| s.to_string()),
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

impl GateioConnector {
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
                    info!("Gate.io WebSocket connected successfully");
                    backoff.reset();

                    *status.write().await = ConnectionStatus::Connected;

                    let _ = event_sender.send(ConnectionEvent::StatusChange {
                        exchange: ExchangeId::GateIo,
                        old_status: ConnectionStatus::Connecting,
                        new_status: ConnectionStatus::Connected,
                        timestamp: chrono::Utc::now(),
                    });

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
                        error!("Gate.io WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to Gate.io WebSocket: {}", e);
                    *status.write().await =
                        ConnectionStatus::Error("WebSocket connection failed".to_string());
                    let _ = event_sender.send(ConnectionEvent::Error {
                        exchange: ExchangeId::GateIo,
                        error: format!("WebSocket connection failed: {}", e),
                        timestamp: chrono::Utc::now(),
                    });
                }
            }

            let current_status = status.read().await;
            if *current_status == ConnectionStatus::Disconnected {
                break;
            }

            let delay = backoff.next_delay();
            warn!("Gate.io WebSocket reconnecting in {:?}", delay);
            tokio::time::sleep(delay).await;
        }
    }

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
        let mut subscription_id = 1u64;

        loop {
            if last_subscription_check.elapsed() > Duration::from_secs(5) {
                let symbols = subscribed_symbols.read().await;
                let mut new_params = Vec::new();

                for symbol in symbols.iter() {
                    let gateio_symbol = Self::symbol_to_gateio_static(symbol);

                    if !current_subscriptions.contains(&gateio_symbol) {
                        new_params.push(gateio_symbol.clone());
                        current_subscriptions.push(gateio_symbol);
                    }
                }

                if !new_params.is_empty() {
                    let subscription = GateioSubscription {
                        method: "depth.subscribe".to_string(),
                        params: new_params,
                        id: subscription_id,
                    };
                    subscription_id += 1;

                    let msg = serde_json::to_string(&subscription).map_err(|e| {
                        arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                            "Failed to serialize: {}",
                            e
                        ))
                    })?;

                    debug!("Sending Gate.io subscription: {}", msg);
                    ws_stream
                        .send(Message::Text(msg.into()))
                        .await
                        .map_err(|e| {
                            arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                                "Failed to send: {}",
                                e
                            ))
                        })?;
                }

                last_subscription_check = std::time::Instant::now();
            }

            match tokio::time::timeout(Duration::from_secs(30), ws_stream.next()).await {
                Ok(Some(Ok(message))) => {
                    {
                        let mut stats_guard = stats.lock().await;
                        stats_guard.messages_received += 1;
                        stats_guard.last_update = chrono::Utc::now();
                    }

                    match message {
                        Message::Text(text) => {
                            if let Err(e) = Self::handle_text_message(&text, event_sender).await {
                                warn!("Failed to handle Gate.io message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            ws_stream.send(Message::Pong(data)).await.map_err(|e| {
                                arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                                    "Failed to send pong: {}",
                                    e
                                ))
                            })?;
                        }
                        Message::Close(_) => {
                            info!("Gate.io WebSocket connection closed by server");
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("Gate.io WebSocket error: {}", e);
                    break;
                }
                Ok(None) => {
                    info!("Gate.io WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    warn!("Gate.io WebSocket timeout, sending ping");
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

            let current_status = status.read().await;
            if *current_status == ConnectionStatus::Disconnected {
                break;
            }
        }

        Ok(())
    }

    async fn handle_text_message(
        text: &str,
        event_sender: &broadcast::Sender<ConnectionEvent>,
    ) -> Result<()> {
        debug!("Received Gate.io message: {}", text);

        if let Ok(data) = serde_json::from_str::<Value>(text) {
            if let Some(method) = data["method"].as_str() {
                if method == "depth.update" {
                    if let Ok(order_book) = Self::parse_orderbook_message(&data) {
                        let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                            exchange: ExchangeId::GateIo,
                            order_book,
                            timestamp: chrono::Utc::now(),
                        });
                        let _ = event_sender.send(event);
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_orderbook_message(data: &Value) -> Result<OrderBook> {
        let params = data["params"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing params".to_string())
        })?;

        if params.len() < 3 {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(
                "Invalid params length".to_string(),
            ));
        }

        let currency_pair = params[2].as_str().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing currency pair".to_string())
        })?;

        let symbol = Self::symbol_from_gateio_static(currency_pair)?;

        let book_data = &params[1];
        let mut asks = Vec::new();
        let mut bids = Vec::new();

        if let Some(asks_data) = book_data["asks"].as_array() {
            for ask in asks_data.iter().take(50) {
                if let Some(ask_arr) = ask.as_array() {
                    if ask_arr.len() >= 2 {
                        let price = parse_decimal(&ask_arr[0])?;
                        let quantity = parse_decimal(&ask_arr[1])?;
                        asks.push(OrderBookLevel { price, quantity });
                    }
                }
            }
        }

        if let Some(bids_data) = book_data["bids"].as_array() {
            for bid in bids_data.iter().take(50) {
                if let Some(bid_arr) = bid.as_array() {
                    if bid_arr.len() >= 2 {
                        let price = parse_decimal(&bid_arr[0])?;
                        let quantity = parse_decimal(&bid_arr[1])?;
                        bids.push(OrderBookLevel { price, quantity });
                    }
                }
            }
        }

        Ok(OrderBook {
            exchange: ExchangeId::GateIo,
            symbol,
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    fn symbol_to_gateio_static(symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Underscore)
    }

    fn symbol_from_gateio_static(gateio_symbol: &str) -> Result<Symbol> {
        parse_symbol(gateio_symbol, SymbolFormat::Underscore)
    }

    fn parse_order_book(&self, data: &Value, symbol: &Symbol) -> Result<OrderBook> {
        let asks_data = data["asks"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string())
        })?;
        let bids_data = data["bids"].as_array().ok_or_else(|| {
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
            exchange: ExchangeId::GateIo,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    fn parse_ticker(&self, data: &Value, symbol: &Symbol) -> Result<TickerData> {
        let last_price = parse_decimal(&data["last"])?;
        let bid_price = parse_decimal(&data["highest_bid"])?;
        let ask_price = parse_decimal(&data["lowest_ask"])?;
        let volume_24h = parse_decimal(&data["base_volume"])?;
        let price_change_24h = parse_decimal(&data["change_percentage"])?;

        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::GateIo,
            last_price,
            bid_price,
            ask_price,
            volume_24h,
            price_change_24h,
            timestamp: chrono::Utc::now(),
        })
    }
}
