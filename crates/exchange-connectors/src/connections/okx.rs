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

/// OKX WebSocket subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OkxSubscription {
    op: String,
    args: Vec<OkxSubscriptionArg>,
}

/// OKX WebSocket subscription argument
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OkxSubscriptionArg {
    channel: String,
    #[serde(rename = "instId")]
    inst_id: String,
}

/// OKX WebSocket response message
#[derive(Debug, Clone, Deserialize)]
struct OkxWsResponse {
    event: Option<String>,
    code: Option<String>,
    msg: Option<String>,
    #[serde(rename = "connId")]
    conn_id: Option<String>,
}

/// OKX WebSocket market data message
#[derive(Debug, Clone, Deserialize)]
struct OkxMarketData {
    arg: OkxMarketDataArg,
    action: Option<String>,
    data: Vec<Value>,
}

/// OKX market data argument
#[derive(Debug, Clone, Deserialize)]
struct OkxMarketDataArg {
    channel: String,
    #[serde(rename = "instId")]
    inst_id: String,
}

pub struct OKXConnector {
    pub config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl OKXConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::OKX,
            ws_url: "wss://ws.okx.com:8443/ws/v5/public".to_string(),
            rest_url: "https://www.okx.com".to_string(),
            rate_limit_per_second: 20,
            rate_limit_burst: 40,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::OKX;

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

    fn symbol_to_okx(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Dash)
    }

    fn symbol_from_okx(&self, okx_symbol: &str) -> Result<Symbol> {
        parse_symbol(okx_symbol, SymbolFormat::Dash)
    }
}

#[async_trait]
impl ExchangeConnector for OKXConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::OKX
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
        let okx_symbol = self.symbol_to_okx(symbol);
        let url = format!("{}/api/v5/market/books?instId={}&sz={}", 
                         self.config.rest_url, okx_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        if let Some(data_array) = data["data"].as_array() {
            if let Some(book) = data_array.first() {
                return self.parse_order_book(book, symbol);
            }
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection("No order book data".to_string()))
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/api/v5/public/instruments?instType=SPOT", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(data_array) = data["data"].as_array() {
            for item in data_array {
                if let Some(inst_id) = item["instId"].as_str() {
                    if let Some(state) = item["state"].as_str() {
                        if state == "live" {
                            if let Ok(symbol) = self.symbol_from_okx(inst_id) {
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
            let okx_symbol = self.symbol_to_okx(symbol);
            let url = format!("{}/api/v5/market/ticker?instId={}", self.config.rest_url, okx_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(data_array) = data["data"].as_array() {
                        if let Some(ticker_data) = data_array.first() {
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
            let okx_symbol = format!("{}-SWAP", self.symbol_to_okx(symbol));
            let url = format!("{}/api/v5/public/funding-rate?instId={}", 
                             self.config.rest_url, okx_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(data_array) = data["data"].as_array() {
                        if let Some(funding_data) = data_array.first() {
                            if let Ok(funding_rate) = self.parse_funding_rate(funding_data, symbol) {
                                funding_rates.insert(symbol.clone(), funding_rate);
                            }
                        }
                    }
                }
            }
        }
        Ok(funding_rates)
    }

    async fn connect(&mut self) -> Result<()> {
        {
            let status = self.status.read().await;
            if *status == ConnectionStatus::Connected {
                return Ok(());
            }
        }

        info!("Connecting to OKX WebSocket: {}", self.config.ws_url);
        
        *self.status.write().await = ConnectionStatus::Connecting;

        let ws_url = self.config.ws_url.clone();
        let event_sender = self.event_sender.clone();
        let status = self.status.clone();
        let stats = self.stats.clone();
        let subscribed_symbols = self.subscribed_symbols.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            Self::websocket_task(ws_url, event_sender, status, stats, subscribed_symbols, config).await;
        });

        *self.ws_handle.lock().await = Some(handle);
        
        // Wait for connection to establish (increased from 100ms for reliability)
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from OKX WebSocket");
        
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
        info!("Subscribing to OKX order books for {} symbols", symbols.len());
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
}


impl OKXConnector {
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
                    info!("OKX WebSocket connected successfully");
                    backoff.reset();
                    
                    *status.write().await = ConnectionStatus::Connected;
                    
                    let _ = event_sender.send(ConnectionEvent::StatusChange {
                        exchange: ExchangeId::OKX,
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
                    ).await {
                        error!("OKX WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to OKX WebSocket: {}", e);
                    *status.write().await = ConnectionStatus::Error("WebSocket connection failed".to_string());
                    let _ = event_sender.send(ConnectionEvent::Error {
                        exchange: ExchangeId::OKX,
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
            warn!("OKX WebSocket reconnecting in {:?}", delay);
            tokio::time::sleep(delay).await;
        }
    }

    async fn connect_websocket(
        ws_url: &str,
    ) -> Result<(tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tokio_tungstenite::tungstenite::http::Response<Option<Vec<u8>>>)> {
        let (ws_stream, response) = connect_async(ws_url).await
            .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("WebSocket connection failed: {}", e)))?;
        Ok((ws_stream, response))
    }

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
                let mut new_args = Vec::new();
                
                for symbol in symbols.iter() {
                    let okx_symbol = Self::symbol_to_okx_static(symbol);
                    
                    if !current_subscriptions.contains(&okx_symbol) {
                        new_args.push(OkxSubscriptionArg {
                            channel: "books5".to_string(),
                            inst_id: okx_symbol.clone(),
                        });
                        current_subscriptions.push(okx_symbol);
                    }
                }

                if !new_args.is_empty() {
                    let subscription = OkxSubscription {
                        op: "subscribe".to_string(),
                        args: new_args,
                    };

                    let msg = serde_json::to_string(&subscription)
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to serialize: {}", e)))?;
                    
                    debug!("Sending OKX subscription: {}", msg);
                    ws_stream.send(Message::Text(msg.into())).await
                        .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send: {}", e)))?;
                }
                
                last_subscription_check = std::time::Instant::now();
            }

            if last_ping.elapsed() > Duration::from_secs(25) {
                ws_stream.send(Message::Text("ping".into())).await
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
                                warn!("Failed to handle OKX message: {}", e);
                            }
                        }
                        Message::Ping(data) => {
                            ws_stream.send(Message::Pong(data)).await
                                .map_err(|e| arbitrage_core::ArbitrageError::ExchangeConnection(format!("Failed to send pong: {}", e)))?;
                        }
                        Message::Close(_) => {
                            info!("OKX WebSocket connection closed by server");
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("OKX WebSocket error: {}", e);
                    break;
                }
                Ok(None) => {
                    info!("OKX WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    warn!("OKX WebSocket timeout, sending ping");
                    ws_stream.send(Message::Text("ping".into())).await
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

    async fn handle_text_message(
        text: &str,
        event_sender: &broadcast::Sender<ConnectionEvent>,
    ) -> Result<()> {
        debug!("Received OKX message: {}", text);

        if text == "pong" {
            return Ok(());
        }

        if let Ok(response) = serde_json::from_str::<OkxWsResponse>(text) {
            if let Some(event) = &response.event {
                if event == "subscribe" {
                    debug!("OKX subscription confirmed: {:?}", response);
                } else if event == "error" {
                    warn!("OKX error: code={:?}, msg={:?}", response.code, response.msg);
                }
            }
            return Ok(());
        }

        if let Ok(market_data) = serde_json::from_str::<OkxMarketData>(text) {
            if market_data.arg.channel.starts_with("books") {
                if let Ok(order_book) = Self::parse_orderbook_message(&market_data) {
                    let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                        exchange: ExchangeId::OKX,
                        order_book,
                        timestamp: chrono::Utc::now(),
                    });
                    let _ = event_sender.send(event);
                }
            }
        }

        Ok(())
    }

    fn parse_orderbook_message(market_data: &OkxMarketData) -> Result<OrderBook> {
        let data = market_data.data.first().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("No data in message".to_string()))?;
        
        let symbol = Self::symbol_from_okx_static(&market_data.arg.inst_id)?;

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

        let timestamp = if let Some(ts_str) = data["ts"].as_str() {
            if let Ok(ts) = ts_str.parse::<i64>() {
                chrono::DateTime::from_timestamp_millis(ts).unwrap_or_else(chrono::Utc::now)
            } else {
                chrono::Utc::now()
            }
        } else {
            chrono::Utc::now()
        };

        Ok(OrderBook {
            exchange: ExchangeId::OKX,
            symbol,
            bids,
            asks,
            timestamp,
            sequence: data["seqId"].as_u64(),
        })
    }

    fn symbol_to_okx_static(symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Dash)
    }

    fn symbol_from_okx_static(okx_symbol: &str) -> Result<Symbol> {
        parse_symbol(okx_symbol, SymbolFormat::Dash)
    }

    fn parse_order_book(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<OrderBook> {
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

        let timestamp = if let Some(ts_str) = data["ts"].as_str() {
            if let Ok(ts) = ts_str.parse::<i64>() {
                chrono::DateTime::from_timestamp_millis(ts).unwrap_or_else(chrono::Utc::now)
            } else {
                chrono::Utc::now()
            }
        } else {
            chrono::Utc::now()
        };

        Ok(OrderBook {
            exchange: ExchangeId::OKX,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp,
            sequence: None,
        })
    }

    fn parse_ticker(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<TickerData> {
        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::OKX,
            last_price: parse_decimal(&data["last"])?,
            bid_price: parse_decimal(&data["bidPx"])?,
            ask_price: parse_decimal(&data["askPx"])?,
            volume_24h: parse_decimal(&data["vol24h"]).unwrap_or_default(),
            price_change_24h: parse_decimal(&data["sodUtc0"]).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_funding_rate(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["fundingRate"])?;
        let funding_time = parse_timestamp(&data["fundingTime"])?;
        let predicted_rate = parse_decimal(&data["nextFundingRate"]).ok();
        
        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::OKX,
            funding_rate,
            predicted_rate,
            funding_time,
            timestamp: chrono::Utc::now(),
        })
    }
}
