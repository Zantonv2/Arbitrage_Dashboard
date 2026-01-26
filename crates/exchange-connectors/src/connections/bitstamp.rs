use crate::connector::{
    AssetBalance, Balance, CancelResponse, ConnectorConfig, ConnectorStats, ExchangeConnector,
    FundingRate, HealthStatus, OrderRequest, OrderResponse, OrderSide, OrderStatus,
    OrderStatusType, OrderType, TickerData,
};
use crate::connector_trait::ConnectorBase;
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{parse_decimal, ExponentialBackoff};

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

/// Bitstamp WebSocket subscription message - used for order book subscriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitstampSubscription {
    event: String,
    data: BitstampSubscriptionData,
}

/// Bitstamp subscription data - used for order book subscriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitstampSubscriptionData {
    channel: String,
}

/// Bitstamp WebSocket response message - used for parsing responses
#[derive(Debug, Clone, Deserialize)]
struct BitstampWsResponse {
    event: Option<String>,
    channel: Option<String>,
    data: Option<Value>,
}

#[derive(Clone)]
pub struct BitstampConnector {
    pub base: ConnectorBase,
    client: Client,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
}

impl Default for BitstampConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl BitstampConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::Bitstamp,
            ws_url: "wss://ws.bitstamp.net".to_string(),
            rest_url: "https://www.bitstamp.net/api/v2".to_string(),
            rate_limit_per_second: 8,
            rate_limit_burst: 16,
            ..Default::default()
        };

        let base = ConnectorBase::new(config);
        let client = Client::new();

        Self {
            base,
            client,
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn symbol_to_bitstamp(&self, symbol: &Symbol) -> String {
        format!(
            "{}{}",
            symbol.base.to_lowercase(),
            symbol.quote.to_lowercase()
        )
    }

    pub fn symbol_from_bitstamp(&self, bitstamp_symbol: &str) -> Result<Symbol> {
        if let Some(base) = bitstamp_symbol.strip_suffix("usd") {
            Ok(arbitrage_core::types::Symbol::new(base, "USD"))
        } else if let Some(base) = bitstamp_symbol.strip_suffix("eur") {
            Ok(arbitrage_core::types::Symbol::new(base, "EUR"))
        } else if let Some(base) = bitstamp_symbol.strip_suffix("btc") {
            Ok(arbitrage_core::types::Symbol::new(base, "BTC"))
        } else {
            Ok(arbitrage_core::types::Symbol::new("BTC", "USD"))
        }
    }
}

#[async_trait]
impl ExchangeConnector for BitstampConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Bitstamp
    }

    fn status(&self) -> ConnectionStatus {
        match self.base.status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ConnectionStatus::Disconnected,
        }
    }

    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.base.event_sender.subscribe()
    }

    async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook> {
        let bitstamp_symbol = self.symbol_to_bitstamp(symbol);
        let url = format!(
            "{}/order_book/{}/",
            self.base.config.rest_url, bitstamp_symbol
        );

        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        self.parse_order_book(&data, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/trading-pairs-info/", self.base.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(pairs) = data.as_array() {
            for pair in pairs {
                if let Some(url_symbol) = pair["url_symbol"].as_str() {
                    if let Ok(symbol) = self.symbol_from_bitstamp(url_symbol) {
                        symbols.push(symbol);
                    }
                }
            }
        }
        Ok(symbols)
    }

    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
        let mut tickers = HashMap::new();
        for symbol in symbols {
            let bitstamp_symbol = self.symbol_to_bitstamp(symbol);
            let url = format!("{}/ticker/{}/", self.base.config.rest_url, bitstamp_symbol);

            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<Value>().await {
                    if let Ok(ticker) = self.parse_ticker(&data, symbol) {
                        tickers.insert(symbol.clone(), ticker);
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
        Ok(HashMap::new())
    }

    async fn connect(&mut self) -> Result<()> {
        self.base.connect().await
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.base.disconnect().await
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
            "Subscribing to Bitstamp order books for {} symbols",
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
        let status = self.status();
        let stats = self.get_stats();
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
        self.base.get_stats()
    }

    async fn force_reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    // === Trading Methods ===

    async fn place_order(&self, order: &OrderRequest) -> Result<OrderResponse> {
        let bitstamp_symbol = self.symbol_to_bitstamp(&order.symbol);
        let side = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let url = format!(
            "{}/v2/{}/{}/",
            self.base.config.rest_url, side, bitstamp_symbol
        );

        let mut body = serde_json::json!({
            "amount": order.quantity.to_string(),
        });

        match order.order_type {
            OrderType::Market => {}
            OrderType::Limit => {
                if let Some(price) = order.price {
                    body["price"] = serde_json::Value::String(price.to_string());
                }
            }
            _ => {
                if let Some(price) = order.price {
                    body["price"] = serde_json::Value::String(price.to_string());
                }
            }
        }

        if let Some(client_id) = &order.client_order_id {
            body["client_order_id"] = serde_json::Value::String(client_id.clone());
        }

        let response = self.client.post(&url).json(&body).send().await?;

        let response_json: Value = response.json().await?;

        let order_id = response_json["id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let status = match response_json["status"].as_str() {
            Some("Open") => OrderStatusType::New,
            Some("Finished") => OrderStatusType::Filled,
            Some("Canceled") => OrderStatusType::Cancelled,
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

    async fn cancel_order(&self, _symbol: &Symbol, order_id: &str) -> Result<CancelResponse> {
        let url = format!("{}/v2/cancel_order/", self.base.config.rest_url);

        let body = serde_json::json!({
            "id": order_id,
        });

        self.client.post(&url).json(&body).send().await?;

        Ok(CancelResponse {
            order_id: order_id.to_string(),
            client_order_id: None,
            status: OrderStatusType::Cancelled,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        let url = format!("{}/order_status/", self.base.config.rest_url);

        let body = serde_json::json!({
            "id": order_id,
        });

        let response = self.client.post(&url).json(&body).send().await?;
        let order_data: Value = response.json().await?;

        let order_symbol = if let Some(transactions) = order_data["transactions"].as_array() {
            if let Some(first_tx) = transactions.first() {
                if let Some(pair) = first_tx["pair"].as_str() {
                    self.symbol_from_bitstamp(pair)?
                } else {
                    arbitrage_core::types::Symbol::new("BTC", "USD")
                }
            } else {
                arbitrage_core::types::Symbol::new("BTC", "USD")
            }
        } else {
            arbitrage_core::types::Symbol::new("BTC", "USD")
        };

        let side = match order_data["type"].as_str() {
            Some("0") => OrderSide::Buy,
            Some("1") => OrderSide::Sell,
            _ => OrderSide::Buy,
        };

        let quantity = order_data["amount"]
            .as_str()
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
            .unwrap_or_default();

        let price = order_data["price"]
            .as_str()
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

        let filled_quantity = order_data["amount_remaining"]
            .as_str()
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
            .map(|remaining| quantity - remaining)
            .unwrap_or_default();

        let status = match order_data["status"].as_str() {
            Some("Open") => OrderStatusType::New,
            Some("Finished") => OrderStatusType::Filled,
            Some("Canceled") => OrderStatusType::Cancelled,
            _ => OrderStatusType::New,
        };

        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: order_data["client_order_id"]
                .as_str()
                .map(|s| s.to_string()),
            symbol: order_symbol,
            side,
            order_type: OrderType::Limit,
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
        let url = format!("{}/v2/balance/", self.base.config.rest_url);

        let response = self.client.post(&url).send().await?;
        let balance_data: Value = response.json().await?;

        let mut balances = HashMap::new();

        if let Some(balance_obj) = balance_data.as_object() {
            let mut assets: std::collections::HashSet<String> = std::collections::HashSet::new();

            for key in balance_obj.keys() {
                if let Some(asset) = key.strip_suffix("_balance") {
                    assets.insert(asset.to_uppercase());
                } else if let Some(asset) = key.strip_suffix("_available") {
                    assets.insert(asset.to_uppercase());
                } else if let Some(asset) = key.strip_suffix("_reserved") {
                    assets.insert(asset.to_uppercase());
                }
            }

            for asset in assets {
                let available_key = format!("{}_available", asset.to_lowercase());
                let reserved_key = format!("{}_reserved", asset.to_lowercase());

                let available = balance_obj
                    .get(&available_key)
                    .and_then(|v| v.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                let locked = balance_obj
                    .get(&reserved_key)
                    .and_then(|v| v.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                balances.insert(
                    asset.clone(),
                    AssetBalance {
                        asset: asset.clone(),
                        free: available,
                        locked,
                        total: available + locked,
                    },
                );
            }
        }

        Ok(Balance {
            exchange: ExchangeId::Bitstamp,
            balances,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_open_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<OrderStatus>> {
        let url = if let Some(sym) = symbol {
            let bitstamp_symbol = self.symbol_to_bitstamp(sym);
            format!(
                "{}/v2/open_orders/{}/",
                self.base.config.rest_url, bitstamp_symbol
            )
        } else {
            format!("{}/v2/open_orders/all/", self.base.config.rest_url)
        };

        let response = self.client.post(&url).send().await?;
        let response_json: Value = response.json().await?;

        let mut orders = Vec::new();

        if let Some(order_array) = response_json.as_array() {
            for order_data in order_array {
                let order_id = order_data["id"].as_str().unwrap_or("unknown").to_string();

                // Try to determine symbol from currency_pair or default
                let order_symbol = if let Some(pair) = order_data["currency_pair"].as_str() {
                    self.symbol_from_bitstamp(pair)?
                } else {
                    arbitrage_core::types::Symbol::new("BTC", "USD")
                };

                let side = match order_data["type"].as_str() {
                    Some("0") => OrderSide::Buy,
                    Some("1") => OrderSide::Sell,
                    _ => OrderSide::Buy,
                };

                let quantity = order_data["amount"]
                    .as_str()
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                let price = order_data["price"]
                    .as_str()
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

                let filled_quantity = order_data["amount_remaining"]
                    .as_str()
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .map(|remaining| quantity - remaining)
                    .unwrap_or_default();

                orders.push(OrderStatus {
                    order_id,
                    client_order_id: order_data["client_order_id"]
                        .as_str()
                        .map(|s| s.to_string()),
                    symbol: order_symbol,
                    side,
                    order_type: OrderType::Limit, // Bitstamp doesn't specify order type
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

impl BitstampConnector {
    // NOTE: WebSocket functionality is intentionally not implemented for Bitstamp
    // as it's not currently used in the system. The websocket_task implementation
    // was removed to reduce code duplication and dead code.

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

        loop {
            if last_subscription_check.elapsed() > Duration::from_secs(5) {
                let symbols = subscribed_symbols.read().await;

                for symbol in symbols.iter() {
                    let bitstamp_symbol = Self::symbol_to_bitstamp_static(symbol);
                    let channel = format!("order_book_{}", bitstamp_symbol);

                    if !current_subscriptions.contains(&channel) {
                        let subscription = BitstampSubscription {
                            event: "bts:subscribe".to_string(),
                            data: BitstampSubscriptionData {
                                channel: channel.clone(),
                            },
                        };

                        let msg = serde_json::to_string(&subscription).map_err(|e| {
                            arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                                "Failed to serialize: {}",
                                e
                            ))
                        })?;

                        debug!("Sending Bitstamp subscription: {}", msg);
                        ws_stream
                            .send(Message::Text(msg.into()))
                            .await
                            .map_err(|e| {
                                arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                                    "Failed to send: {}",
                                    e
                                ))
                            })?;

                        current_subscriptions.push(channel);
                    }
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
                                warn!("Failed to handle Bitstamp message: {}", e);
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
                            info!("Bitstamp WebSocket connection closed by server");
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("Bitstamp WebSocket error: {}", e);
                    break;
                }
                Ok(None) => {
                    info!("Bitstamp WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    warn!("Bitstamp WebSocket timeout, sending ping");
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
        debug!("Received Bitstamp message: {}", text);

        if let Ok(response) = serde_json::from_str::<BitstampWsResponse>(text) {
            if let Some(event) = &response.event {
                if event == "data" {
                    if let Some(channel) = &response.channel {
                        if channel.starts_with("order_book_") {
                            if let Ok(order_book) = Self::parse_orderbook_message(&response) {
                                let event =
                                    ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                                        exchange: ExchangeId::Bitstamp,
                                        order_book,
                                        timestamp: chrono::Utc::now(),
                                    });
                                let _ = event_sender.send(event);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_orderbook_message(response: &BitstampWsResponse) -> Result<OrderBook> {
        let channel = response.channel.as_ref().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing channel".to_string())
        })?;

        let bitstamp_symbol = channel.strip_prefix("order_book_").ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Invalid channel format".to_string())
        })?;

        let symbol = Self::symbol_from_bitstamp_static(bitstamp_symbol)?;

        let data = response.data.as_ref().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing data".to_string())
        })?;

        let mut asks = Vec::new();
        let mut bids = Vec::new();

        if let Some(asks_data) = data["asks"].as_array() {
            if asks_data.is_empty() {
                warn!("Bitstamp WebSocket received empty asks for {}", symbol);
            }
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

        if let Some(bids_data) = data["bids"].as_array() {
            if bids_data.is_empty() {
                warn!("Bitstamp WebSocket received empty bids for {}", symbol);
            }
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

        if asks.is_empty() || bids.is_empty() {
            warn!(
                "Bitstamp orderbook has empty side for {}: bids={}, asks={}",
                symbol,
                bids.len(),
                asks.len()
            );
        }

        Ok(OrderBook {
            exchange: ExchangeId::Bitstamp,
            symbol,
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    fn symbol_to_bitstamp_static(symbol: &Symbol) -> String {
        format!(
            "{}{}",
            symbol.base.to_lowercase(),
            symbol.quote.to_lowercase()
        )
    }

    fn symbol_from_bitstamp_static(bitstamp_symbol: &str) -> Result<Symbol> {
        if let Some(base) = bitstamp_symbol.strip_suffix("usd") {
            Ok(arbitrage_core::types::Symbol::new(base, "USD"))
        } else if let Some(base) = bitstamp_symbol.strip_suffix("eur") {
            Ok(arbitrage_core::types::Symbol::new(base, "EUR"))
        } else if let Some(base) = bitstamp_symbol.strip_suffix("btc") {
            Ok(arbitrage_core::types::Symbol::new(base, "BTC"))
        } else {
            Ok(arbitrage_core::types::Symbol::new("BTC", "USD"))
        }
    }

    pub fn parse_order_book(&self, data: &Value, symbol: &Symbol) -> Result<OrderBook> {
        let asks_data = data["asks"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string())
        })?;
        let bids_data = data["bids"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bids data".to_string())
        })?;

        if asks_data.is_empty() {
            warn!("Bitstamp REST API returned empty asks for {}", symbol);
        }
        if bids_data.is_empty() {
            warn!("Bitstamp REST API returned empty bids for {}", symbol);
        }

        let mut asks = Vec::new();
        for ask in asks_data
            .iter()
            .take(self.base.config.order_book_depth as usize)
        {
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price = parse_decimal(&ask_array[0])?;
                    let quantity = parse_decimal(&ask_array[1])?;
                    asks.push(OrderBookLevel { price, quantity });
                }
            }
        }

        let mut bids = Vec::new();
        for bid in bids_data
            .iter()
            .take(self.base.config.order_book_depth as usize)
        {
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price = parse_decimal(&bid_array[0])?;
                    let quantity = parse_decimal(&bid_array[1])?;
                    bids.push(OrderBookLevel { price, quantity });
                }
            }
        }

        if asks.is_empty() || bids.is_empty() {
            warn!(
                "Bitstamp orderbook parsing resulted in empty data for {}: bids={}, asks={}",
                symbol,
                bids.len(),
                asks.len()
            );
        }

        Ok(OrderBook {
            exchange: ExchangeId::Bitstamp,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    pub fn parse_ticker(&self, data: &Value, symbol: &Symbol) -> Result<TickerData> {
        let last_price = parse_decimal(&data["last"])?;
        let bid_price = parse_decimal(&data["bid"])?;
        let ask_price = parse_decimal(&data["ask"])?;
        let volume_24h = parse_decimal(&data["volume"])?;

        // Bitstamp doesn't provide 24h change directly, calculate from open if available
        let price_change_24h = if let Ok(open) = parse_decimal(&data["open"]) {
            last_price - open
        } else {
            rust_decimal::Decimal::ZERO
        };

        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::Bitstamp,
            last_price,
            bid_price,
            ask_price,
            volume_24h,
            price_change_24h,
            timestamp: chrono::Utc::now(),
        })
    }
}
