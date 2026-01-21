use crate::server::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use jsonwebtoken::{DecodingKey, Validation};
use serde_json::json;
use std::fmt;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

const WS_AUTH_TIMEOUT_SECS: u64 = 10;

#[derive(Debug)]
struct WebSocketAuthError;

impl fmt::Display for WebSocketAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WebSocket authentication error")
    }
}

impl std::error::Error for WebSocketAuthError {}

fn validate_ws_token(token: &str, secret: &str) -> bool {
    jsonwebtoken::decode::<crate::server::Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .is_ok()
}

async fn authenticate_websocket(
    receiver: &mut futures_util::stream::SplitStream<WebSocket>,
    state: &AppState,
) -> Result<bool, WebSocketAuthError> {
    if let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let parsed: serde_json::Value =
                    serde_json::from_str(&text).map_err(|_| WebSocketAuthError)?;
                if let Some(token) = parsed.get("token").and_then(|t| t.as_str()) {
                    let secret = state.jwt_secret.as_str();
                    return Ok(validate_ws_token(token, secret));
                }
            }
            Ok(Message::Close(_)) => {
                return Ok(false);
            }
            Err(_) => {
                return Err(WebSocketAuthError);
            }
            _ => {}
        }
    }
    Ok(false)
}

/// Handle WebSocket upgrade
pub async fn websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    debug!("WebSocket connection requested");
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let client_id = uuid::Uuid::new_v4();

    info!("WebSocket client connected: {}", client_id);

    let auth_result = tokio::time::timeout(
        std::time::Duration::from_secs(WS_AUTH_TIMEOUT_SECS),
        authenticate_websocket(&mut receiver, &state),
    )
    .await;

    let authenticated = match auth_result {
        Ok(Ok(true)) => true,
        Ok(Ok(false)) => {
            warn!("WebSocket authentication failed for client: {}", client_id);
            false
        }
        Ok(Err(e)) => {
            warn!("WebSocket authentication error for {}: {}", client_id, e);
            false
        }
        Err(_) => {
            warn!("WebSocket authentication timeout for client: {}", client_id);
            false
        }
    };

    if !authenticated {
        let error_msg = json!({
            "type": "auth_error",
            "message": "Authentication required. Please provide a valid JWT token."
        });
        let _ = sender
            .send(Message::Text(error_msg.to_string().into()))
            .await;
        return;
    }

    info!("WebSocket client authenticated: {}", client_id);

    // Subscribe to signals
    let mut signal_receiver = state.arbitrage_engine.subscribe();

    // Send welcome message
    let welcome_msg = json!({
        "type": "welcome",
        "client_id": client_id.to_string(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    if let Err(e) = sender
        .send(Message::Text(welcome_msg.to_string().into()))
        .await
    {
        error!("Failed to send welcome message: {}", e);
        return;
    }

    // Spawn task to handle incoming messages from client
    let state_clone = state.clone();
    let client_receiver_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received message from client {}: {}", client_id, text);

                    // Handle client messages (ping, subscribe, etc.)
                    if let Err(e) = handle_client_message(&text, &state_clone).await {
                        warn!("Error handling client message: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Client {} disconnected", client_id);
                    break;
                }
                Ok(Message::Ping(_data)) => {
                    debug!("Received ping from client {}", client_id);
                    // Pong will be sent automatically by axum
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong from client {}", client_id);
                }
                Ok(Message::Binary(_)) => {
                    warn!(
                        "Received unexpected binary message from client {}",
                        client_id
                    );
                }
                Err(e) => {
                    error!("WebSocket error for client {}: {}", client_id, e);
                    break;
                }
            }
        }
    });

    // Handle outgoing messages (signals, status updates)
    let sender_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Forward signals to client
                signal_result = signal_receiver.recv() => {
                    match signal_result {
                        Ok(signal) => {
                            let signal_msg = json!({
                                "type": "signal",
                                "data": {
                                    "id": signal.id.to_string(),
                                    "symbol": signal.symbol.to_pair(),
                                    "buy_exchange": signal.buy_exchange.to_string(),
                                    "sell_exchange": signal.sell_exchange.to_string(),
                                    "buy_price": signal.buy_price,
                                    "sell_price": signal.sell_price,
                                    "gross_profit_percent": signal.gross_profit_percent,
                                    "net_profit_percent": signal.net_profit_percent,
                                    "confidence": signal.confidence,
                                    "recommended_size": signal.recommended_size,
                                    "created_at": signal.created_at.to_rfc3339(),
                                    "expires_at": signal.expires_at.to_rfc3339()
                                },
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            });

                            if let Err(e) = sender.send(Message::Text(signal_msg.to_string().into())).await {
                                error!("Failed to send signal to client {}: {}", client_id, e);
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("Client {} lagged, skipped {} signals", client_id, skipped);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Signal channel closed for client {}", client_id);
                            break;
                        }
                    }
                }

                // Send periodic heartbeat
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    let heartbeat_msg = json!({
                        "type": "heartbeat",
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    });

                    if let Err(e) = sender.send(Message::Text(heartbeat_msg.to_string().into())).await {
                        error!("Failed to send heartbeat to client {}: {}", client_id, e);
                        break;
                    }
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = client_receiver_task => {
            debug!("Client receiver task completed for {}", client_id);
        }
        _ = sender_task => {
            debug!("Sender task completed for {}", client_id);
        }
    }

    info!("WebSocket connection closed for client {}", client_id);
}

/// Handle incoming message from client
async fn handle_client_message(
    message: &str,
    _state: &AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    let parsed: serde_json::Value = serde_json::from_str(message)?;

    let msg_type = parsed
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");

    match msg_type {
        "ping" => {
            debug!("Received ping from client");
            // Pong response will be handled by the sender task
        }
        "subscribe" => {
            debug!("Client subscription request: {:?}", parsed);
            // TODO: Handle subscription to specific symbols/exchanges
        }
        "unsubscribe" => {
            debug!("Client unsubscription request: {:?}", parsed);
            // TODO: Handle unsubscription
        }
        _ => {
            warn!("Unknown message type: {}", msg_type);
        }
    }

    Ok(())
}
