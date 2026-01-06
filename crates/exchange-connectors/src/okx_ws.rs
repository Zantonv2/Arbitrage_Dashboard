use crate::connector::{ConnectorError, ConnectionEvent, ExchangeConnector};
use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// OKX WebSocket connector - NO API KEY REQUIRED
pub struct OKXWebSocketConnector {
    exchange: ExchangeId,
    url: String,
    subscribed_symbols: Vec<Symbol>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: ConnectionStatus,
}

impl OKXWebSocketConnector {
    pub fn new() -> (Self, broadcast::Receiver<ConnectionEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        
        let connector = Self {
            exchange: ExchangeId::OKX,
            url: "wss://ws.okx.com:8443/ws/v5/public".to_string(),
            subscribed_symbols: Vec::new(),
            event_sender,
            status: ConnectionStatus::Disconnected,
        };
        
        (connector, event_receiver)
    }
    
    /// Create subscription message for OKX
    fn create_subscription_message(&self, symbols: &[Symbol]) -> String {
        let args: Vec<serde_json::Value> = symbols.iter()
            .map(|s| json!({
                "channel": "books5",
                "instId": format!("{}-{}", s.base, s.quote)
            }))
            .collect();
        
        json!({
            "op": "subscribe",
            "args": args
        }).to_string()
    }
    
    /// Parse OKX WebSocket message
    fn parse_message(exchange: ExchangeId, message: &str) -> Result<Option<OrderBook>, ConnectorError> {
        let data: Value = serde_json::from_str(message)
            .map_err(|e| ConnectorError::Parse(format!("Failed to parse JSON: {}", e)))?;
        
        // Check if it's an order book update
        if let Some(arg) = data.get("arg") {
            if let Some(channel) = arg.get("channel").and_then(|c| c.as_str()) {
                if channel == "books5" {
                    return Self::parse_orderbook_update(exchange, &data);
                }
            }
        }
        
        // Check for subscription confirmation
        if let Some(event) = data.get("event").and_then(|e| e.as_str()) {
            if event == "subscribe" {
                debug!("OKX subscription confirmed");
            }
        }
        
        Ok(None)
    }
    
    /// Parse order book update from OKX
    fn parse_orderbook_update(exchange: ExchangeId, data: &Value) -> Result<Option<OrderBook>, ConnectorError> {
        let arg = data.get("arg")
            .ok_or_else(|| ConnectorError::Parse("Missing arg field".to_string()))?;
        
        let inst_id = arg.get("instId")
            .and_then(|i| i.as_str())
            .ok_or_else(|| ConnectorError::Parse("Missing instId".to_string()))?;
        
        // Parse symbol (BTC-USDT -> BTC/USDT)
        let symbol = Self::parse_symbol(inst_id)?;
        
        let data_array = data.get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| ConnectorError::Parse("Missing data array".to_string()))?;
        
        if let Some(order_data) = data_array.first() {
            // Parse bids
            let mut bids = Vec::new();
            if let Some(bids_array) = order_data.get("bids").and_then(|b| b.as_array()) {
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
            if let Some(asks_array) = order_data.get("asks").and_then(|a| a.as_array()) {
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
        } else {
            Ok(None)
        }
    }
    
    /// Parse OKX symbol format (BTC-USDT -> BTC/USDT)
    fn parse_symbol(symbol_str: &str) -> Result<Symbol, ConnectorError> {
        let parts: Vec<&str> = symbol_str.split('-').collect();
        if parts.len() == 2 {
            Ok(Symbol::new(parts[0], parts[1]))
        } else {
            Err(ConnectorError::Parse(format!("Cannot parse OKX symbol: {}", symbol_str)))
        }
    }
}

#[async_trait]
impl ExchangeConnector for OKXWebSocketConnector {
    fn exchange_id(&self) -> ExchangeId {
        self.exchange
    }
    
    async fn connect(&mut self) -> Result<(), ConnectorError> {
        info!("🔗 Connecting to OKX WebSocket: {}", self.url);
        
        match connect_async(&self.url).await {
            Ok((mut ws_stream, response)) => {
                info!("✅ Connected to OKX WebSocket, status: {}", response.status());
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
                                        debug!("OKX non-orderbook message: {}", text);
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse OKX message: {}", e);
                                    }
                                }
                                last_heartbeat = Instant::now();
                            }
                            Some(Ok(Message::Ping(data))) => {
                                if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                                    error!("Failed to send pong to OKX: {}", e);
                                    break;
                                }
                                last_heartbeat = Instant::now();
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("OKX WebSocket closed by server");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("OKX WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                warn!("OKX WebSocket stream ended");
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
                error!("❌ Failed to connect to OKX WebSocket: {}", e);
                self.status = ConnectionStatus::Error(e.to_string());
                Err(ConnectorError::Connection(format!("WebSocket connection failed: {}", e)))
            }
        }
    }
    
    async fn disconnect(&mut self) -> Result<(), ConnectorError> {
        info!("🔌 Disconnecting from OKX WebSocket");
        self.status = ConnectionStatus::Disconnected;
        let _ = self.event_sender.send(ConnectionEvent::Disconnected(self.exchange));
        Ok(())
    }
    
    async fn subscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols = symbols.clone();
        info!("📡 Subscribed to {} symbols on OKX", symbols.len());
        Ok(())
    }
    
    async fn unsubscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols.retain(|s| !symbols.contains(s));
        info!("📡 Unsubscribed from {} symbols on OKX", symbols.len());
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