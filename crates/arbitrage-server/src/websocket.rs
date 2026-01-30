use crate::server::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::{IntoResponse, Response},
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use http::StatusCode;
use jsonwebtoken::{DecodingKey, Validation};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

#[derive(Debug, Deserialize)]
struct ClientMessage {
    #[serde(default)]
    msg_type: String,
    #[serde(default)]
    r#type: String,
}

#[derive(Debug, Deserialize)]
pub struct WsQueryParams {
    pub token: Option<String>,
}

fn validate_ws_token(token: &str, secret: &str) -> bool {
    jsonwebtoken::decode::<crate::server::Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .is_ok()
}

/// Handle WebSocket upgrade with token validation at HTTP level
///
/// # Security Note
/// Authentication is performed at the HTTP level before the WebSocket upgrade.
/// The JWT token must be provided in the Authorization header (not query parameters)
/// to prevent token leakage in server logs or browser history.
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(_params): Query<WsQueryParams>,
    req: http::Request<axum::body::Body>,
) -> Response {
    debug!("WebSocket connection requested");

    // SECURITY: Query parameters are intentionally ignored to prevent token leakage
    // in server logs, browser history, and referrer headers. The token must be
    // provided in the Authorization header only.

    // Extract and validate JWT token from Authorization header
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|auth_str| auth_str.strip_prefix("Bearer "));

    let secret = state.jwt_secret.as_str();
    let is_authenticated = token.is_some_and(|t| validate_ws_token(t, secret));

    if !is_authenticated {
        warn!(
            "WebSocket authentication failed: invalid or missing JWT token in Authorization header"
        );
        return (
            StatusCode::UNAUTHORIZED,
            json!({
                "type": "auth_error",
                "message": "Authentication required. Please provide a valid JWT token in the Authorization header."
            }).to_string()
        ).into_response();
    }

    debug!("WebSocket authentication successful, upgrading connection");

    // Authentication successful - proceed with WebSocket upgrade
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let client_id = uuid::Uuid::new_v4();

    info!("WebSocket client connected: {}", client_id);

    // Authentication is validated at HTTP level in websocket_handler
    // This flag tracks if we should proceed with the connection
    // The pre-validation in websocket_handler ensures token is valid before upgrade
    let authenticated = true;

    if !authenticated {
        let error_msg = json!({
            "type": "auth_error",
            "message": "Authentication required. Please provide a valid JWT token."
        });
        if let Err(e) = sender
            .send(Message::Text(error_msg.to_string().into()))
            .await
        {
            error!("Failed to send auth error message: {}", e);
        }
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
    let client_msg: ClientMessage = serde_json::from_str(message)?;

    let msg_type = if !client_msg.msg_type.is_empty() {
        &client_msg.msg_type
    } else {
        &client_msg.r#type
    };

    match msg_type.as_str() {
        "ping" => {
            debug!("Received ping from client");
        }
        "subscribe" => {
            debug!("Client subscription request: {:?}", client_msg);
        }
        "unsubscribe" => {
            debug!("Client unsubscription request: {:?}", client_msg);
        }
        _ => {
            warn!("Unknown message type: {}", msg_type);
        }
    }

    Ok(())
}
