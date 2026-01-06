use crate::connector::ConnectorError;
use std::time::Duration;
use tokio::time::sleep;
use std::collections::HashMap;
use arbitrage_core::types::ExchangeId;

/// Exponential backoff utility for reconnection attempts
pub struct ExponentialBackoff {
    base_delay_ms: u64,
    max_delay_ms: u64,
    multiplier: f64,
    current_attempt: u32,
}

impl ExponentialBackoff {
    pub fn new(base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            base_delay_ms,
            max_delay_ms,
            multiplier: 2.0,
            current_attempt: 0,
        }
    }

    /// Calculate delay for current attempt
    pub fn delay(&self) -> Duration {
        let delay_ms = (self.base_delay_ms as f64 * self.multiplier.powi(self.current_attempt as i32)) as u64;
        let capped_delay = delay_ms.min(self.max_delay_ms);
        Duration::from_millis(capped_delay)
    }

    /// Sleep for the calculated delay and increment attempt counter
    pub async fn sleep_and_increment(&mut self) {
        let delay = self.delay();
        self.current_attempt += 1;
        sleep(delay).await;
    }

    /// Reset attempt counter
    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }

    /// Get current attempt number
    pub fn attempt(&self) -> u32 {
        self.current_attempt
    }
}

/// Rate limiter for API requests
pub struct RateLimiter {
    requests_per_second: u32,
    last_request: Option<std::time::Instant>,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            requests_per_second,
            last_request: None,
        }
    }

    /// Wait if necessary to respect rate limit
    pub async fn wait_if_needed(&mut self) {
        if self.requests_per_second == 0 {
            return; // No rate limiting
        }

        let min_interval = Duration::from_millis(1000 / self.requests_per_second as u64);
        
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < min_interval {
                let wait_time = min_interval - elapsed;
                sleep(wait_time).await;
            }
        }

        self.last_request = Some(std::time::Instant::now());
    }
}

/// Unified rate limit manager for all exchanges
pub struct UnifiedRateLimitManager {
    exchange_limiters: HashMap<ExchangeId, RateLimiter>,
    rate_limit_violations: HashMap<ExchangeId, std::time::Instant>,
}

impl UnifiedRateLimitManager {
    pub fn new() -> Self {
        Self {
            exchange_limiters: HashMap::new(),
            rate_limit_violations: HashMap::new(),
        }
    }

    /// Add rate limiter for an exchange
    pub fn add_exchange(&mut self, exchange: ExchangeId, requests_per_second: u32) {
        self.exchange_limiters.insert(exchange, RateLimiter::new(requests_per_second));
    }

    /// Wait if necessary to respect rate limit for an exchange
    pub async fn wait_if_needed(&mut self, exchange: ExchangeId) -> Result<(), ConnectorError> {
        // Check if we're in a rate limit violation cooldown
        if let Some(violation_time) = self.rate_limit_violations.get(&exchange) {
            let cooldown_duration = Duration::from_secs(60); // 1 minute cooldown
            if violation_time.elapsed() < cooldown_duration {
                let remaining = cooldown_duration - violation_time.elapsed();
                tracing::warn!("Exchange {} in rate limit cooldown, waiting {:?}", exchange, remaining);
                sleep(remaining).await;
                self.rate_limit_violations.remove(&exchange);
            }
        }

        // Apply normal rate limiting
        if let Some(limiter) = self.exchange_limiters.get_mut(&exchange) {
            limiter.wait_if_needed().await;
        }

        Ok(())
    }

    /// Record a rate limit violation for an exchange
    pub fn record_rate_limit_violation(&mut self, exchange: ExchangeId) {
        tracing::error!("Rate limit violation detected for exchange: {}", exchange);
        self.rate_limit_violations.insert(exchange, std::time::Instant::now());
    }

    /// Check if an exchange is currently in rate limit violation cooldown
    pub fn is_in_cooldown(&self, exchange: ExchangeId) -> bool {
        if let Some(violation_time) = self.rate_limit_violations.get(&exchange) {
            violation_time.elapsed() < Duration::from_secs(60)
        } else {
            false
        }
    }
}

impl Default for UnifiedRateLimitManager {
    fn default() -> Self {
        let mut manager = Self::new();
        
        // Add default rate limits for supported exchanges
        manager.add_exchange(ExchangeId::ByBit, 10);      // 10 requests per second
        manager.add_exchange(ExchangeId::BingX, 5);       // 5 requests per second
        manager.add_exchange(ExchangeId::Hyperliquid, 20); // 20 requests per second
        
        manager
    }
}

/// Parse symbol from exchange-specific format
pub fn parse_symbol(exchange_symbol: &str, exchange: arbitrage_core::types::ExchangeId) -> Result<arbitrage_core::types::Symbol, ConnectorError> {
    use arbitrage_core::types::{ExchangeId, Symbol};

    match exchange {
        ExchangeId::ByBit => {
            // ByBit uses format like "BTCUSDT"
            if exchange_symbol.len() >= 6 {
                // Try common splits
                for split_pos in [3, 4, 5] {
                    if split_pos < exchange_symbol.len() {
                        let base = &exchange_symbol[..split_pos];
                        let quote = &exchange_symbol[split_pos..];
                        
                        // Check if quote is a known quote currency
                        if matches!(quote, "USDT" | "USDC" | "BTC" | "ETH" | "USD") {
                            return Ok(Symbol::new(base, quote));
                        }
                    }
                }
            }
            Err(ConnectorError::Parse(format!("Cannot parse ByBit symbol: {}", exchange_symbol)))
        }
        ExchangeId::BingX => {
            // BingX uses format like "BTC-USDT"
            if let Some(pos) = exchange_symbol.find('-') {
                let base = &exchange_symbol[..pos];
                let quote = &exchange_symbol[pos + 1..];
                Ok(Symbol::new(base, quote))
            } else {
                Err(ConnectorError::Parse(format!("Cannot parse BingX symbol: {}", exchange_symbol)))
            }
        }
        ExchangeId::Hyperliquid => {
            // Hyperliquid uses format like "BTC/USDT"
            if let Some(pos) = exchange_symbol.find('/') {
                let base = &exchange_symbol[..pos];
                let quote = &exchange_symbol[pos + 1..];
                Ok(Symbol::new(base, quote))
            } else {
                Err(ConnectorError::Parse(format!("Cannot parse Hyperliquid symbol: {}", exchange_symbol)))
            }
        }
        _ => {
            Err(ConnectorError::Parse(format!("Unsupported exchange for symbol parsing: {}", exchange)))
        }
    }
}

/// Format symbol to exchange-specific format
pub fn format_symbol(symbol: &arbitrage_core::types::Symbol, exchange: arbitrage_core::types::ExchangeId) -> String {
    use arbitrage_core::types::ExchangeId;

    match exchange {
        ExchangeId::ByBit => format!("{}{}", symbol.base, symbol.quote),
        ExchangeId::BingX => format!("{}-{}", symbol.base, symbol.quote),
        ExchangeId::Hyperliquid => format!("{}/{}", symbol.base, symbol.quote),
        _ => symbol.to_pair(), // Default to base/quote format
    }
}

/// Detect rate limit from WebSocket or HTTP response
pub fn detect_rate_limit_from_message(message: &str, exchange: ExchangeId) -> bool {
    match exchange {
        ExchangeId::ByBit => {
            message.contains("rate limit") || 
            message.contains("too many requests") ||
            message.contains("\"ret_code\":10006") // ByBit rate limit code
        }
        ExchangeId::BingX => {
            message.contains("rate limit") || 
            message.contains("too many requests") ||
            message.contains("\"code\":100429") // BingX rate limit code
        }
        ExchangeId::Hyperliquid => {
            message.contains("rate limit") || 
            message.contains("too many requests") ||
            message.contains("429") // HTTP 429 Too Many Requests
        }
        _ => {
            message.contains("rate limit") || message.contains("too many requests")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arbitrage_core::types::{ExchangeId, Symbol};

    #[test]
    fn test_exponential_backoff() {
        let mut backoff = ExponentialBackoff::new(1000, 60000);
        
        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.delay(), Duration::from_millis(1000));
        
        backoff.current_attempt = 1;
        assert_eq!(backoff.delay(), Duration::from_millis(2000));
        
        backoff.current_attempt = 10;
        assert_eq!(backoff.delay(), Duration::from_millis(60000)); // Capped at max
    }

    #[test]
    fn test_parse_symbol() {
        // ByBit
        let symbol = parse_symbol("BTCUSDT", ExchangeId::ByBit).unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");

        // BingX
        let symbol = parse_symbol("BTC-USDT", ExchangeId::BingX).unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");

        // Hyperliquid
        let symbol = parse_symbol("BTC/USDT", ExchangeId::Hyperliquid).unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_format_symbol() {
        let symbol = Symbol::new("BTC", "USDT");
        
        assert_eq!(format_symbol(&symbol, ExchangeId::ByBit), "BTCUSDT");
        assert_eq!(format_symbol(&symbol, ExchangeId::BingX), "BTC-USDT");
        assert_eq!(format_symbol(&symbol, ExchangeId::Hyperliquid), "BTC/USDT");
    }

    #[test]
    fn test_rate_limit_detection() {
        assert!(detect_rate_limit_from_message("rate limit exceeded", ExchangeId::ByBit));
        assert!(detect_rate_limit_from_message("too many requests", ExchangeId::BingX));
        assert!(detect_rate_limit_from_message("{\"ret_code\":10006}", ExchangeId::ByBit));
        assert!(detect_rate_limit_from_message("{\"code\":100429}", ExchangeId::BingX));
        assert!(!detect_rate_limit_from_message("normal message", ExchangeId::ByBit));
    }

    #[test]
    fn test_unified_rate_limit_manager() {
        let mut manager = UnifiedRateLimitManager::default();
        
        // Test cooldown functionality
        assert!(!manager.is_in_cooldown(ExchangeId::ByBit));
        
        manager.record_rate_limit_violation(ExchangeId::ByBit);
        assert!(manager.is_in_cooldown(ExchangeId::ByBit));
    }
}