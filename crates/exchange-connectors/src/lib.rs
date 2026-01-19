pub mod connections;
pub mod connector;
pub mod events;
pub mod exchange_manager;
pub mod rate_limiter;
pub mod utils;

// Re-exports
pub use connections::{
    BitstampConnector, BybitConnector, GateioConnector, KrakenConnector, MEXCConnector,
    OKXConnector,
};
pub use connector::ExchangeConnector;
pub use events::{ConnectionEvent, MarketDataEvent};
pub use exchange_manager::{ExchangeManager, ExchangeManagerConfig};
pub use rate_limiter::{RateLimitConfig, RateLimiter, UnifiedRateLimitManager};
pub use utils::*;

use arbitrage_core::{types::ExchangeId, Result};

/// Create a connector for the specified exchange
pub fn create_connector(exchange_id: ExchangeId) -> Result<Box<dyn ExchangeConnector>> {
    match exchange_id {
        ExchangeId::OKX => Ok(Box::new(OKXConnector::new())),
        ExchangeId::ByBit => Ok(Box::new(BybitConnector::new())),
        ExchangeId::MEXC => Ok(Box::new(MEXCConnector::new())),
        ExchangeId::GateIo => Ok(Box::new(GateioConnector::new())),
        ExchangeId::Bitstamp => Ok(Box::new(BitstampConnector::new())),
        ExchangeId::Kraken => Ok(Box::new(KrakenConnector::new())),
        _ => Err(arbitrage_core::ArbitrageError::Validation(format!(
            "Exchange connector not implemented: {}",
            exchange_id
        ))),
    }
}
