use arbitrage_core::{types::ExchangeId, types::Symbol, ArbitrageError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{error, warn};

/// Maximum number of retry attempts for JSON parsing
pub const MAX_PARSE_RETRIES: u32 = 3;

/// Base delay for exponential backoff in milliseconds
pub const PARSE_RETRY_BASE_DELAY_MS: u64 = 100;

/// Maximum delay for exponential backoff in milliseconds
pub const PARSE_RETRY_MAX_DELAY_MS: u64 = 1000;

/// Parse timestamp from various formats (milliseconds, seconds, ISO string)
pub fn parse_timestamp(value: &Value) -> Result<DateTime<Utc>> {
    match value {
        Value::Number(n) => {
            if let Some(timestamp) = n.as_i64() {
                // Try milliseconds first, then seconds
                if let Some(dt) = DateTime::from_timestamp(
                    timestamp / 1000,
                    ((timestamp % 1000) * 1_000_000) as u32,
                ) {
                    Ok(dt)
                } else if let Some(dt) = DateTime::from_timestamp(timestamp, 0) {
                    Ok(dt)
                } else {
                    Err(ArbitrageError::ParsingError(format!(
                        "Invalid timestamp: {}",
                        timestamp
                    )))
                }
            } else {
                Err(ArbitrageError::ParsingError(
                    "Timestamp is not a valid integer".to_string(),
                ))
            }
        }
        Value::String(s) => s.parse::<DateTime<Utc>>().map_err(|e| {
            ArbitrageError::ParsingError(format!("Failed to parse timestamp string: {}", e))
        }),
        _ => Err(ArbitrageError::ParsingError(
            "Invalid timestamp format".to_string(),
        )),
    }
}

/// Parse decimal from string or number
pub fn parse_decimal(value: &Value) -> Result<Decimal> {
    match value {
        Value::String(s) => s
            .parse::<Decimal>()
            .map_err(|e| ArbitrageError::ParsingError(format!("Failed to parse decimal: {}", e))),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Decimal::try_from(f).map_err(|e| {
                    ArbitrageError::ParsingError(format!(
                        "Failed to convert float to decimal: {}",
                        e
                    ))
                })
            } else {
                Err(ArbitrageError::ParsingError(
                    "Timestamp is not an integer".to_string(),
                ))
            }
        }
        _ => Err(ArbitrageError::ParsingError(
            "Value is not a string or number".to_string(),
        )),
    }
}

/// Format symbol for different exchange formats
pub fn format_symbol(symbol: &Symbol, format: SymbolFormat) -> String {
    match format {
        SymbolFormat::Dash => format!(
            "{}-{}",
            symbol.base.to_uppercase(),
            symbol.quote.to_uppercase()
        ),
        SymbolFormat::Underscore => format!(
            "{}_{}",
            symbol.base.to_uppercase(),
            symbol.quote.to_uppercase()
        ),
        SymbolFormat::NoSeparator => format!(
            "{}{}",
            symbol.base.to_uppercase(),
            symbol.quote.to_uppercase()
        ),
        SymbolFormat::Lowercase => format!(
            "{}{}",
            symbol.base.to_lowercase(),
            symbol.quote.to_lowercase()
        ),
        SymbolFormat::Dot => format!(
            "{}.{}",
            symbol.base.to_uppercase(),
            symbol.quote.to_uppercase()
        ),
        SymbolFormat::Slash => format!(
            "{}/{}",
            symbol.base.to_uppercase(),
            symbol.quote.to_uppercase()
        ),
    }
}

/// Parse symbol from different exchange formats
pub fn parse_symbol(symbol_str: &str, format: SymbolFormat) -> Result<Symbol> {
    let (base, quote) = match format {
        SymbolFormat::Dash => {
            let parts: Vec<&str> = symbol_str.split('-').collect();
            if parts.len() != 2 {
                return Err(ArbitrageError::InvalidSymbol(format!(
                    "Invalid dash format: {}",
                    symbol_str
                )));
            }
            (parts[0], parts[1])
        }
        SymbolFormat::Underscore => {
            let parts: Vec<&str> = symbol_str.split('_').collect();
            if parts.len() != 2 {
                return Err(ArbitrageError::InvalidSymbol(format!(
                    "Invalid underscore format: {}",
                    symbol_str
                )));
            }
            (parts[0], parts[1])
        }
        SymbolFormat::Dot => {
            let parts: Vec<&str> = symbol_str.split('.').collect();
            if parts.len() != 2 {
                return Err(ArbitrageError::InvalidSymbol(format!(
                    "Invalid dot format: {}",
                    symbol_str
                )));
            }
            (parts[0], parts[1])
        }
        SymbolFormat::Slash => {
            let parts: Vec<&str> = symbol_str.split('/').collect();
            if parts.len() != 2 {
                return Err(ArbitrageError::InvalidSymbol(format!(
                    "Invalid slash format: {}",
                    symbol_str
                )));
            }
            (parts[0], parts[1])
        }
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

            return Err(ArbitrageError::InvalidSymbol(format!(
                "Cannot parse symbol: {}",
                symbol_str
            )));
        }
    };

    Ok(Symbol::new(base, quote))
}

/// Symbol format types for different exchanges
#[derive(Debug, Clone, Copy)]
pub enum SymbolFormat {
    Dash,        // BTC-USDT (OKX, Gate.io)
    Underscore,  // BTC_USDT
    NoSeparator, // BTCUSDT (Binance, Bybit)
    Lowercase,   // btcusdt
    Dot,         // BTC.USDT (some exchanges)
    Slash,       // BTC/USDT (Kraken)
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
        let delay = self
            .base_delay
            .mul_f64(self.multiplier.powi(self.current_attempt as i32));
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
    let remaining = headers
        .get("x-ratelimit-remaining")
        .or_else(|| headers.get("x-rate-limit-remaining"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())?;

    let reset = headers
        .get("x-ratelimit-reset")
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

/// Parses JSON with retry logic and exponential backoff.
///
/// This function attempts to parse JSON data with up to 3 retries,
/// using exponential backoff between attempts. Detailed error logging
/// is performed for each failure.
///
/// # Arguments
///
/// * `data` - The raw bytes to parse as JSON
/// * `exchange_id` - The exchange ID for error logging and metrics
/// * `operation` - The operation name for error logging
/// * `parse_counter` - Optional atomic counter to track parsing failures
///
/// # Returns
///
/// `Ok(Value)` on successful parse, `Err(ArbitrageError)` on failure.
pub async fn parse_json_with_retry(
    data: &[u8],
    exchange_id: ExchangeId,
    operation: &str,
    parse_counter: Option<&AtomicU64>,
) -> Result<Value> {
    let mut attempt = 0u32;
    let max_attempts = MAX_PARSE_RETRIES;

    loop {
        match serde_json::from_slice::<Value>(data) {
            Ok(value) => return Ok(value),
            Err(e) => {
                attempt += 1;

                if attempt >= max_attempts {
                    error!(
                        target: "json_parsing",
                        "[{}] JSON parsing failed for {} after {} attempts: {}",
                        exchange_id, operation, attempt, e
                    );

                    if let Some(counter) = parse_counter {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }

                    return Err(ArbitrageError::ParsingError(format!(
                        "[{}] {} JSON parsing failed after {} attempts: {}",
                        exchange_id, operation, attempt, e
                    )));
                }

                let delay_ms = (PARSE_RETRY_BASE_DELAY_MS as f64 * 2.0f64.powi(attempt as i32 - 1))
                    .min(PARSE_RETRY_MAX_DELAY_MS as f64) as u64;

                warn!(
                    target: "json_parsing_retry",
                    "[{}] JSON parsing attempt {} for {} failed: {}, retrying in {}ms",
                    exchange_id, attempt, operation, e, delay_ms
                );

                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}

/// Tracks parsing failures per exchange
#[derive(Debug, Default)]
pub struct ParsingFailureTracker {
    failures: std::sync::Mutex<std::collections::HashMap<String, AtomicU64>>,
}

impl ParsingFailureTracker {
    /// Creates a new failure tracker
    pub fn new() -> Self {
        Self {
            failures: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Records a parsing failure for an exchange
    pub fn record_failure(&self, exchange: &str, operation: &str) {
        let key = format!("{}_{}", exchange, operation);
        let mut map = self.failures.lock().unwrap();
        let counter = map.entry(key).or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(1, Ordering::SeqCst);
    }

    /// Gets the failure count for an exchange and operation
    pub fn get_failure_count(&self, exchange: &str, operation: &str) -> u64 {
        let key = format!("{}_{}", exchange, operation);
        let map = self.failures.lock().unwrap();
        map.get(&key).map(|c| c.load(Ordering::SeqCst)).unwrap_or(0)
    }

    /// Gets all failure counts
    pub fn get_all_failures(&self) -> std::collections::HashMap<String, u64> {
        let map = self.failures.lock().unwrap();
        map.iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::SeqCst)))
            .collect()
    }
}
