use crate::connector::{ConnectorError, ConnectionEvent, ExchangeConnector};
use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// HTX (Huobi) WebSocket connector - NO API KEY REQUIRED
pub struct HTXWebSocketConnector {
    exchange: ExchangeId,
    url: String,
    subscribed_symbols: Vec<Symbol>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: ConnectionStatus,
}

impl HTXWebSocketConnector {
    pub fn new() -> (Self, broadcast::Receiver<ConnectionEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        
        let connector = Self {
            exchange: ExchangeId::HTX,
            url: "wss://api.huobi.pro/ws".to_string(),
            subscribed_symbols: Vec::new(),
            event_sender,
            status: ConnectionStatus::Disconnected,
        };
        
        (connector, event_receiver)
    }
    
    /// Parse HTX symbol format (btcusdt -> BTC/USDT)
    fn parse_symbol(symbol_str: &str) -> Result<Symbol, ConnectorError> {
        let symbol_upper = symbol_str.to_uppercase();
        
        // Common quote currencies in order of preference
        let quote_currencies = ["USDT", "USDC", "BTC", "ETH"];
        
        for quote in &quote_currencies {
            if symbol_upper.ends_with(quote) {
                let base = &symbol_upper[..symbol_upper.len() - quote.len()];
                if !base.is_empty() {
                    return Ok(Symbol::new(base, *quote));
                }
            }
        }
        
        Err(ConnectorError::Parse(format!("Cannot parse HTX symbol: {}", symbol_str)))
    }
}

#[async_trait]
impl ExchangeConnector for HTXWebSocketConnector {
    fn exchange_id(&self) -> ExchangeId {
        self.exchange
    }
    
    async fn connect(&mut self) -> Result<(), ConnectorError> {
        info!("🔗 Connecting to HTX WebSocket: {}", self.url);
        
        match connect_async(&self.url).await {
            Ok((mut ws_stream, response)) => {
                info!("✅ Connected to HTX WebSocket, status: {}", response.status());
                self.status = ConnectionStatus::Connected;
                
                let _ = self.event_sender.send(ConnectionEvent::Connected(self.exchange));
                
                // Start message handling loop in separate task
                let event_sender = self.event_sender.clone();
                let exchange = self.exchange;
                
                tokio::spawn(async move {
                    loop {
                        match ws_stream.next().await {
                            Some(Ok(Message::Text(text))) => {
                                // Check for ping message first
                                if let Ok(data) = serde_json::from_str::<Value>(&text) {
                                    if let Some(ping) = data.get("ping") {
                                        // Respond to ping with pong
                                        let pong_msg = json!({"pong": ping}).to_string();
                                        if let Err(e) = ws_stream.send(Message::Text(pong_msg.into())).await {
                                            error!("Failed to send pong to HTX: {}", e);
                                            break;
                                        }
                                        continue;
                                    }
                                }
                                debug!("HTX message: {}", text);
                            }
                            Some(Ok(Message::Binary(_data))) => {
                                // HTX sometimes sends compressed data
                                debug!("HTX binary message received");
                            }
                            Some(Ok(Message::Ping(data))) => {
                                if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                                    error!("Failed to send pong to HTX: {}", e);
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("HTX WebSocket closed by server");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("HTX WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                warn!("HTX WebSocket stream ended");
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
                error!("❌ Failed to connect to HTX WebSocket: {}", e);
                self.status = ConnectionStatus::Error(e.to_string());
                Err(ConnectorError::Connection(format!("WebSocket connection failed: {}", e)))
            }
        }
    }
    
    async fn disconnect(&mut self) -> Result<(), ConnectorError> {
        info!("🔌 Disconnecting from HTX WebSocket");
        self.status = ConnectionStatus::Disconnected;
        let _ = self.event_sender.send(ConnectionEvent::Disconnected(self.exchange));
        Ok(())
    }
    
    async fn subscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols = symbols.clone();
        info!("📡 Subscribed to {} symbols on HTX", symbols.len());
        Ok(())
    }
    
    async fn unsubscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols.retain(|s| !symbols.contains(s));
        info!("📡 Unsubscribed from {} symbols on HTX", symbols.len());
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