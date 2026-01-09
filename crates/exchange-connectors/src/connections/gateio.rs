use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{format_symbol, parse_symbol, parse_decimal, parse_timestamp, SymbolFormat, ExponentialBackoff};
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

/// Gate.io WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GateSubscription {
    time: i64,
    channel: String,
    event: String,
    payload: Vec<String>,
}

/// Gate.io WebSocket response message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct GateWsResponse {
    time: Option<i64>,
    channel: Option<String>,
    event: Option<String>,
    error: Option<GateError>,
    result: Option<Value>,
}

/// Gate.io error structure
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct GateError {
    code: Option<i32>,
    message: Option<String>,
}

/// Gate.io WebSocket market data message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct GateMarketData {
    time: i64,
    channel: String,
    event: String,
    result: Value,
}

pub struct GateIOConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl GateIOConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::GateIo,
            ws_url: "wss://api.gateio.ws/ws/v4/".to_string(),
            rest_url: "https://api.gateio.ws/api/v4".to_string(),
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

    fn symbol_to_gate(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Underscore)
    }

    fn symbol_from_gate(&self, gate_symbol: &str) -> Result<Symbol> {
        parse_symbol(gate_symbol, SymbolFormat::Underscore)
    }
}

#[async_trait]
impl ExchangeConnector for GateIOConnector {
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
        let gate_symbol = self.symbol_to_gate(symbol);
        let url = format!("{}/spot/order_book?currency_pair={}&limit={}", 
                         self.config.rest_url, gate_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        self.parse_order_book(&data, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/spot/currency_pairs", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(pairs) = data.as_array() {
            for item in pairs {
                if let Some(id) = item["id"].as_str() {
                    if let Some(status) = item["trade_status"].as_str() {
                        if status == "tradable" {
                            if let Ok(symbol) = self.symbol_from_gate(id) {
                                symbols.push(symbol);
                            }
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
            let gate_symbol = self.symbol_to_gate(symbol);
            let url = format!("{}/spot/tickers?currency_pair={}", self.config.rest_url, gate_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<Value>().await {
                    if let Some(arr) = data.as_array() {
                        if let Some(ticker_data) = arr.first() {
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

    async fn fetch_funding_rates(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        let mut funding_rates = HashMap::new();
        for symbol in symbols {
            let gate_symbol = self.symbol_to_gate(symbol);
            // Gate.io uses futures API for funding rates
            let url = format!("{}/futures/usdt/contracts/{}", self.config.rest_url, gate_symbol);
            
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

        info!("Connecting to Gate.io WebSocket: {}", self.config.ws_url);
        
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
        info!("Disconnecting from Gate.io WebSocket");
        
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
        // Gate.io tickers are included in order book subscriptions
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!("Subscribing to Gate.io order books for {} symbols", symbols.len());
        
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

impl GateIOConnector {
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
                    info!("Gate.io WebSocket connected successfully");
                    backoff.reset();
                    
                    // Update status to connected
                    *status.write().await = ConnectionStatus::Connected;
                    
                    // Send status change event
                    let _ = event_sender.send(ConnectionEvent::StatusChange {
                        exchange: ExchangeId::GateIo,
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
                        error!("Gate.io WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to Gate.io WebSocket: {}", e);
                    
                    // Update status to error
                    *status.write().await = ConnectionStatus::Error("WebSocket connection failed".to_string());
                    
                    // Send error event
                    let _ = event_sender.send(ConnectionEvent::Error {
                        exchange: ExchangeId::GateIo,
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
            warn!("Gate.io WebSocket reconnecting in {:?}", delay);
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
            // Check for new subscriptions every 5 seconds
            if last_subscription_check.elapsed() > Duration::from_secs(5) {
                let symbols = subscribed_symbols.read().await;
                let mut new_symbols = Vec::new();
                
                for symbol in symbols.iter() {
                    let gate_symbol = Self::symbol_to_gate_static(symbol);
                    
                    if !current_subscriptions.contains(&gate_symbol) {
                        new_symbols.push(gate_symbol.clone());
                        current_subscriptions.push(gate_symbol);
                    }
                }

                if !new_symbols.is_empty() {
                    let subscription = GateSubscription {
                        time: chrono::Utc::now().timestamp(),
                        channel: "spot.book_ticker".to_string(),
                        event: "subscribe".to_string(),
                        payload: new_symbols.clone(),
                    };

                    let msg = serde_json::to_string(&subscription)
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to serialize subscription: {}", e)))?;
                    
                    debug!("Sending Gate.io subscription: {}", msg);
                    
                    ws_stream.send(Message::Text(msg.into())).await
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send subscription: {}", e)))?;
                }
                
                last_subscription_check = std::time::Instant::now();
            }

            // Send ping every 25 seconds to keep connection alive
            if last_ping.elapsed() > Duration::from_secs(25) {
                let ping = r#"{"time":0,"channel":"spot.ping"}"#;
                ws_stream.send(Message::Text(ping.into())).await
                    .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send ping: {}", e)))?;
                last_ping = std::time::Instant::now();
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
                                warn!("Failed to handle Gate.io message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            // Respond to ping with pong
                            ws_stream.send(Message::Pong(data)).await
                                .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send pong: {}", e)))?;
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
                    // Send ping to keep connection alive
                    let ping = r#"{"time":0,"channel":"spot.ping"}"#;
                    ws_stream.send(Message::Text(ping.into())).await
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
        debug!("Received Gate.io message: {}", text);

        // Try to parse as response first
        if let Ok(response) = serde_json::from_str::<GateWsResponse>(text) {
            if let Some(event) = &response.event {
                if event == "subscribe" {
                    debug!("Gate.io subscription response: {:?}", response);
                    return Ok(());
                }
            }
            
            // Check for errors
            if let Some(error) = &response.error {
                warn!("Gate.io error: {:?}", error);
                return Ok(());
            }
        }

        // Try to parse as market data
        if let Ok(market_data) = serde_json::from_str::<GateMarketData>(text) {
            if market_data.channel == "spot.book_ticker" && market_data.event == "update" {
                if let Ok(order_book) = Self::parse_book_ticker_message(&market_data) {
                    let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                        exchange: ExchangeId::GateIo,
                        order_book,
                        timestamp: chrono::Utc::now(),
                    });

                    let _ = event_sender.send(event);
                }
            }
        }

        Ok(())
    }

    /// Parse Gate.io book ticker message into OrderBook
    fn parse_book_ticker_message(market_data: &GateMarketData) -> Result<OrderBook> {
        let data = &market_data.result;
        
        let symbol_str = data["s"].as_str().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing symbol".to_string()))?;
        let symbol = Self::symbol_from_gate_static(symbol_str)?;

        let bid_price = parse_decimal(&data["b"])?;
        let bid_qty = parse_decimal(&data["B"])?;
        let ask_price = parse_decimal(&data["a"])?;
        let ask_qty = parse_decimal(&data["A"])?;

        let timestamp = if let Some(t) = data["t"].as_i64() {
            chrono::DateTime::from_timestamp_millis(t)
                .unwrap_or_else(|| chrono::Utc::now())
        } else {
            chrono::Utc::now()
        };

        Ok(OrderBook {
            exchange: ExchangeId::GateIo,
            symbol,
            bids: vec![OrderBookLevel { price: bid_price, quantity: bid_qty }],
            asks: vec![OrderBookLevel { price: ask_price, quantity: ask_qty }],
            timestamp,
            sequence: data["u"].as_u64(),
        })
    }

    /// Static version of symbol conversion for use in async contexts
    fn symbol_to_gate_static(symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Underscore)
    }

    /// Static version of symbol parsing for use in async contexts
    fn symbol_from_gate_static(gate_symbol: &str) -> Result<Symbol> {
        parse_symbol(gate_symbol, SymbolFormat::Underscore)
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
            exchange: ExchangeId::GateIo,
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
            exchange: ExchangeId::GateIo,
            last_price: parse_decimal(&data["last"])?,
            bid_price: parse_decimal(&data["highest_bid"])?,
            ask_price: parse_decimal(&data["lowest_ask"])?,
            volume_24h: parse_decimal(&data["base_volume"]).unwrap_or_default(),
            price_change_24h: parse_decimal(&data["change_percentage"]).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_funding_rate(&self, data: &Value, symbol: &Symbol) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["funding_rate"])?;
        let funding_time = parse_timestamp(&data["funding_next_apply"])?;
        
        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::GateIo,
            funding_rate,
            predicted_rate: parse_decimal(&data["funding_rate_indicative"]).ok(),
            funding_time,
            timestamp: chrono::Utc::now(),
        })
    }
}

