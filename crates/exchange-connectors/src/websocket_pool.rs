use arbitrage_core::types::Symbol;
use dashmap::DashMap;
use dashmap::DashSet;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    max_attempts: u32,
    initial_delay_ms: u64,
    max_delay_ms: u64,
    backoff_multiplier: f64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
        }
    }
}

impl ReconnectPolicy {
    pub fn new(
        max_attempts: u32,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_attempts,
            initial_delay_ms,
            max_delay_ms,
            backoff_multiplier,
        }
    }

    pub fn next_delay(&self, attempt: u32) -> Duration {
        let delay = self.initial_delay_ms as f64 * self.backoff_multiplier.powf(attempt as f64);
        Duration::from_millis(delay.min(self.max_delay_ms as f64) as u64)
    }
}

#[derive(Debug)]
pub enum WebSocketMessage {
    Subscribe(Vec<Symbol>),
    Unsubscribe(Vec<Symbol>),
    Text(String),
    Ping,
    Disconnect,
}

#[derive(Debug, Clone)]
pub enum WebSocketEvent {
    Message(Value),
    Connected,
    Disconnected,
    Error(String),
}

struct WebSocketPoolInner {
    url: String,
    state: Arc<DashMap<(), ConnectionState>>,
    subscriptions: Arc<DashSet<Symbol>>,
    reconnect_policy: ReconnectPolicy,
    reconnect_attempts: Arc<DashMap<String, u32>>,
    message_sender: mpsc::UnboundedSender<WebSocketMessage>,
    event_sender: broadcast::Sender<WebSocketEvent>,
    heartbeat_interval_ms: u64,
    last_heartbeat: Arc<DashMap<(), Instant>>,
    message_queue: Arc<DashMap<usize, (String, Instant)>>,
    queue_counter: Arc<DashMap<(), usize>>,
}

pub struct WebSocketPool {
    inner: Arc<WebSocketPoolInner>,
    _handle: tokio::task::JoinHandle<()>,
}

impl WebSocketPool {
    pub async fn connect(url: &str) -> Result<Self, String> {
        let (message_sender, mut message_receiver) = mpsc::unbounded_channel::<WebSocketMessage>();
        let (event_sender, _) = broadcast::channel(1000);

        let inner = Arc::new(WebSocketPoolInner {
            url: url.to_string(),
            state: Arc::new(DashMap::new()),
            subscriptions: Arc::new(DashSet::new()),
            reconnect_policy: ReconnectPolicy::default(),
            reconnect_attempts: Arc::new(DashMap::new()),
            message_sender,
            event_sender,
            heartbeat_interval_ms: 30000,
            last_heartbeat: Arc::new(DashMap::new()),
            message_queue: Arc::new(DashMap::new()),
            queue_counter: Arc::new(DashMap::new()),
        });

        inner.state.insert((), ConnectionState::Disconnected);

        let inner_clone = inner.clone();
        let handle = tokio::spawn(async move {
            Self::run_connection(inner_clone, &mut message_receiver).await;
        });

        let pool = WebSocketPool {
            inner,
            _handle: handle,
        };

        Ok(pool)
    }

    async fn run_connection(
        inner: Arc<WebSocketPoolInner>,
        message_receiver: &mut mpsc::UnboundedReceiver<WebSocketMessage>,
    ) {
        let mut reconnect_attempt = 0u32;

        loop {
            let current_state = inner
                .state
                .get(&())
                .map(|r| r.clone())
                .unwrap_or(ConnectionState::Disconnected);

            if current_state == ConnectionState::Disconnected {
                break;
            }

            match Self::establish_connection(&inner).await {
                Ok((mut ws_stream, _)) => {
                    info!("WebSocket connected to {}", inner.url);
                    inner.state.insert((), ConnectionState::Connected);
                    reconnect_attempt = 0;
                    inner.reconnect_attempts.insert(inner.url.clone(), 0);

                    let _ = inner.event_sender.send(WebSocketEvent::Connected);

                    if let Err(e) =
                        Self::handle_connection(&inner, &mut ws_stream, message_receiver).await
                    {
                        error!("WebSocket connection error: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to connect to {}: {}", inner.url, e);
                    inner.state.insert(
                        (),
                        ConnectionState::Error(format!("Connection failed: {}", e)),
                    );
                    let _ = inner
                        .event_sender
                        .send(WebSocketEvent::Error(format!("Connection failed: {}", e)));

                    reconnect_attempt += 1;
                    inner
                        .reconnect_attempts
                        .insert(inner.url.clone(), reconnect_attempt);

                    if reconnect_attempt >= inner.reconnect_policy.max_attempts {
                        error!("Max reconnect attempts reached for {}", inner.url);
                        let _ = inner.event_sender.send(WebSocketEvent::Error(
                            "Max reconnect attempts reached".to_string(),
                        ));
                        break;
                    }

                    inner.state.insert((), ConnectionState::Reconnecting);
                }
            }

            if inner.state.get(&()).map(|r| r.clone()) == Some(ConnectionState::Disconnected) {
                break;
            }

            let delay = inner.reconnect_policy.next_delay(reconnect_attempt);
            info!("Reconnecting to {} in {:?}", inner.url, delay);
            tokio::time::sleep(delay).await;
        }

        inner.state.insert((), ConnectionState::Disconnected);
        let _ = inner.event_sender.send(WebSocketEvent::Disconnected);
    }

    async fn establish_connection(
        inner: &Arc<WebSocketPoolInner>,
    ) -> Result<
        (
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            tokio_tungstenite::tungstenite::http::Response<Option<Vec<u8>>>,
        ),
        String,
    > {
        inner.state.insert((), ConnectionState::Connecting);

        connect_async(&inner.url)
            .await
            .map_err(|e| format!("WebSocket connection failed: {}", e))
    }

    async fn handle_connection(
        inner: &Arc<WebSocketPoolInner>,
        ws_stream: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        message_receiver: &mut mpsc::UnboundedReceiver<WebSocketMessage>,
    ) -> Result<(), String> {
        let heartbeat_interval = Duration::from_millis(inner.heartbeat_interval_ms);
        let mut last_heartbeat = Instant::now();
        let mut subscription_check_interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            let current_state = inner
                .state
                .get(&())
                .map(|r| r.clone())
                .unwrap_or(ConnectionState::Disconnected);
            if current_state == ConnectionState::Disconnected {
                break;
            }

            let timeout_duration = Duration::from_secs(30);

            tokio::select! {
                Some(message) = message_receiver.recv() => {
                    if let Err(e) = Self::handle_outgoing_message(ws_stream, &message).await {
                        error!("Failed to send message: {}", e);
                    }
                }
                result = timeout(timeout_duration, ws_stream.next()) => {
                    match result {
                        Ok(Some(Ok(message))) => {
                            if let Err(e) = Self::handle_incoming_message(inner, message).await {
                                error!("Failed to handle message: {}", e);
                            }
                            last_heartbeat = Instant::now();
                        }
                        Ok(Some(Err(e))) => {
                            return Err(format!("WebSocket error: {}", e));
                        }
                        Ok(None) => {
                            info!("WebSocket stream ended");
                            return Ok(());
                        }
                        Err(_) => {
                            warn!("WebSocket timeout, sending ping");
                            if let Err(e) = ws_stream.send(Message::Ping(vec![].into())).await {
                                return Err(format!("Failed to send ping: {}", e));
                            }
                        }
                    }
                }
                _ = subscription_check_interval.tick() => {
                    Self::resend_subscriptions(inner, ws_stream).await;
                }
                _ = tokio::time::sleep(heartbeat_interval) => {
                    if last_heartbeat.elapsed() > heartbeat_interval {
                        let _ = ws_stream.send(Message::Ping(vec![].into())).await;
                        last_heartbeat = Instant::now();
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_outgoing_message(
        ws_stream: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        message: &WebSocketMessage,
    ) -> Result<(), String> {
        match message {
            WebSocketMessage::Subscribe(symbols) => {
                for symbol in symbols {
                    ws_stream
                        .send(Message::Text(format!("subscribe:{}", symbol).into()))
                        .await
                        .map_err(|e| format!("Failed to send subscribe: {}", e))?;
                }
            }
            WebSocketMessage::Unsubscribe(symbols) => {
                for symbol in symbols {
                    ws_stream
                        .send(Message::Text(format!("unsubscribe:{}", symbol).into()))
                        .await
                        .map_err(|e| format!("Failed to send unsubscribe: {}", e))?;
                }
            }
            WebSocketMessage::Text(text) => {
                ws_stream
                    .send(Message::Text(text.clone().into()))
                    .await
                    .map_err(|e| format!("Failed to send text: {}", e))?;
            }
            WebSocketMessage::Ping => {
                ws_stream
                    .send(Message::Ping(vec![].into()))
                    .await
                    .map_err(|e| format!("Failed to send ping: {}", e))?;
            }
            WebSocketMessage::Disconnect => {
                ws_stream.close(None).await.ok();
                return Ok(());
            }
        }
        Ok(())
    }

    async fn handle_incoming_message(
        inner: &Arc<WebSocketPoolInner>,
        message: tokio_tungstenite::tungstenite::Message,
    ) -> Result<(), String> {
        match message {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                debug!("Received message: {}", text);

                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    let new_counter = {
                        let counter = inner.queue_counter.entry(()).or_insert(0);
                        *counter + 1
                    };
                    inner
                        .message_queue
                        .insert(new_counter, (text.to_string(), Instant::now()));

                    let _ = inner.event_sender.send(WebSocketEvent::Message(value));
                } else if text.starts_with("pong") {
                    debug!("Received pong");
                } else {
                    let new_counter = {
                        let counter = inner.queue_counter.entry(()).or_insert(0);
                        *counter + 1
                    };
                    inner
                        .message_queue
                        .insert(new_counter, (text.to_string(), Instant::now()));

                    if let Ok(value) = serde_json::from_str::<Value>(&text) {
                        let _ = inner.event_sender.send(WebSocketEvent::Message(value));
                    }
                }
            }
            tokio_tungstenite::tungstenite::Message::Ping(data) => {
                debug!("Received ping");
            }
            tokio_tungstenite::tungstenite::Message::Pong(_) => {
                debug!("Received pong");
            }
            tokio_tungstenite::tungstenite::Message::Close(_) => {
                info!("WebSocket connection closed by server");
                return Err("Connection closed".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    async fn resend_subscriptions(
        inner: &Arc<WebSocketPoolInner>,
        ws_stream: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) {
        let subscriptions: Vec<Symbol> = inner
            .subscriptions
            .iter()
            .map(|s| s.key().clone())
            .collect();

        if !subscriptions.is_empty() {
            let sub_message = format!(
                "subscribe:{}",
                subscriptions
                    .iter()
                    .map(|s| format!("{}/{}", s.base, s.quote))
                    .collect::<Vec<_>>()
                    .join(",")
            );

            if let Err(e) = ws_stream.send(Message::Text(sub_message.into())).await {
                warn!("Failed to resend subscriptions: {}", e);
            }
        }
    }

    pub async fn subscribe(&self, symbols: &[Symbol]) -> Result<(), String> {
        for symbol in symbols {
            self.inner.subscriptions.insert(symbol.clone());
        }

        self.send_message(WebSocketMessage::Subscribe(symbols.to_vec()))
            .await
    }

    pub async fn unsubscribe(&self, symbols: &[Symbol]) -> Result<(), String> {
        for symbol in symbols {
            self.inner.subscriptions.remove(symbol);
        }

        self.send_message(WebSocketMessage::Unsubscribe(symbols.to_vec()))
            .await
    }

    pub fn on_message(&self) -> broadcast::Receiver<WebSocketEvent> {
        self.inner.event_sender.subscribe()
    }

    pub fn state(&self) -> ConnectionState {
        self.inner
            .state
            .get(&())
            .map(|r| r.clone())
            .unwrap_or(ConnectionState::Disconnected)
    }

    pub async fn disconnect(&self) {
        self.inner.state.insert((), ConnectionState::Disconnected);
        self.send_message(WebSocketMessage::Disconnect).await.ok();
    }

    async fn send_message(&self, message: WebSocketMessage) -> Result<(), String> {
        self.inner
            .message_sender
            .send(message)
            .map_err(|e| format!("Failed to send message: {}", e))
    }

    pub fn queued_messages(&self) -> usize {
        self.inner.message_queue.len()
    }

    pub fn is_connected(&self) -> bool {
        self.state() == ConnectionState::Connected
    }
}

impl Drop for WebSocketPool {
    fn drop(&mut self) {
        self.inner.state.insert((), ConnectionState::Disconnected);
    }
}
