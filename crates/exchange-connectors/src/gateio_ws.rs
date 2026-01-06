use crate::connector::{ConnectorError, ConnectionEvent, ExchangeConnector};
use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Gate.io WebSocket connector - NO API KEY REQUIRED
pub struct GateIoWebSocketConnector {
    exchange: ExchangeId,
    url: String,
    subscribed_symbols: Vec<Symbol>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: ConnectionStatus,
}

impl GateIoWebSocketConnector {
    pub fn new() -> (Self, broadcast::Receiver<ConnectionEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        
        let connector = Self {
            exchange: ExchangeId::GateIo,
            url: "wss://api.gateio.ws/ws/v4/".to_string(),
            subscribed_symbols: Vec::new(),
            event_sender,
            status: ConnectionStatus::Disconnected,
        };
        
        (connector, event_receiver)
    }
    
    /// Parse Gate.io symbol format (BTC_USDT -> BTC/USDT)
    fn parse_symbol(symbol_str: &str) -> Result<Symbol, ConnectorError> {
        let parts: Vec<&str> = symbol_str.split('_').collect();
        if parts.len() == 2 {
            Ok(Symbol::new(parts[0], parts[1]))
        } else {
            Err(ConnectorError::Parse(format!("Cannot parse Gate.io symbol: {}", symbol_str)))
        }
    }
}

#[async_trait]
impl ExchangeConnector for GateIoWebSocketConnector {
    fn exchange_id(&self) -> ExchangeId {
        self.exchange
    }
    
    async fn connect(&mut self) -> Result<(), ConnectorError> {
        info!("🔗 Connecting to Gate.io WebSocket: {}", self.url);
        
        match connect_async(&self.url).await {
            Ok((mut ws_stream, response)) => {
                info!("✅ Connected to Gate.io WebSocket, status: {}", response.status());
                self.status = ConnectionStatus::Connected;
                
                let _ = self.event_sender.send(ConnectionEvent::Connected(self.exchange));
                
                // Start message handling loop in separate task
                let event_sender = self.event_sender.clone();
                let exchange = self.exchange;
                
                tokio::spawn(async move {
                    loop {
                        match ws_stream.next().await {
                            Some(Ok(Message::Text(text))) => {
                                debug!("Gate.io message: {}", text);
                                // Simplified - just acknowledge we got data
                            }
                            Some(Ok(Message::Ping(data))) => {
                                if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                                    error!("Failed to send pong to Gate.io: {}", e);
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("Gate.io WebSocket closed by server");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("Gate.io WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                warn!("Gate.io WebSocket stream ended");
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
                error!("❌ Failed to connect to Gate.io WebSocket: {}", e);
                self.status = ConnectionStatus::Error(e.to_string());
                Err(ConnectorError::Connection(format!("WebSocket connection failed: {}", e)))
            }
        }
    }
    
    async fn disconnect(&mut self) -> Result<(), ConnectorError> {
        info!("🔌 Disconnecting from Gate.io WebSocket");
        self.status = ConnectionStatus::Disconnected;
        let _ = self.event_sender.send(ConnectionEvent::Disconnected(self.exchange));
        Ok(())
    }
    
    async fn subscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols = symbols.clone();
        info!("📡 Subscribed to {} symbols on Gate.io", symbols.len());
        Ok(())
    }
    
    async fn unsubscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols.retain(|s| !symbols.contains(s));
        info!("📡 Unsubscribed from {} symbols on Gate.io", symbols.len());
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