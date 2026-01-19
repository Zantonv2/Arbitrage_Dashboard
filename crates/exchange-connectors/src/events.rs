use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, Symbol};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::connector::{FundingRate, TickerData};

/// Events emitted by exchange connectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionEvent {
    /// Market data update
    MarketData(MarketDataEvent),

    /// Connection status change
    StatusChange {
        exchange: ExchangeId,
        old_status: ConnectionStatus,
        new_status: ConnectionStatus,
        timestamp: DateTime<Utc>,
    },

    /// Error occurred
    Error {
        exchange: ExchangeId,
        error: String,
        timestamp: DateTime<Utc>,
    },

    /// Subscription confirmed
    SubscriptionConfirmed {
        exchange: ExchangeId,
        symbols: Vec<Symbol>,
        data_type: String,
        timestamp: DateTime<Utc>,
    },

    /// Heartbeat/ping received
    Heartbeat {
        exchange: ExchangeId,
        timestamp: DateTime<Utc>,
    },

    /// Rate limit warning
    RateLimitWarning {
        exchange: ExchangeId,
        remaining: u32,
        reset_time: DateTime<Utc>,
        timestamp: DateTime<Utc>,
    },
}

/// Market data events from exchanges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketDataEvent {
    /// Order book update
    OrderBook {
        exchange: ExchangeId,
        order_book: OrderBook,
        timestamp: DateTime<Utc>,
    },

    /// Ticker update
    Ticker {
        exchange: ExchangeId,
        ticker: TickerData,
        timestamp: DateTime<Utc>,
    },

    /// Trade executed
    Trade {
        exchange: ExchangeId,
        symbol: Symbol,
        price: Decimal,
        quantity: Decimal,
        side: TradeSide,
        trade_id: String,
        timestamp: DateTime<Utc>,
    },

    /// Funding rate update
    FundingRate {
        exchange: ExchangeId,
        funding_rate: FundingRate,
        timestamp: DateTime<Utc>,
    },

    /// 24h statistics update
    Statistics {
        exchange: ExchangeId,
        symbol: Symbol,
        volume_24h: Decimal,
        price_change_24h: Decimal,
        high_24h: Decimal,
        low_24h: Decimal,
        timestamp: DateTime<Utc>,
    },

    /// Raw market data event
    Raw {
        exchange: ExchangeId,
        data: Vec<u8>,
        timestamp: DateTime<Utc>,
    },
}

/// Trade side enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// Exchange-specific event wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeEvent {
    pub exchange: ExchangeId,
    pub event: ConnectionEvent,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
}

impl ExchangeEvent {
    pub fn new(exchange: ExchangeId, event: ConnectionEvent, sequence: u64) -> Self {
        Self {
            exchange,
            event,
            sequence,
            timestamp: Utc::now(),
        }
    }
}

/// Event statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub exchange: ExchangeId,
    pub total_events: u64,
    pub order_book_updates: u64,
    pub ticker_updates: u64,
    pub trade_updates: u64,
    pub funding_rate_updates: u64,
    pub errors: u64,
    pub last_event_time: Option<DateTime<Utc>>,
    pub events_per_second: f64,
}

impl Default for EventStats {
    fn default() -> Self {
        Self {
            exchange: ExchangeId::OKX,
            total_events: 0,
            order_book_updates: 0,
            ticker_updates: 0,
            trade_updates: 0,
            funding_rate_updates: 0,
            errors: 0,
            last_event_time: None,
            events_per_second: 0.0,
        }
    }
}

impl EventStats {
    pub fn update(&mut self, event: &ConnectionEvent) {
        self.total_events += 1;
        self.last_event_time = Some(Utc::now());

        match event {
            ConnectionEvent::MarketData(market_event) => match market_event {
                MarketDataEvent::OrderBook { .. } => self.order_book_updates += 1,
                MarketDataEvent::Ticker { .. } => self.ticker_updates += 1,
                MarketDataEvent::Trade { .. } => self.trade_updates += 1,
                MarketDataEvent::FundingRate { .. } => self.funding_rate_updates += 1,
                MarketDataEvent::Statistics { .. } => self.ticker_updates += 1,
                MarketDataEvent::Raw { .. } => {}
            },
            ConnectionEvent::Error { .. } => self.errors += 1,
            _ => {}
        }
    }
}
