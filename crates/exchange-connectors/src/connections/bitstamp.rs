use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{parse_symbol, parse_decimal, SymbolFormat, ExponentialBackoff};
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

/// Bitstamp WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitstampSubscription {
    event: String,
    data: BitstampSubscriptionData,
}

/// Bitstamp subscription data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BitstampSubscriptionData {
    channel: String,
}

/// Bitstamp WebSocket response message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct BitstampWsResponse {
    event: String,
    channel: Option<String>,
    data: Option<Value>,
}

/// Bitstamp WebSocket market data message
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct BitstampMarketData {
    event: String,
    channel: String,
    data: Value,
}

pub struct BitstampConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}


impl BitstampConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::Bitstamp,
            ws_url: "wss://ws.bitstamp.net".to_string(),
            rest_url: "https://www.bitstamp.net/api/v2".to_string(),
            rate_limit_per_second: 10,
            rate_limit_burst: 20,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        
        // Bitstamp requires a proper User-Agent header for REST API requests
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());
        
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::Bitstamp;

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

    fn symbol_to_bitstamp(&self, symbol: &Symbol) -> String {
        format!("{}{}", symbol.base.to_lowercase(), symbol.quote.to_lowercase())
    }

    fn symbol_from_bitstamp(&self, bitstamp_symbol: &str) -> Result<Symbol> {
        parse_symbol(bitstamp_symbol, SymbolFormat::NoSeparator)
    }
}

#[async_trait]
impl ExchangeConnector for BitstampConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Bitstamp
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
        let bitstamp_symbol = self.symbol_to_bitstamp(symbol);
        let url = format!("{}/order_book/{}", self.config.rest_url, bitstamp_symbol);

        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        self.parse_order_book(&data, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/trading-pairs-info/", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(pairs) = data.as_array() {
            for item in pairs {
                if let Some(trading) = item["trading"].as_str() {
                    if trading == "Enabled" {
                        if let Some(url_symbol) = item["url_symbol"].as_str() {
                            if let Ok(symbol) = self.symbol_from_bitstamp(url_symbol) {
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
            let bitstamp_symbol = self.symbol_to_bitstamp(symbol);
            let url = format!("{}/ticker/{}", self.config.rest_url, bitstamp_symbol);
            
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

    async fn fetch_funding_rates(&self, _symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        // Bitstamp is spot only, no funding rates
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

        info!("Connecting to Bitstamp WebSocket: {}", self.config.ws_url);
        
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
        info!("Disconnecting from Bitstamp WebSocket");
        
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
        // Bitstamp tickers are included in order book subscriptions
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!("Subscribing to Bitstamp order books for {} symbols", symbols.len());
        
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


impl BitstampConnector {
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
                    info!("Bitstamp WebSocket connected successfully");
                    backoff.reset();
                    
                    // Update status to connected
                    *status.write().await = ConnectionStatus::Connected;
                    
                    // Send status change event
                    let _ = event_sender.send(ConnectionEvent::StatusChange {
                        exchange: ExchangeId::Bitstamp,
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
                        error!("Bitstamp WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to Bitstamp WebSocket: {}", e);
                    
                    // Update status to error
                    *status.write().await = ConnectionStatus::Error("WebSocket connection failed".to_string());
                    
                    // Send error event
                    let _ = event_sender.send(ConnectionEvent::Error {
                        exchange: ExchangeId::Bitstamp,
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
            warn!("Bitstamp WebSocket reconnecting in {:?}", delay);
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

                        let msg = serde_json::to_string(&subscription)
                            .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to serialize subscription: {}", e)))?;
                        
                        debug!("Sending Bitstamp subscription: {}", msg);
                        
                        ws_stream.send(Message::Text(msg.into())).await
                            .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send subscription: {}", e)))?;
                        
                        current_subscriptions.push(channel);
                    }
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
                                warn!("Failed to handle Bitstamp message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            // Respond to ping with pong
                            ws_stream.send(Message::Pong(data)).await
                                .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send pong: {}", e)))?;
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
        debug!("Received Bitstamp message: {}", text);

        // Try to parse as response first
        if let Ok(response) = serde_json::from_str::<BitstampWsResponse>(text) {
            match response.event.as_str() {
                "bts:subscription_succeeded" => {
                    debug!("Bitstamp subscription successful: {:?}", response.channel);
                    return Ok(());
                }
                "bts:error" => {
                    warn!("Bitstamp error: {:?}", response.data);
                    return Ok(());
                }
                "bts:request_reconnect" => {
                    warn!("Bitstamp requested reconnect");
                    return Err(arbitrage_core::ArbitrageError::ExchangeConnection("Reconnect requested".to_string()));
                }
                _ => {}
            }
        }

        // Try to parse as market data
        if let Ok(market_data) = serde_json::from_str::<BitstampMarketData>(text) {
            if market_data.event == "data" && market_data.channel.starts_with("order_book_") {
                if let Ok(order_book) = Self::parse_orderbook_message(&market_data) {
                    let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                        exchange: ExchangeId::Bitstamp,
                        order_book,
                        timestamp: chrono::Utc::now(),
                    });

                    let _ = event_sender.send(event);
                }
            }
        }

        Ok(())
    }

    /// Parse Bitstamp orderbook message into OrderBook
    fn parse_orderbook_message(market_data: &BitstampMarketData) -> Result<OrderBook> {
        let data = &market_data.data;
        
        // Extract symbol from channel (e.g., "order_book_btcusd" -> "btcusd")
        let symbol_str = market_data.channel.strip_prefix("order_book_")
            .ok_or_else(|| arbitrage_core::ArbitrageError::ExchangeConnection("Invalid channel format".to_string()))?;
        let symbol = Self::symbol_from_bitstamp_static(symbol_str)?;

        let asks_data = data["asks"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string()))?;
        let bids_data = data["bids"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bids data".to_string()))?;

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

        let timestamp = if let Some(ts) = data["microtimestamp"].as_str() {
            let micros: i64 = ts.parse().unwrap_or(0);
            chrono::DateTime::from_timestamp_micros(micros)
                .unwrap_or_else(|| chrono::Utc::now())
        } else {
            chrono::Utc::now()
        };

        Ok(OrderBook {
            exchange: ExchangeId::Bitstamp,
            symbol,
            bids,
            asks,
            timestamp,
            sequence: None,
        })
    }


    /// Static version of symbol conversion for use in async contexts
    fn symbol_to_bitstamp_static(symbol: &Symbol) -> String {
        format!("{}{}", symbol.base.to_lowercase(), symbol.quote.to_lowercase())
    }

    /// Static version of symbol parsing for use in async contexts
    fn symbol_from_bitstamp_static(bitstamp_symbol: &str) -> Result<Symbol> {
        parse_symbol(bitstamp_symbol, SymbolFormat::NoSeparator)
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
            exchange: ExchangeId::Bitstamp,
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
            exchange: ExchangeId::Bitstamp,
            last_price: parse_decimal(&data["last"])?,
            bid_price: parse_decimal(&data["bid"])?,
            ask_price: parse_decimal(&data["ask"])?,
            volume_24h: parse_decimal(&data["volume"]).unwrap_or_default(),
            price_change_24h: rust_decimal::Decimal::ZERO,
            timestamp: chrono::Utc::now(),
        })
    }
}

