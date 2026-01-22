use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArbitrageError>;

#[derive(Error, Debug)]
pub enum ArbitrageError {
    #[error("Exchange error: {0}")]
    Exchange(String),

    #[error("Exchange connection error: {0}")]
    ExchangeConnection(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),

    #[error("Parsing error: {0}")]
    ParsingError(String),

    #[error("Exchange API error: {code} - {message}")]
    ExchangeApiError { code: i32, message: String },

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Normalization error: {0}")]
    Normalization(String),

    #[error("Calculation error: {0}")]
    Calculation(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Circuit breaker is open: {0}")]
    CircuitBreakerOpen(String),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Broadcast send error: {0}")]
    BroadcastSend(#[from] tokio::sync::broadcast::error::SendError<()>),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Decimal error: {0}")]
    Decimal(#[from] rust_decimal::Error),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Lock error: {0}")]
    LockError(String),
}
