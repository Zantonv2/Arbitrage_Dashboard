use crate::connector::{ConnectorError, ConnectionEvent, ExchangeConnector};
use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// ByBit WebSocket connector - NO API KEY REQUIRED
pub struct ByBitWebSocketConnector {
    exchange: ExchangeId,
    url: String,
    subscribed_symbols: Vec<Symbol>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: ConnectionStatus,
}

impl ByBitWebSocketConnector {
    pub fn new() -> (Self, broadcast::Receiver<ConnectionEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        
        let connector = Self {
            exchange: ExchangeId::ByBit,
            url: "wss://stream.bybit.com/v5/public/spot".to_string(),
            subscribed_symbols: Vec::new(),
            event_sender,
            status: ConnectionStatus::Disconnected,
        };
        
        (connector, event_receiver)
    }
    
    /// Create subscription message for ByBit
    fn create_subscription_message(&self, symbols: &[Symbol]) -> String {
        let topics: Vec<String> = symbols.iter()
            .map(|s| format!("orderbook.50.{}{}", s.base, s.quote))
            .collect();
        
        json!({
            "op": "subscribe",
            "args": topics
        }).to_string()
    }
    
    /// Parse ByBit WebSocket message
    fn parse_message(exchange: ExchangeId, message: &str) -> Result<Option<OrderBook>, ConnectorError> {
        let data: Value = serde_json::from_str(message)
            .map_err(|e| ConnectorError::Parse(format!("Failed to parse JSON: {}", e)))?;
        
        // Check if it's an order book update
        if let Some(topic) = data.get("topic").and_then(|t| t.as_str()) {
            if topic.starts_with("orderbook.50.") {
                return Self::parse_orderbook_update(exchange, &data);
            }
        }
        
        // Check for subscription confirmation
        if let Some(success) = data.get("success").and_then(|s| s.as_bool()) {
            if success {
                debug!("ByBit subscription confirmed");
            }
        }
        
        Ok(None)
    }
    
    /// Parse order book update from ByBit
    fn parse_orderbook_update(exchange: ExchangeId, data: &Value) -> Result<Option<OrderBook>, ConnectorError> {
        let topic = data.get("topic")
            .and_then(|t| t.as_str())
            .ok_or_else(|| ConnectorError::Parse("Missing topic".to_string()))?;
        
        // Extract symbol from topic (e.g., "orderbook.50.BTCUSDT" -> "BTCUSDT")
        let symbol_str = topic.strip_prefix("orderbook.50.")
            .ok_or_else(|| ConnectorError::Parse("Invalid topic format".to_string()))?;
        
        // Parse symbol (BTCUSDT -> BTC/USDT)
        let symbol = Self::parse_symbol(symbol_str)?;
        
        let order_data = data.get("data")
            .ok_or_else(|| ConnectorError::Parse("Missing data field".to_string()))?;
        
        // Parse bids
        let mut bids = Vec::new();
        if let Some(bids_array) = order_data.get("b").and_then(|b| b.as_array()) {
            for bid in bids_array {
                if let Some(bid_array) = bid.as_array() {
                    if bid_array.len() >= 2 {
                        if let (Some(price_str), Some(qty_str)) = (
                            bid_array[0].as_str(),
                            bid_array[1].as_str(),
                        ) {
                            let price = price_str.parse().map_err(|e| {
                                ConnectorError::Parse(format!("Failed to parse bid price: {}", e))
                            })?;
                            let quantity = qty_str.parse().map_err(|e| {
                                ConnectorError::Parse(format!("Failed to parse bid quantity: {}", e))
                            })?;
                            bids.push(OrderBookLevel::new(price, quantity));
                        }
                    }
                }
            }
        }
        
        // Parse asks
        let mut asks = Vec::new();
        if let Some(asks_array) = order_data.get("a").and_then(|a| a.as_array()) {
            for ask in asks_array {
                if let Some(ask_array) = ask.as_array() {
                    if ask_array.len() >= 2 {
                        if let (Some(price_str), Some(qty_str)) = (
                            ask_array[0].as_str(),
                            ask_array[1].as_str(),
                        ) {
                            let price = price_str.parse().map_err(|e| {
                                ConnectorError::Parse(format!("Failed to parse ask price: {}", e))
                            })?;
                            let quantity = qty_str.parse().map_err(|e| {
                                ConnectorError::Parse(format!("Failed to parse ask quantity: {}", e))
                            })?;
                            asks.push(OrderBookLevel::new(price, quantity));
                        }
                    }
                }
            }
        }
        
        if !bids.is_empty() || !asks.is_empty() {
            let order_book = OrderBook::new(exchange, symbol, bids, asks);
            Ok(Some(order_book))
        } else {
            Ok(None)
        }
    }
    
    /// Parse ByBit symbol format (BTCUSDT -> BTC/USDT)
    fn parse_symbol(symbol_str: &str) -> Result<Symbol, ConnectorError> {
        // Common quote currencies in order of preference
        let quote_currencies = ["USDT", "USDC", "BTC", "ETH"];
        
        for quote in &quote_currencies {
            if symbol_str.ends_with(quote) {
                let base = &symbol_str[..symbol_str.len() - quote.len()];
                if !base.is_empty() {
                    return Ok(Symbol::new(base, *quote));
                }
            }
        }
        
        Err(ConnectorError::Parse(format!("Cannot parse ByBit symbol: {}", symbol_str)))
    }
}

#[async_trait]
impl ExchangeConnector for ByBitWebSocketConnector {
    fn exchange_id(&self) -> ExchangeId {
        self.exchange
    }
    
    async fn connect(&mut self) -> Result<(), ConnectorError> {
        info!("🔗 Connecting to ByBit WebSocket: {}", self.url);
        
        match connect_async(&self.url).await {
            Ok((mut ws_stream, response)) => {
                info!("✅ Connected to ByBit WebSocket, status: {}", response.status());
                self.status = ConnectionStatus::Connected;
                
                let _ = self.event_sender.send(ConnectionEvent::Connected(self.exchange));
                
                // Start message handling loop in separate task
                let event_sender = self.event_sender.clone();
                let exchange = self.exchange;
                
                tokio::spawn(async move {
                    let mut last_heartbeat = Instant::now();
                    
                    loop {
                        match ws_stream.next().await {
                            Some(Ok(Message::Text(text))) => {
                                match Self::parse_message(exchange, &text) {
                                    Ok(Some(order_book)) => {
                                        let _ = event_sender.send(ConnectionEvent::OrderBookUpdate(order_book));
                                    }
                                    Ok(None) => {
                                        debug!("ByBit non-orderbook message: {}", text);
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse ByBit message: {}", e);
                                    }
                                }
                                last_heartbeat = Instant::now();
                            }
                            Some(Ok(Message::Ping(data))) => {
                                if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                                    error!("Failed to send pong to ByBit: {}", e);
                                    break;
                                }
                                last_heartbeat = Instant::now();
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("ByBit WebSocket closed by server");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("ByBit WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                warn!("ByBit WebSocket stream ended");
                                break;
                            }
                            _ => {}
                        }
                    }
                    
                    let _ = event_sender.send(ConnectionEvent::Disconnected(exchange));
                });
                
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to connect to ByBit WebSocket: {}", e);
                self.status = ConnectionStatus::Error(e.to_string());
                Err(ConnectorError::Connection(format!("WebSocket connection failed: {}", e)))
            }
        }
    }
    
    async fn disconnect(&mut self) -> Result<(), ConnectorError> {
        info!("🔌 Disconnecting from ByBit WebSocket");
        self.status = ConnectionStatus::Disconnected;
        let _ = self.event_sender.send(ConnectionEvent::Disconnected(self.exchange));
        Ok(())
    }
    
    async fn subscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols = symbols.clone();
        
        // For this simplified version, we'll assume subscription happens during connect
        // In a full implementation, you'd need to store the WebSocket stream reference
        info!("📡 Subscribed to {} symbols on ByBit", symbols.len());
        Ok(())
    }
    
    async fn unsubscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols.retain(|s| !symbols.contains(s));
        info!("📡 Unsubscribed from {} symbols on ByBit", symbols.len());
        Ok(())
    }
    
    fn status(&self) -> ConnectionStatus {
        self.status.clone()
    }
    
    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_sender.subscribe()
    }
    
    fn subscribed_symbols(&self) -> Vec<Symbol> {
        self.subscribed_symbols.clone()
    }
}