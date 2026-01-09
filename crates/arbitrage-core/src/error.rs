use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArbitrageError>;

#[derive(Error, Debug)]
pub enum ArbitrageError {
    #[error("Exchange error: {0}")]
    Exchange(String),
    
    #[error("Exchange connection error: {0}")]
    ExchangeConnection(String),
    
    #[error("Normalization error: {0}")]
    Normalization(String),
    
    #[error("Calculation error: {0}")]
    Calculation(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Decimal error: {0}")]
    Decimal(#[from] rust_decimal::Error),
    
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),
    
    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),
}