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

/// MEXC WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MexcSubscription {
    method: String,
    params: Vec<String>,
}

/// MEXC WebSocket response message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct MexcWsResponse {
    id: Option<u64>,
    code: Option<i32>,
    msg: Option<String>,
}

/// MEXC WebSocket market data message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct MexcMarketData {
    c: Option<String>,
    d: Option<Value>,
    s: Option<String>,
    t: Option<i64>,
}

pub struct MEXCConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl MEXCConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::MEXC,
            ws_url: "wss://wbs.mexc.com/ws".to_string(),
            rest_url: "https://api.mexc.com".to_string(),
            rate_limit_per_second: 20,
            rate_limit_burst: 40,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::MEXC;

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

    fn symbol_to_mexc(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    fn symbol_from_mexc(&self, mexc_symbol: &str) -> Result<Symbol> {
        parse_symbol(mexc_symbol, SymbolFormat::NoSeparator)
    }
}

#[async_trait]
impl ExchangeConnector for MEXCConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::MEXC
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
        let mexc_symbol = self.symbol_to_mexc(symbol);
        let url = format!("{}/api/v3/depth?symbol={}&limit={}", 
                         self.config.rest_url, mexc_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        self.parse_order_book(&data, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/api/v3/exchangeInfo", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(symbols_array) = data["symbols"].as_array() {
            for item in symbols_array {
                if let Some(symbol_str) = item["symbol"].as_str() {
                    // MEXC uses status "1" for trading enabled (not "TRADING")
                    // Also check isSpotTradingAllowed for spot trading
                    let status = item["status"].as_str().unwrap_or("");
                    let is_spot_allowed = item["isSpotTradingAllowed"].as_bool().unwrap_or(false);
                    
                    // Accept status "1" (enabled) or "TRADING" for compatibility
                    if (status == "1" || status == "TRADING") && is_spot_allowed {
                        if let Ok(symbol) = self.symbol_from_mexc(symbol_str) {
                            symbols.push(symbol);
                        }
                    }
                }
            }
        }
        Ok(symbols)
    }

    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
        let url = format!("{}/api/v3/ticker/bookTicker", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut tickers = HashMap::new();
        if let Some(ticker_array) = data.as_array() {
            for ticker_data in ticker_array {
                if let Some(symbol_str) = ticker_data["symbol"].as_str() {
                    if let Ok(symbol) = self.symbol_from_mexc(symbol_str) {
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

    async fn fetch_funding_rates(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        let mut funding_rates = HashMap::new();
        for symbol in symbols {
            let mexc_symbol = format!("{}_USDT", symbol.base.to_uppercase());
            let url = format!("{}/api/v1/contract/funding_rate/{}", self.config.rest_url, mexc_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<Value>().await {
                    if let Ok(funding_rate) = self.parse_funding_rate(&data, symbol) {
                        funding_rates.insert(symbol.clone(), funding_rate);
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

        info!("Connecting to MEXC WebSocket: {}", self.config.ws_url);
        
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
        info!("Disconnecting from MEXC WebSocket");
        
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
        // MEXC tickers are included in order book subscriptions
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!("Subscribing to MEXC order books for {} symbols", symbols.len());
        
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
}


impl MEXCConnector {
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
                    info!("MEXC WebSocket connected successfully");
                    backoff.reset();
                    
                    // Update status to connected
                    *status.write().await = ConnectionStatus::Connected;
                    
                    // Send status change event
                    let _ = event_sender.send(ConnectionEvent::StatusChange {
                        exchange: ExchangeId::MEXC,
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
                        error!("MEXC WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to MEXC WebSocket: {}", e);
                    
                    // Update status to error
                    *status.write().await = ConnectionStatus::Error("WebSocket connection failed".to_string());
                    
                    // Send error event
                    let _ = event_sender.send(ConnectionEvent::Error {
                        exchange: ExchangeId::MEXC,
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
            warn!("MEXC WebSocket reconnecting in {:?}", delay);
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
        let mut last_ping = std::time::Instant::now();

        loop {
            if last_subscription_check.elapsed() > Duration::from_secs(5) {
                let symbols = subscribed_symbols.read().await;
                let mut new_params = Vec::new();
                
                for symbol in symbols.iter() {
                    let mexc_symbol = Self::symbol_to_mexc_static(symbol);
                    let channel = format!("spot@public.bookTicker.v3.api@{}", mexc_symbol);
                    
                    if !current_subscriptions.contains(&channel) {
                        new_params.push(channel.clone());
                        current_subscriptions.push(channel);
                    }
                }

                if !new_params.is_empty() {
                    let subscription = MexcSubscription {
                        method: "SUBSCRIPTION".to_string(),
                        params: new_params,
                    };

                    let msg = serde_json::to_string(&subscription)
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to serialize: {}", e)))?;
                    
                    debug!("Sending MEXC subscription: {}", msg);
                    ws_stream.send(Message::Text(msg.into())).await
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send: {}", e)))?;
                }
                
                last_subscription_check = std::time::Instant::now();
            }

            if last_ping.elapsed() > Duration::from_secs(25) {
                let ping_msg = r#"{"method":"PING"}"#;
                ws_stream.send(Message::Text(ping_msg.into())).await
                    .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send ping: {}", e)))?;
                last_ping = std::time::Instant::now();
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
                                warn!("Failed to handle MEXC message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            ws_stream.send(Message::Pong(data)).await
                                .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send pong: {}", e)))?;
                        }
                        Message::Close(_) => {
                            info!("MEXC WebSocket connection closed by server");
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("MEXC WebSocket error: {}", e);
                    break;
                }
                Ok(None) => {
                    info!("MEXC WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    warn!("MEXC WebSocket timeout, sending ping");
                    let ping_msg = r#"{"method":"PING"}"#;
                    ws_stream.send(Message::Text(ping_msg.into())).await
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send ping: {}", e)))?;
                }
            }

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
        debug!("Received MEXC message: {}", text);

        // Try to parse as JSON
        let data: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        // Handle PONG response
        if data.get("msg").and_then(|m| m.as_str()) == Some("PONG") {
            debug!("MEXC PONG received");
            return Ok(());
        }

        // Handle subscription response
        if let Some(code) = data.get("code").and_then(|c| c.as_i64()) {
            if code == 0 {
                debug!("MEXC subscription successful");
            } else {
                warn!("MEXC subscription error: code {}", code);
            }
            return Ok(());
        }

        // Handle market data
        if let Some(channel) = data.get("c").and_then(|c| c.as_str()) {
            if channel.contains("bookTicker") {
                if let Ok(order_book) = Self::parse_bookticker_message(&data) {
                    let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                        exchange: ExchangeId::MEXC,
                        order_book,
                        timestamp: chrono::Utc::now(),
                    });
                    let _ = event_sender.send(event);
                }
            }
        }

        Ok(())
    }

    /// Parse MEXC book ticker message into OrderBook
    fn parse_bookticker_message(data: &Value) -> Result<OrderBook> {
        let d = data.get("d").ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing data".to_string()))?;
        
        let symbol_str = data.get("s").and_then(|s| s.as_str()).ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing symbol".to_string()))?;
        
        let symbol = Self::symbol_from_mexc_static(symbol_str)?;

        let bid_price = parse_decimal(&d["b"])?;
        let bid_qty = parse_decimal(&d["B"])?;
        let ask_price = parse_decimal(&d["a"])?;
        let ask_qty = parse_decimal(&d["A"])?;

        let timestamp = if let Some(t) = data.get("t").and_then(|t| t.as_i64()) {
            chrono::DateTime::from_timestamp_millis(t)
                .unwrap_or_else(|| chrono::Utc::now())
        } else {
            chrono::Utc::now()
        };

        Ok(OrderBook {
            exchange: ExchangeId::MEXC,
            symbol,
            bids: vec![OrderBookLevel { price: bid_price, quantity: bid_qty }],
            asks: vec![OrderBookLevel { price: ask_price, quantity: ask_qty }],
            timestamp,
            sequence: None,
        })
    }

    /// Static version of symbol conversion for use in async contexts
    fn symbol_to_mexc_static(symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    /// Static version of symbol parsing for use in async contexts
    fn symbol_from_mexc_static(mexc_symbol: &str) -> Result<Symbol> {
        parse_symbol(mexc_symbol, SymbolFormat::NoSeparator)
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
            exchange: ExchangeId::MEXC,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    fn parse_ticker(&self, data: &Value, symbol: &Symbol) -> Result<TickerData> {
        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::MEXC,
            last_price: parse_decimal(&data["price"]).unwrap_or_default(),
            bid_price: parse_decimal(&data["bidPrice"])?,
            ask_price: parse_decimal(&data["askPrice"])?,
            volume_24h: parse_decimal(&data["bidQty"]).unwrap_or_default(),
            price_change_24h: rust_decimal::Decimal::ZERO,
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_funding_rate(&self, data: &Value, symbol: &Symbol) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["data"]["fundingRate"])?;
        
        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::MEXC,
            funding_rate,
            predicted_rate: None,
            funding_time: chrono::Utc::now(),
            timestamp: chrono::Utc::now(),
        })
    }
}
