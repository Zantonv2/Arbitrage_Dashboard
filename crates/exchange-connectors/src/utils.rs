use crate::connector::ConnectorError;
use std::time::Duration;
use tokio::time::sleep;

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
}