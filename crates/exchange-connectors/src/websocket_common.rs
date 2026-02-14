//! Shared WebSocket functionality for all exchange connectors
//!
//! This module provides common WebSocket connection handling utilities to reduce
//! code duplication across connectors.

use crate::connector::ConnectorStats;
use crate::events::ConnectionEvent;
use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId},
    Result,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{error, info, warn};

/// Common WebSocket reconnection logic shared across exchanges
pub struct WebSocketReconnector {
    exchange_id: ExchangeId,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
}

impl WebSocketReconnector {
    pub fn new(
        exchange_id: ExchangeId,
        event_sender: broadcast::Sender<ConnectionEvent>,
        status: Arc<RwLock<ConnectionStatus>>,
        stats: Arc<Mutex<ConnectorStats>>,
    ) -> Self {
        Self {
            exchange_id,
            event_sender,
            status,
            stats,
        }
    }

    /// Handle WebSocket connection error with common pattern
    pub async fn handle_connection_error(&self, error_message: &str) {
        error!(
            "Failed to connect to {} WebSocket: {}",
            self.exchange_id, error_message
        );

        // Update status to error
        *self.status.write().await =
            ConnectionStatus::Error("WebSocket connection failed".to_string());

        // Send error event
        let _ = self.event_sender.send(ConnectionEvent::Error {
            exchange: self.exchange_id,
            error: format!("WebSocket connection failed: {}", error_message),
            timestamp: chrono::Utc::now(),
        });
    }

    /// Handle successful WebSocket connection with common pattern
    pub async fn handle_connection_success(&self) {
        info!("{} WebSocket connected successfully", self.exchange_id);

        // Update status to connected
        *self.status.write().await = ConnectionStatus::Connected;

        // Send status change event
        let _ = self.event_sender.send(ConnectionEvent::StatusChange {
            exchange: self.exchange_id,
            old_status: ConnectionStatus::Connecting,
            new_status: ConnectionStatus::Connected,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Check if reconnection should continue
    pub async fn should_continue_reconnecting(&self) -> bool {
        let current_status = self.status.read().await;
        *current_status != ConnectionStatus::Disconnected
    }

    /// Log reconnection attempt
    pub fn log_reconnection(&self, delay: Duration) {
        warn!("{} WebSocket reconnecting in {:?}", self.exchange_id, delay);
    }
}

/// Common WebSocket message handling patterns
pub struct WebSocketMessageHandler;

impl WebSocketMessageHandler {
    /// Update connection statistics when message is received
    pub async fn update_message_stats(stats: &Arc<Mutex<ConnectorStats>>) {
        let mut stats_guard = stats.lock().await;
        stats_guard.messages_received += 1;
        stats_guard.last_update = chrono::Utc::now();
    }

    /// Handle common WebSocket message types
    pub async fn handle_common_message(
        message: std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
        ws_stream: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        stats: &Arc<Mutex<ConnectorStats>>,
    ) -> Result<bool> {
        match message {
            Ok(Message::Text(_text)) => {
                Self::update_message_stats(stats).await;
                Ok(false) // Continue processing
            }
            Ok(Message::Ping(data)) => {
                use futures_util::SinkExt;
                ws_stream.send(Message::Pong(data)).await.map_err(|e| {
                    arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                        "Failed to send pong: {}",
                        e
                    ))
                })?;
                Ok(false) // Continue processing
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection closed by server");
                Ok(true) // Break loop
            }
            Ok(_) => Ok(false), // Continue processing for other message types
            Err(e) => {
                error!("WebSocket error: {}", e);
                Ok(true) // Break loop on error
            }
        }
    }

    /// Handle WebSocket timeout with ping
    pub async fn handle_timeout(
        ws_stream: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Result<()> {
        warn!("WebSocket timeout, sending ping");
        let ping_msg = r#"{"method":"PING"}"#;
        use futures_util::SinkExt;
        ws_stream
            .send(Message::Text(ping_msg.into()))
            .await
            .map_err(|e| {
                arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                    "Failed to send ping: {}",
                    e
                ))
            })?;
        Ok(())
    }
}
