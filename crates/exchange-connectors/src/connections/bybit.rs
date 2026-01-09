use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{format_symbol, parse_symbol, parse_timestamp, parse_decimal, SymbolFormat, ExponentialBackoff};
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

pub struct BybitConnector {
    config: ConnectorConfig,
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

    fn symbol_to_bybit(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    fn symbol_from_bybit(&self, bybit_symbol: &str) -> Result<Symbol> {
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
        let url = format!("{}/v5/market/orderbook?category=spot&symbol={}&limit={}", 
                         self.config.rest_url, bybit_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        if let Some(result) = data["result"].as_object() {
            return self.parse_order_book(result, symbol);
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection("No order book data".to_string()))
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/v5/market/instruments-info?category=spot", self.config.rest_url);
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
            let url = format!("{}/v5/market/tickers?category=spot&symbol={}", self.config.rest_url, bybit_symbol);
            
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

    async fn fetch_funding_rates(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        let mut funding_rates = HashMap::new();
        for symbol in symbols {
            let bybit_symbol = self.symbol_to_bybit(symbol);
            let url = format!("{}/v5/market/funding/history?category=linear&symbol={}&limit=1", 
                             self.config.rest_url, bybit_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(result) = data["result"].as_object() {
                        if let Some(list) = result["list"].as_array() {
                            if let Some(funding_data) = list.first() {
                                if let Ok(funding_rate) = self.parse_funding_rate(funding_data, symbol) {
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
            Self::websocket_task(ws_url, event_sender, status, stats, subscribed_symbols, config).await;
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
        info!("Subscribing to ByBit order books for {} symbols", symbols.len());
        
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
            "Trading methods not yet implemented for ByBit".to_string()
        ))
    }

    async fn cancel_order(&self, _order_id: &str) -> Result<crate::connector::CancelResponse> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented for ByBit".to_string()
        ))
    }

    async fn get_order_status(&self, _order_id: &str) -> Result<crate::connector::OrderStatus> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented for ByBit".to_string()
        ))
    }

    async fn get_balance(&self) -> Result<crate::connector::Balance> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented for ByBit".to_string()
        ))
    }

    async fn get_open_orders(&self, _symbol: Option<&Symbol>) -> Result<Vec<crate::connector::OrderStatus>> {
        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "Trading methods not yet implemented for ByBit".to_string()
        ))
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
        let mut backoff = ExponentialBackoff::new(
            Duration::from_millis(1000),
            Duration::from_millis(30000),
        );

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
                    ).await {
                        error!("ByBit WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to ByBit WebSocket: {}", e);
                    
                    // Update status to error
                    *status.write().await = ConnectionStatus::Error("WebSocket connection failed".to_string());
                    
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

                    let msg = serde_json::to_string(&subscription)
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to serialize subscription: {}", e)))?;
                    
                    debug!("Sending ByBit subscription: {}", msg);
                    
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
                                warn!("Failed to handle ByBit message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            // Respond to ping with pong
                            ws_stream.send(Message::Pong(data)).await
                                .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send pong: {}", e)))?;
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
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection("Invalid topic format".to_string()));
        }
        let bybit_symbol = topic_parts[2];
        let symbol = Self::symbol_from_bybit_static(bybit_symbol)?;

        let asks_data = data["a"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string()))?;
        let bids_data = data["b"].as_array().ok_or_else(|| 
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

    fn parse_order_book(&self, data: &serde_json::Map<String, serde_json::Value>, symbol: &Symbol) -> Result<OrderBook> {
        let asks_data = data["a"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string()))?;
        let bids_data = data["b"].as_array().ok_or_else(|| 
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
            exchange: ExchangeId::ByBit,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    fn parse_ticker(&self, data: &serde_json::Value) -> Result<TickerData> {
        let symbol_str = data["symbol"].as_str().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing symbol".to_string()))?;
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

    fn parse_funding_rate(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<FundingRate> {
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