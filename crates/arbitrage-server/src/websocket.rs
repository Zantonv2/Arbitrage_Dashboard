use crate::server::AppState;
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// WebSocket handler - no authentication required for local-only use
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    debug!("WebSocket connection requested");

    // No authentication needed for local-only use
    debug!("WebSocket connection accepted (no auth required for local use)");

    // Proceed with WebSocket upgrade
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (sender, receiver) = socket.split();
    let client_id = uuid::Uuid::new_v4();
    let sender = Arc::new(Mutex::new(sender));

    info!("WebSocket client connected: {}", client_id);

    // Subscribe to signal broadcasts
    let mut signal_rx = state.arbitrage_engine.subscribe();

    // Clone sender for receive task
    let sender_for_recv = Arc::clone(&sender);
    
    // Spawn a task to handle incoming messages
    let recv_task = tokio::spawn(async move {
        let mut receiver = receiver;
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received message from client {}: {}", client_id, text);
                }
                Ok(Message::Binary(data)) => {
                    debug!("Received binary data from client {}: {} bytes", client_id, data.len());
                }
                Ok(Message::Ping(ping)) => {
                    if let Err(e) = sender_for_recv.lock().await.send(Message::Pong(ping)).await {
                        error!("Failed to send pong to client {}: {}", client_id, e);
                        return;
                    }
                }
                Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    info!("Client {} disconnected", client_id);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error for client {}: {}", client_id, e);
                    break;
                }
            }
        }
    });

    // Clone sender for broadcast task
    let sender_for_broadcast = Arc::clone(&sender);
    
    // Broadcast signals to the client
    let broadcast_task = tokio::spawn(async move {
        while let Ok(signal) = signal_rx.recv().await {
            let signal_json = json!({
                "type": "signal",
                "data": {
                    "id": signal.id.to_string(),
                    "symbol": signal.symbol.to_pair(),
                    "buy_exchange": signal.buy_exchange.to_string(),
                    "sell_exchange": signal.sell_exchange.to_string(),
                    "buy_price": signal.buy_price.to_string(),
                    "sell_price": signal.sell_price.to_string(),
                    "gross_profit_percent": (signal.gross_profit_percent * rust_decimal::Decimal::from(100)).to_string(),
                    "net_profit_percent": (signal.net_profit_percent * rust_decimal::Decimal::from(100)).to_string(),
                    "confidence": signal.confidence.to_string(),
                    "recommended_size": signal.recommended_size.to_string(),
                    "created_at": signal.created_at.to_rfc3339()
                }
            });

            if let Err(e) = sender_for_broadcast.lock().await.send(Message::Text(signal_json.to_string().into())).await {
                error!("Failed to broadcast signal to client {}: {}", client_id, e);
                break;
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = recv_task => {
            info!("Client {} receive task ended", client_id);
        }
        _ = broadcast_task => {
            info!("Client {} broadcast task ended", client_id);
        }
    }

    info!("WebSocket client disconnected: {}", client_id);
}
