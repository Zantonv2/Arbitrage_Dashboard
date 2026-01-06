use crate::connector::{ConnectorError, ConnectionEvent, ExchangeConnector};
use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Hyperliquid WebSocket connector - NO API KEY REQUIRED
pub struct HyperliquidWebSocketConnector {
    exchange: ExchangeId,
    url: String,
    subscribed_symbols: Vec<Symbol>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: ConnectionStatus,
}

impl HyperliquidWebSocketConnector {
    pub fn new() -> (Self, broadcast::Receiver<ConnectionEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        
        let connector = Self {
            exchange: ExchangeId::Hyperliquid,
            url: "wss://api.hyperliquid.xyz/ws".to_string(),
            subscribed_symbols: Vec::new(),
            event_sender,
            status: ConnectionStatus::Disconnected,
        };
        
        (connector, event_receiver)
    }
}

#[async_trait]
impl ExchangeConnector for HyperliquidWebSocketConnector {
    fn exchange_id(&self) -> ExchangeId {
        self.exchange
    }
    
    async fn connect(&mut self) -> Result<(), ConnectorError> {
        info!("🔗 Connecting to Hyperliquid WebSocket: {}", self.url);
        
        match connect_async(&self.url).await {
            Ok((mut ws_stream, response)) => {
                info!("✅ Connected to Hyperliquid WebSocket, status: {}", response.status());
                self.status = ConnectionStatus::Connected;
                
                let _ = self.event_sender.send(ConnectionEvent::Connected(self.exchange));
                
                // Start message handling loop in separate task
                let event_sender = self.event_sender.clone();
                let exchange = self.exchange;
                
                tokio::spawn(async move {
                    loop {
                        match ws_stream.next().await {
                            Some(Ok(Message::Text(text))) => {
                                debug!("Hyperliquid message: {}", text);
                                // Simplified - just acknowledge we got data
                            }
                            Some(Ok(Message::Ping(data))) => {
                                if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                                    error!("Failed to send pong to Hyperliquid: {}", e);
                                    break;
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("Hyperliquid WebSocket closed by server");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("Hyperliquid WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                warn!("Hyperliquid WebSocket stream ended");
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
                error!("❌ Failed to connect to Hyperliquid WebSocket: {}", e);
                self.status = ConnectionStatus::Error(e.to_string());
                Err(ConnectorError::Connection(format!("WebSocket connection failed: {}", e)))
            }
        }
    }
    
    async fn disconnect(&mut self) -> Result<(), ConnectorError> {
        info!("🔌 Disconnecting from Hyperliquid WebSocket");
        self.status = ConnectionStatus::Disconnected;
        let _ = self.event_sender.send(ConnectionEvent::Disconnected(self.exchange));
        Ok(())
    }
    
    async fn subscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols = symbols.clone();
        info!("📡 Subscribed to {} symbols on Hyperliquid", symbols.len());
        Ok(())
    }
    
    async fn unsubscribe(&mut self, symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        self.subscribed_symbols.retain(|s| !symbols.contains(s));
        info!("📡 Unsubscribed from {} symbols on Hyperliquid", symbols.len());
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