use arbitrage_core::{types::Symbol, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use std::time::{Duration, Instant};

use crate::connector::ConnectorError;

/// Parse timestamp from various formats (milliseconds, seconds, ISO string)
pub fn parse_timestamp(value: &Value) -> Result<DateTime<Utc>> {
    match value {
        Value::Number(n) => {
            if let Some(timestamp) = n.as_i64() {
                // Try milliseconds first, then seconds
                if let Some(dt) = DateTime::from_timestamp(timestamp / 1000, ((timestamp % 1000) * 1_000_000) as u32) {
                    Ok(dt)
                } else if let Some(dt) = DateTime::from_timestamp(timestamp, 0) {
                    Ok(dt)
                } else {
                    Err(ConnectorError::ParsingError(format!("Invalid timestamp: {}", timestamp)).into())
                }
            } else {
                Err(ConnectorError::ParsingError("Timestamp is not an integer".to_string()).into())
            }
        },
        Value::String(s) => {
            s.parse::<DateTime<Utc>>()
                .map_err(|e| ConnectorError::ParsingError(format!("Failed to parse timestamp string: {}", e)).into())
        },
        _ => Err(ConnectorError::ParsingError("Invalid timestamp format".to_string()).into())
    }
}

/// Parse decimal from string or number
pub fn parse_decimal(value: &Value) -> Result<Decimal> {
    match value {
        Value::String(s) => {
            s.parse::<Decimal>()
                .map_err(|e| ConnectorError::ParsingError(format!("Failed to parse decimal: {}", e)).into())
        },
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Decimal::try_from(f)
                    .map_err(|e| ConnectorError::ParsingError(format!("Failed to convert float to decimal: {}", e)).into())
            } else {
                Err(ConnectorError::ParsingError("Number is not a valid float".to_string()).into())
            }
        },
        _ => Err(ConnectorError::ParsingError("Value is not a string or number".to_string()).into())
    }
}

/// Format symbol for different exchange formats
pub fn format_symbol(symbol: &Symbol, format: SymbolFormat) -> String {
    match format {
        SymbolFormat::Dash => format!("{}-{}", symbol.base.to_uppercase(), symbol.quote.to_uppercase()),
        SymbolFormat::Underscore => format!("{}_{}", symbol.base.to_uppercase(), symbol.quote.to_uppercase()),
        SymbolFormat::NoSeparator => format!("{}{}", symbol.base.to_uppercase(), symbol.quote.to_uppercase()),
        SymbolFormat::Lowercase => format!("{}{}", symbol.base.to_lowercase(), symbol.quote.to_lowercase()),
        SymbolFormat::Dot => format!("{}.{}", symbol.base.to_uppercase(), symbol.quote.to_uppercase()),
    }
}

/// Parse symbol from different exchange formats
pub fn parse_symbol(symbol_str: &str, format: SymbolFormat) -> Result<Symbol> {
    let (base, quote) = match format {
        SymbolFormat::Dash => {
            let parts: Vec<&str> = symbol_str.split('-').collect();
            if parts.len() != 2 {
                return Err(ConnectorError::InvalidSymbol(format!("Invalid dash format: {}", symbol_str)).into());
            }
            (parts[0], parts[1])
        },
        SymbolFormat::Underscore => {
            let parts: Vec<&str> = symbol_str.split('_').collect();
            if parts.len() != 2 {
                return Err(ConnectorError::InvalidSymbol(format!("Invalid underscore format: {}", symbol_str)).into());
            }
            (parts[0], parts[1])
        },
        SymbolFormat::Dot => {
            let parts: Vec<&str> = symbol_str.split('.').collect();
            if parts.len() != 2 {
                return Err(ConnectorError::InvalidSymbol(format!("Invalid dot format: {}", symbol_str)).into());
            }
            (parts[0], parts[1])
        },
        SymbolFormat::NoSeparator | SymbolFormat::Lowercase => {
            // Common quote currencies to try
            let quotes = ["USDT", "USDC", "BTC", "ETH", "BNB", "BUSD", "DAI"];
            let upper_symbol = symbol_str.to_uppercase();
            
            for quote in &quotes {
                if upper_symbol.ends_with(quote) {
                    let base = &upper_symbol[..upper_symbol.len() - quote.len()];
                    if !base.is_empty() {
                        return Ok(Symbol::new(base, *quote));
                    }
                }
            }
            
            return Err(ConnectorError::InvalidSymbol(format!("Cannot parse symbol: {}", symbol_str)).into());
        }
    };
    
    Ok(Symbol::new(base, quote))
}

/// Symbol format types for different exchanges
#[derive(Debug, Clone, Copy)]
pub enum SymbolFormat {
    Dash,         // BTC-USDT (OKX, Gate.io)
    Underscore,   // BTC_USDT 
    NoSeparator,  // BTCUSDT (Binance, Bybit)
    Lowercase,    // btcusdt
    Dot,          // BTC.USDT (some exchanges)
}

/// Exponential backoff for reconnection attempts
pub struct ExponentialBackoff {
    base_delay: Duration,
    max_delay: Duration,
    multiplier: f64,
    current_attempt: u32,
}

impl ExponentialBackoff {
    pub fn new(base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            base_delay,
            max_delay,
            multiplier: 2.0,
            current_attempt: 0,
        }
    }

    pub fn next_delay(&mut self) -> Duration {
        let delay = self.base_delay.mul_f64(self.multiplier.powi(self.current_attempt as i32));
        self.current_attempt += 1;
        delay.min(self.max_delay)
    }

    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }

    pub fn attempt(&self) -> u32 {
        self.current_attempt
    }
}

/// Rate limit detection from HTTP headers
pub fn detect_rate_limit(headers: &reqwest::header::HeaderMap) -> Option<RateLimitInfo> {
    let remaining = headers.get("x-ratelimit-remaining")
        .or_else(|| headers.get("x-rate-limit-remaining"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())?;

    let reset = headers.get("x-ratelimit-reset")
        .or_else(|| headers.get("x-rate-limit-reset"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|ts| DateTime::from_timestamp(ts, 0))?;

    Some(RateLimitInfo { remaining, reset })
}

#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub remaining: u32,
    pub reset: DateTime<Utc>,
}

/// Latency measurement utility
pub struct LatencyTracker {
    start_time: Instant,
}

impl LatencyTracker {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64() * 1000.0
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}