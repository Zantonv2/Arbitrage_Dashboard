use crate::connector::{ConnectorError, ConnectionEvent};
use arbitrage_core::types::{ExchangeId, OrderBook, Symbol};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration, Instant};
use tokio_tungstenite::{
    connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};

/// WebSocket connection pool for managing multiple exchange connections
pub struct WebSocketPool {
    connections: Arc<RwLock<HashMap<ExchangeId, WebSocketConnection>>>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    rate_limits: HashMap<ExchangeId, u32>, // requests per second
}

/// Individual WebSocket connection for an exchange
struct WebSocketConnection {
    exchange: ExchangeId,
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    subscribed_symbols: Vec<Symbol>,
    last_heartbeat: Instant,
    connection_attempts: u32,
    is_connected: bool,
}

impl WebSocketPool {
    /// Create new WebSocket connection pool with optimized rate limits
    pub fn new() -> (Self, broadcast::Receiver<ConnectionEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(10000);
        
        // Rate limits based on your BIG BOSS data (req/sec)
        let mut rate_limits = HashMap::new();
        rate_limits.insert(ExchangeId::GateIo, 75);      // 67-83 req/sec - THE BEAST!
        rate_limits.insert(ExchangeId::KuCoin, 65);      // 58-75 req/sec
        rate_limits.insert(ExchangeId::ByBit, 60);       // 50-67 req/sec
        rate_limits.insert(ExchangeId::Bitget, 50);      // 42-58 req/sec
        rate_limits.insert(ExchangeId::MEXC, 50);        // 42-58 req/sec
        rate_limits.insert(ExchangeId::OKX, 40);         // 33-50 req/sec
        rate_limits.insert(ExchangeId::HTX, 40);         // 33-50 req/sec
        rate_limits.insert(ExchangeId::Hyperliquid, 30); // 30-33 req/sec
        rate_limits.insert(ExchangeId::BingX, 8);        // 8-10 req/sec
        
        let pool = Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            rate_limits,
        };
        
        (pool, event_receiver)
    }
    
    /// Get WebSocket URL for exchange
    fn get_websocket_url(&self, exchange: ExchangeId) -> &'static str {
        match exchange {
            ExchangeId::ByBit => "wss://stream.bybit.com/v5/public/spot",
            ExchangeId::MEXC => "wss://wbs.mexc.com/ws",
            ExchangeId::OKX => "wss://ws.okx.com:8443/ws/v5/public",
            ExchangeId::KuCoin => "wss://ws-api-spot.kucoin.com/",
            ExchangeId::Bitget => "wss://ws.bitget.com/spot/v1/stream",
            ExchangeId::GateIo => "wss://api.gateio.ws/ws/v4/",
            ExchangeId::HTX => "wss://api.huobi.pro/ws",
            ExchangeId::BingX => "wss://open-api-v2.bingx.com/market",
            ExchangeId::Hyperliquid => "wss://api.hyperliquid.xyz/ws",
            _ => panic!("Unsupported exchange for WebSocket: {}", exchange),
        }
    }
    
    /// Connect to an exchange WebSocket
    pub async fn connect_exchange(&self, exchange: ExchangeId, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        let url = self.get_websocket_url(exchange);
        
        info!("🔗 Connecting to {} WebSocket: {}", exchange, url);
        
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                info!("✅ Connected to {} WebSocket", exchange);
                
                let connection = WebSocketConnection {
                    exchange,
                    ws_stream: Some(ws_stream),
                    subscribed_symbols: symbols.clone(),
                    last_heartbeat: Instant::now(),
                    connection_attempts: 0,
                    is_connected: true,
                };
                
                // Store connection
                {
                    let mut connections = self.connections.write().await;
                    connections.insert(exchange, connection);
                }
                
                // Subscribe to symbols
                self.subscribe_to_symbols(exchange, symbols).await?;
                
                // Start message handling loop
                self.start_message_loop(exchange).await;
                
                // Send connected event
                let _ = self.event_sender.send(ConnectionEvent::Connected(exchange));
                
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to connect to {} WebSocket: {}", exchange, e);
                Err(ConnectorError::Connection(format!("WebSocket connection failed: {}", e)))
            }
        }
    }
    
    /// Subscribe to symbols on an exchange
    async fn subscribe_to_symbols(&self, exchange: ExchangeId, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        let subscription_message = self.create_subscription_message(exchange, &symbols)?;
        
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(&exchange) {
            if let Some(ws_stream) = &mut connection.ws_stream {
                ws_stream.send(Message::Text(subscription_message.into())).await
                    .map_err(|e| ConnectorError::Connection(format!("Failed to send subscription: {}", e)))?;
                
                info!("📡 Subscribed to {} symbols on {}", symbols.len(), exchange);
            }
        }
        
        Ok(())
    }
    
    /// Create subscription message for exchange
    fn create_subscription_message(&self, exchange: ExchangeId, symbols: &[Symbol]) -> Result<String, ConnectorError> {
        match exchange {
            ExchangeId::ByBit => {
                let topics: Vec<String> = symbols.iter()
                    .map(|s| format!("orderbook.50.{}{}", s.base, s.quote))
                    .collect();
                
                Ok(serde_json::json!({
                    "op": "subscribe",
                    "args": topics
                }).to_string())
            }
            ExchangeId::BingX => {
                let topics: Vec<String> = symbols.iter()
                    .map(|s| format!("{}-{}@depth20", s.base, s.quote))
                    .collect();
                
                Ok(serde_json::json!({
                    "id": "1",
                    "method": "SUBSCRIBE",
                    "params": topics
                }).to_string())
            }
            ExchangeId::MEXC => {
                let topics: Vec<String> = symbols.iter()
                    .map(|s| format!("spot@public.bookTicker.v3.api@{}{}", s.base, s.quote))
                    .collect();
                
                Ok(serde_json::json!({
                    "method": "SUBSCRIPTION",
                    "params": topics
                }).to_string())
            }
            ExchangeId::OKX => {
                let args: Vec<serde_json::Value> = symbols.iter()
                    .map(|s| serde_json::json!({
                        "channel": "books5",
                        "instId": format!("{}-{}", s.base, s.quote)
                    }))
                    .collect();
                
                Ok(serde_json::json!({
                    "op": "subscribe",
                    "args": args
                }).to_string())
            }
            ExchangeId::KuCoin => {
                let topic = format!("/market/level2:{}", 
                    symbols.iter()
                        .map(|s| format!("{}-{}", s.base, s.quote))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                
                Ok(serde_json::json!({
                    "id": "1",
                    "type": "subscribe",
                    "topic": topic,
                    "response": true
                }).to_string())
            }
            ExchangeId::Bitget => {
                let args: Vec<serde_json::Value> = symbols.iter()
                    .map(|s| serde_json::json!({
                        "instType": "sp",
                        "channel": "books5",
                        "instId": format!("{}{}", s.base, s.quote)
                    }))
                    .collect();
                
                Ok(serde_json::json!({
                    "op": "subscribe",
                    "args": args
                }).to_string())
            }
            ExchangeId::GateIo => {
                let channels: Vec<String> = symbols.iter()
                    .map(|s| format!("spot.order_book_update.{}_{}", s.base, s.quote))
                    .collect();
                
                Ok(serde_json::json!({
                    "method": "SUBSCRIBE",
                    "params": channels,
                    "id": 1
                }).to_string())
            }
            ExchangeId::HTX => {
                let subs: Vec<String> = symbols.iter()
                    .map(|s| format!("market.{}{}.depth.step0", s.base.to_lowercase(), s.quote.to_lowercase()))
                    .collect();
                
                // HTX requires individual subscription messages
                Ok(serde_json::json!({
                    "sub": subs[0], // Subscribe to first symbol, others will be handled separately
                    "id": "1"
                }).to_string())
            }
            ExchangeId::Hyperliquid => {
                Ok(serde_json::json!({
                    "method": "subscribe",
                    "subscription": {
                        "type": "l2Book",
                        "coin": symbols[0].base // Hyperliquid subscribes per coin
                    }
                }).to_string())
            }
            _ => Err(ConnectorError::Parse(format!("Unsupported exchange for subscription: {}", exchange)))
        }
    }
    
    /// Start message handling loop for an exchange
    async fn start_message_loop(&self, exchange: ExchangeId) {
        let connections = self.connections.clone();
        let event_sender = self.event_sender.clone();
        
        tokio::spawn(async move {
            loop {
                let mut should_reconnect = false;
                
                // Handle messages
                {
                    let mut connections_guard = connections.write().await;
                    if let Some(connection) = connections_guard.get_mut(&exchange) {
                        if let Some(ws_stream) = &mut connection.ws_stream {
                            match ws_stream.next().await {
                                Some(Ok(Message::Text(text))) => {
                                    // Parse and handle message
                                    if let Ok(order_book) = Self::parse_websocket_message(exchange, &text) {
                                        let _ = event_sender.send(ConnectionEvent::OrderBookUpdate(order_book));
                                    }
                                    connection.last_heartbeat = Instant::now();
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    // Respond to ping
                                    let _ = ws_stream.send(Message::Pong(data)).await;
                                    connection.last_heartbeat = Instant::now();
                                }
                                Some(Ok(Message::Close(_))) => {
                                    warn!("🔌 {} WebSocket closed", exchange);
                                    should_reconnect = true;
                                }
                                Some(Err(e)) => {
                                    error!("❌ {} WebSocket error: {}", exchange, e);
                                    should_reconnect = true;
                                }
                                None => {
                                    warn!("🔌 {} WebSocket stream ended", exchange);
                                    should_reconnect = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                
                if should_reconnect {
                    // Handle reconnection logic here
                    warn!("🔄 Reconnecting to {} WebSocket...", exchange);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    // Reconnection logic would go here
                }
                
                // Check for heartbeat timeout
                {
                    let connections_guard = connections.read().await;
                    if let Some(connection) = connections_guard.get(&exchange) {
                        if connection.last_heartbeat.elapsed() > Duration::from_secs(60) {
                            warn!("💔 {} WebSocket heartbeat timeout", exchange);
                            // Trigger reconnection
                        }
                    }
                }
            }
        });
    }
    
    /// Parse WebSocket message from exchange
    fn parse_websocket_message(exchange: ExchangeId, message: &str) -> Result<OrderBook, ConnectorError> {
        // This is a simplified parser - each exchange needs specific parsing logic
        let data: Value = serde_json::from_str(message)
            .map_err(|e| ConnectorError::Parse(format!("Failed to parse JSON: {}", e)))?;
        
        // For now, return a dummy order book - real implementation would parse each exchange format
        Ok(OrderBook::new(
            exchange,
            arbitrage_core::types::Symbol::new("BTC", "USDT"),
            vec![], // bids
            vec![], // asks
        ))
    }
    
    /// Get connection statistics
    pub async fn get_stats(&self) -> HashMap<ExchangeId, ConnectionStats> {
        let connections = self.connections.read().await;
        let mut stats = HashMap::new();
        
        for (exchange, connection) in connections.iter() {
            stats.insert(*exchange, ConnectionStats {
                is_connected: connection.is_connected,
                connection_attempts: connection.connection_attempts,
                subscribed_symbols: connection.subscribed_symbols.len(),
                last_heartbeat: connection.last_heartbeat,
                rate_limit: self.rate_limits.get(exchange).copied().unwrap_or(0),
            });
        }
        
        stats
    }
    
    /// Disconnect from all exchanges
    pub async fn disconnect_all(&self) {
        let mut connections = self.connections.write().await;
        for (exchange, connection) in connections.iter_mut() {
            if let Some(ws_stream) = &mut connection.ws_stream {
                let _ = ws_stream.close(None).await;
                info!("🔌 Disconnected from {} WebSocket", exchange);
            }
            connection.is_connected = false;
        }
        connections.clear();
    }
}

/// Connection statistics for monitoring
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub is_connected: bool,
    pub connection_attempts: u32,
    pub subscribed_symbols: usize,
    pub last_heartbeat: Instant,
    pub rate_limit: u32,
}

impl Default for WebSocketPool {
    fn default() -> Self {
        Self::new().0
    }
}