use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{format_symbol, parse_symbol, parse_decimal, SymbolFormat, ExponentialBackoff};
use arbitrage_core::{types::{ExchangeId, Symbol, OrderBook, OrderBookLevel, ConnectionStatus}, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn, error, debug};

/// Kraken WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KrakenSubscription {
    event: String,
    pair: Vec<String>,
    subscription: KrakenSubscriptionDetails,
}

/// Kraken subscription details
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KrakenSubscriptionDetails {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<u32>,
}

/// Kraken WebSocket response message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct KrakenWsResponse {
    event: Option<String>,
    status: Option<String>,
    #[serde(rename = "errorMessage")]
    error_message: Option<String>,
    #[serde(rename = "channelID")]
    channel_id: Option<u64>,
    #[serde(rename = "channelName")]
    channel_name: Option<String>,
    pair: Option<String>,
}

pub struct KrakenConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}


impl KrakenConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::Kraken,
            ws_url: "wss://ws.kraken.com".to_string(),
            rest_url: "https://api.kraken.com".to_string(),
            rate_limit_per_second: 15,
            rate_limit_burst: 30,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::Kraken;

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

    fn symbol_to_kraken(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Slash)
    }

    fn symbol_from_kraken(&self, kraken_symbol: &str) -> Result<Symbol> {
        parse_symbol(kraken_symbol, SymbolFormat::Slash)
    }
}

#[async_trait]
impl ExchangeConnector for KrakenConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Kraken
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
        let kraken_symbol = self.symbol_to_kraken(symbol);
        let url = format!("{}/0/public/Depth?pair={}&count={}", 
                         self.config.rest_url, kraken_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        if let Some(result) = data["result"].as_object() {
            // Kraken returns data under the pair name key
            for (_key, book_data) in result {
                return self.parse_order_book(book_data, symbol);
            }
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection("No order book data".to_string()))
    }


    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/0/public/AssetPairs", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(result) = data["result"].as_object() {
            for (pair_name, pair_data) in result {
                // Skip dark pool pairs
                if pair_name.ends_with(".d") {
                    continue;
                }
                if let Some(wsname) = pair_data["wsname"].as_str() {
                    if let Ok(symbol) = self.symbol_from_kraken(wsname) {
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
            let kraken_symbol = self.symbol_to_kraken(symbol);
            let url = format!("{}/0/public/Ticker?pair={}", self.config.rest_url, kraken_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<Value>().await {
                    if let Some(result) = data["result"].as_object() {
                        for (_key, ticker_data) in result {
                            if let Ok(ticker) = self.parse_ticker(ticker_data, symbol) {
                                tickers.insert(symbol.clone(), ticker);
                            }
                        }
                    }
                }
            }
        }
        Ok(tickers)
    }

    async fn fetch_funding_rates(&self, _symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        // Kraken spot doesn't have funding rates, only futures
        Ok(HashMap::new())
    }

    async fn connect(&mut self) -> Result<()> {
        // Check if already connected
        {
            let status = self.status.read().await;
            if *status == ConnectionStatus::Connected {
                return Ok(());
            }
        }

        info!("Connecting to Kraken WebSocket: {}", self.config.ws_url);
        
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
            Self::websocket_task(ws_url, event_sender, status, stats, subscribed_symbols, config).await;
        });

        // Store the handle
        *self.ws_handle.lock().await = Some(handle);

        // Wait for connection to establish (increased from 100ms for reliability)
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(())
    }


    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Kraken WebSocket");
        
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
        // Kraken tickers are included in order book subscriptions
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!("Subscribing to Kraken order books for {} symbols", symbols.len());
        
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

    async fn place_order(&self, _order: &crate::connector::OrderRequest) -> Result<crate::connector::OrderResponse> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented".to_string()
        ))
    }

    async fn cancel_order(&self, _order_id: &str) -> Result<crate::connector::CancelResponse> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented".to_string()
        ))
    }

    async fn get_order_status(&self, _order_id: &str) -> Result<crate::connector::OrderStatus> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented".to_string()
        ))
    }

    async fn get_balance(&self) -> Result<crate::connector::Balance> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented".to_string()
        ))
    }

    async fn get_open_orders(&self, _symbol: Option<&Symbol>) -> Result<Vec<crate::connector::OrderStatus>> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented".to_string()
        ))
    }
}


impl KrakenConnector {
    /// Main WebSocket connection task with reconnection logic
    async fn websocket_task(
        ws_url: String,
        event_sender: broadcast::Sender<ConnectionEvent>,
        status: Arc<RwLock<ConnectionStatus>>,
        stats: Arc<Mutex<ConnectorStats>>,
        subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
        config: ConnectorConfig,
    ) {
        let mut backoff = ExponentialBackoff::new(
            Duration::from_millis(1000),
            Duration::from_millis(30000),
        );

        loop {
            match Self::connect_websocket(&ws_url).await {
                Ok((ws_stream, _)) => {
                    info!("Kraken WebSocket connected successfully");
                    backoff.reset();
                    
                    // Update status to connected
                    *status.write().await = ConnectionStatus::Connected;
                    
                    // Send status change event
                    let _ = event_sender.send(ConnectionEvent::StatusChange {
                        exchange: ExchangeId::Kraken,
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
                    ).await {
                        error!("Kraken WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to Kraken WebSocket: {}", e);
                    
                    // Update status to error
                    *status.write().await = ConnectionStatus::Error("WebSocket connection failed".to_string());
                    
                    // Send error event
                    let _ = event_sender.send(ConnectionEvent::Error {
                        exchange: ExchangeId::Kraken,
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
            warn!("Kraken WebSocket reconnecting in {:?}", delay);
            tokio::time::sleep(delay).await;
        }
    }


    /// Establish WebSocket connection
    async fn connect_websocket(
        ws_url: &str,
    ) -> Result<(tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tokio_tungstenite::tungstenite::http::Response<Option<Vec<u8>>>)> {
        let (ws_stream, response) = connect_async(ws_url).await
            .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("WebSocket connection failed: {}", e)))?;
        
        Ok((ws_stream, response))
    }

    /// Handle WebSocket connection and messages
    async fn handle_websocket_connection(
        mut ws_stream: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
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
                let mut new_pairs = Vec::new();
                
                for symbol in symbols.iter() {
                    let kraken_symbol = Self::symbol_to_kraken_static(symbol);
                    
                    if !current_subscriptions.contains(&kraken_symbol) {
                        new_pairs.push(kraken_symbol.clone());
                        current_subscriptions.push(kraken_symbol);
                    }
                }

                if !new_pairs.is_empty() {
                    let subscription = KrakenSubscription {
                        event: "subscribe".to_string(),
                        pair: new_pairs.clone(),
                        subscription: KrakenSubscriptionDetails {
                            name: "book".to_string(),
                            depth: Some(10),
                        },
                    };

                    let msg = serde_json::to_string(&subscription)
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to serialize subscription: {}", e)))?;
                    
                    debug!("Sending Kraken subscription: {}", msg);
                    
                    ws_stream.send(Message::Text(msg.into())).await
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send subscription: {}", e)))?;
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
                                warn!("Failed to handle Kraken message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            // Respond to ping with pong
                            ws_stream.send(Message::Pong(data)).await
                                .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send pong: {}", e)))?;
                        }
                        Message::Close(_) => {
                            info!("Kraken WebSocket connection closed by server");
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("Kraken WebSocket error: {}", e);
                    break;
                }
                Ok(None) => {
                    info!("Kraken WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    warn!("Kraken WebSocket timeout, sending ping");
                    // Send ping to keep connection alive
                    ws_stream.send(Message::Ping(vec![].into())).await
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send ping: {}", e)))?;
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
        debug!("Received Kraken message: {}", text);

        // Try to parse as response first (subscription confirmations, errors)
        if let Ok(response) = serde_json::from_str::<KrakenWsResponse>(text) {
            if let Some(event) = &response.event {
                match event.as_str() {
                    "subscriptionStatus" => {
                        if let Some(status) = &response.status {
                            if status == "subscribed" {
                                debug!("Kraken subscription successful: {:?}", response);
                            } else if status == "error" {
                                warn!("Kraken subscription error: {:?}", response.error_message);
                            }
                        }
                    }
                    "heartbeat" => {
                        debug!("Kraken heartbeat received");
                    }
                    "systemStatus" => {
                        debug!("Kraken system status: {:?}", response);
                    }
                    _ => {}
                }
            }
            return Ok(());
        }

        // Try to parse as array (market data format)
        if let Ok(data) = serde_json::from_str::<Value>(text) {
            if let Some(arr) = data.as_array() {
                if arr.len() >= 4 {
                    // Kraken format: [channelID, data, channelName, pair]
                    if let Ok(order_book) = Self::parse_orderbook_message(arr) {
                        let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                            exchange: ExchangeId::Kraken,
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


    /// Parse Kraken orderbook message into OrderBook
    fn parse_orderbook_message(arr: &[Value]) -> Result<OrderBook> {
        // Kraken format: [channelID, {"as":[[price,vol,ts],...], "bs":[[price,vol,ts],...]}, "book-10", "XBT/USD"]
        // or update: [channelID, {"a":[[price,vol,ts,updateType],...], "b":...}, "book-10", "XBT/USD"]
        
        let pair = arr.last()
            .and_then(|v| v.as_str())
            .ok_or_else(|| arbitrage_core::ArbitrageError::ExchangeConnection("Missing pair".to_string()))?;
        
        let symbol = Self::symbol_from_kraken_static(pair)?;
        
        let data = &arr[1];
        
        let mut asks = Vec::new();
        let mut bids = Vec::new();

        // Handle snapshot (as/bs) or update (a/b)
        let asks_key = if data.get("as").is_some() { "as" } else { "a" };
        let bids_key = if data.get("bs").is_some() { "bs" } else { "b" };

        if let Some(asks_data) = data[asks_key].as_array() {
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

        if let Some(bids_data) = data[bids_key].as_array() {
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
            exchange: ExchangeId::Kraken,
            symbol,
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }


    /// Static version of symbol conversion for use in async contexts
    fn symbol_to_kraken_static(symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Slash)
    }

    /// Static version of symbol parsing for use in async contexts
    fn symbol_from_kraken_static(kraken_symbol: &str) -> Result<Symbol> {
        parse_symbol(kraken_symbol, SymbolFormat::Slash)
    }

    fn parse_order_book(&self, data: &Value, symbol: &Symbol) -> Result<OrderBook> {
        let asks_data = data["asks"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string()))?;
        let bids_data = data["bids"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bids data".to_string()))?;

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
            exchange: ExchangeId::Kraken,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    fn parse_ticker(&self, data: &Value, symbol: &Symbol) -> Result<TickerData> {
        // Kraken ticker format: c=[price, lot_volume], v=[today, 24h], ...
        let last_price = data["c"].as_array()
            .and_then(|arr| arr.first())
            .map(|v| parse_decimal(v))
            .transpose()?
            .ok_or_else(|| arbitrage_core::ArbitrageError::ExchangeConnection("Missing last price".to_string()))?;
        
        let bid_price = data["b"].as_array()
            .and_then(|arr| arr.first())
            .map(|v| parse_decimal(v))
            .transpose()?
            .ok_or_else(|| arbitrage_core::ArbitrageError::ExchangeConnection("Missing bid price".to_string()))?;
        
        let ask_price = data["a"].as_array()
            .and_then(|arr| arr.first())
            .map(|v| parse_decimal(v))
            .transpose()?
            .ok_or_else(|| arbitrage_core::ArbitrageError::ExchangeConnection("Missing ask price".to_string()))?;

        let volume_24h = data["v"].as_array()
            .and_then(|arr| arr.get(1))
            .map(|v| parse_decimal(v))
            .transpose()?
            .unwrap_or_default();

        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::Kraken,
            last_price,
            bid_price,
            ask_price,
            volume_24h,
            price_change_24h: rust_decimal::Decimal::ZERO,
            timestamp: chrono::Utc::now(),
        })
    }
}

