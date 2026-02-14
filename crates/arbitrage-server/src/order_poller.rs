//! # Order Polling Module
//!
//! Provides order confirmation polling for arbitrage trading. Polls order status
//! at regular intervals until filled, cancelled, or timeout.
//!
//! # Example
//!
//! ```rust,ignore
//! use arbitrage_server::order_poller::{OrderPoller, OrderPollerConfig};
//! use exchange_connectors::exchange_manager::ExchangeManager;
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//!
//! // OrderPoller requires an ExchangeManager instance
//! let config = OrderPollerConfig::default();
//! ```

use arbitrage_core::Result;
use exchange_connectors::{
    connector::{OrderStatus, OrderStatusType},
    exchange_manager::ExchangeManager,
};
use rust_decimal::Decimal;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Configuration for order polling
#[derive(Debug, Clone)]
pub struct OrderPollerConfig {
    /// Polling interval in milliseconds
    pub poll_interval_ms: u64,
    /// Maximum time to wait for order fill in milliseconds
    pub max_wait_ms: u64,
    /// Maximum number of polling attempts before giving up
    pub max_poll_attempts: u32,
}

impl Default for OrderPollerConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 100,
            max_wait_ms: 5000,
            max_poll_attempts: 50,
        }
    }
}

impl OrderPollerConfig {
    /// Create a new config with custom values
    pub fn new(poll_interval_ms: u64, max_wait_ms: u64) -> Self {
        Self {
            poll_interval_ms,
            max_wait_ms,
            max_poll_attempts: (max_wait_ms / poll_interval_ms.max(1)) as u32,
        }
    }
}

/// Result of order polling
#[derive(Debug, Clone)]
pub struct PollResult {
    /// Whether the order was filled
    pub filled: bool,
    /// Current order status
    pub status: OrderStatusType,
    /// Filled quantity
    pub filled_quantity: Decimal,
    /// Average fill price
    pub average_price: Option<Decimal>,
    /// Number of polls performed
    pub poll_count: u32,
    /// Time spent waiting (ms)
    pub wait_time_ms: u64,
    /// Error message if polling failed
    pub error: Option<String>,
}

impl PollResult {
    /// Create a successful filled result
    pub fn filled(filled_qty: Decimal, avg_price: Option<Decimal>, polls: u32, wait_ms: u64) -> Self {
        Self {
            filled: true,
            status: OrderStatusType::Filled,
            filled_quantity: filled_qty,
            average_price: avg_price,
            poll_count: polls,
            wait_time_ms: wait_ms,
            error: None,
        }
    }

    /// Create a successful but not filled result
    pub fn not_filled(status: OrderStatusType, polls: u32, wait_ms: u64) -> Self {
        Self {
            filled: false,
            status,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            poll_count: polls,
            wait_time_ms: wait_ms,
            error: None,
        }
    }

    /// Create an error result
    pub fn error(msg: String) -> Self {
        Self {
            filled: false,
            status: OrderStatusType::New,
            filled_quantity: Decimal::ZERO,
            average_price: None,
            poll_count: 0,
            wait_time_ms: 0,
            error: Some(msg),
        }
    }
}

/// Order poller service
pub struct OrderPoller {
    config: OrderPollerConfig,
    exchange_manager: Arc<Mutex<ExchangeManager>>,
}

impl OrderPoller {
    /// Create a new order poller
    pub fn new(
        config: OrderPollerConfig,
        exchange_manager: Arc<Mutex<ExchangeManager>>,
    ) -> Self {
        Self {
            config,
            exchange_manager,
        }
    }

    /// Create a new order poller with default config
    pub fn with_defaults(exchange_manager: Arc<Mutex<ExchangeManager>>) -> Self {
        Self::new(OrderPollerConfig::default(), exchange_manager)
    }

    /// Poll for order fill confirmation
    ///
    /// Continuously polls the exchange for order status until:
    /// - Order is filled
    /// - Order is cancelled/rejected/expired
    /// - Timeout is reached
    /// - Error occurs
    pub async fn poll_for_fill(
        &self,
        exchange: &arbitrage_core::types::ExchangeId,
        order_id: &str,
    ) -> PollResult {
        let start_time = std::time::Instant::now();
        let mut poll_count: u32 = 0;
        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);
        let max_wait = Duration::from_millis(self.config.max_wait_ms);

        debug!(
            "Starting order fill polling: {} on {} (max wait: {}ms)",
            order_id,
            exchange,
            self.config.max_wait_ms
        );

        loop {
            // Check timeout
            if start_time.elapsed() > max_wait {
                warn!(
                    "Order {} polling timeout after {} polls ({}ms)",
                    order_id,
                    poll_count,
                    start_time.elapsed().as_millis()
                );
                return PollResult::not_filled(
                    OrderStatusType::New,
                    poll_count,
                    start_time.elapsed().as_millis() as u64,
                );
            }

            // Get order status
            let order_status = self
                .get_order_status(exchange, order_id)
                .await;

            poll_count += 1;

            match order_status {
                Ok(status) => {
                    debug!(
                        "Order {} status: {:?} (poll {}/{})",
                        order_id,
                        status.status,
                        poll_count,
                        self.config.max_poll_attempts
                    );

                    // Check if order is in terminal state
                    match status.status {
                        OrderStatusType::Filled => {
                            debug!(
                                "Order {} filled after {} polls ({}ms)",
                                order_id,
                                poll_count,
                                start_time.elapsed().as_millis()
                            );
                            return PollResult::filled(
                                status.filled_quantity,
                                status.average_price,
                                poll_count,
                                start_time.elapsed().as_millis() as u64,
                            );
                        }
                        OrderStatusType::PartiallyFilled => {
                            // Continue polling for full fill
                            debug!(
                                "Order {} partially filled: {}/{} (poll {}/{})",
                                order_id,
                                status.filled_quantity,
                                status.quantity,
                                poll_count,
                                self.config.max_poll_attempts
                            );
                        }
                        OrderStatusType::Cancelled => {
                            warn!("Order {} cancelled", order_id);
                            return PollResult::not_filled(
                                OrderStatusType::Cancelled,
                                poll_count,
                                start_time.elapsed().as_millis() as u64,
                            );
                        }
                        OrderStatusType::Rejected => {
                            warn!("Order {} rejected", order_id);
                            return PollResult::not_filled(
                                OrderStatusType::Rejected,
                                poll_count,
                                start_time.elapsed().as_millis() as u64,
                            );
                        }
                        OrderStatusType::Expired => {
                            warn!("Order {} expired", order_id);
                            return PollResult::not_filled(
                                OrderStatusType::Expired,
                                poll_count,
                                start_time.elapsed().as_millis() as u64,
                            );
                        }
                        OrderStatusType::New => {
                            // Continue polling
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get order status for {}: {}", order_id, e);
                    // Don't fail immediately on error - continue polling
                    // But check if we've exceeded max attempts
                    if poll_count >= self.config.max_poll_attempts {
                        return PollResult::error(format!(
                            "Max poll attempts reached with error: {}",
                            e
                        ));
                    }
                }
            }

            // Check max attempts
            if poll_count >= self.config.max_poll_attempts {
                warn!(
                    "Order {} max poll attempts reached ({})",
                    order_id, poll_count
                );
                return PollResult::not_filled(
                    OrderStatusType::New,
                    poll_count,
                    start_time.elapsed().as_millis() as u64,
                );
            }

            // Wait before next poll
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Get order status from exchange
    async fn get_order_status(
        &self,
        exchange: &arbitrage_core::types::ExchangeId,
        order_id: &str,
    ) -> Result<OrderStatus> {
        let em = self.exchange_manager.lock().await;
        em.get_order_status(exchange, order_id).await
    }
}

/// Order execution helper that combines placement with polling
pub struct OrderExecutionHelper {
    poller: OrderPoller,
}

impl OrderExecutionHelper {
    /// Create a new order execution helper
    pub fn new(exchange_manager: Arc<Mutex<ExchangeManager>>) -> Self {
        Self {
            poller: OrderPoller::with_defaults(exchange_manager),
        }
    }

    /// Create with custom polling config
    pub fn with_config(
        config: OrderPollerConfig,
        exchange_manager: Arc<Mutex<ExchangeManager>>,
    ) -> Self {
        Self {
            poller: OrderPoller::new(config, exchange_manager),
        }
    }

    /// Get reference to the poller
    pub fn poller(&self) -> &OrderPoller {
        &self.poller
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrderPollerConfig::default();
        assert_eq!(config.poll_interval_ms, 100);
        assert_eq!(config.max_wait_ms, 5000);
        assert_eq!(config.max_poll_attempts, 50);
    }

    #[test]
    fn test_custom_config() {
        let config = OrderPollerConfig::new(50, 2000);
        assert_eq!(config.poll_interval_ms, 50);
        assert_eq!(config.max_wait_ms, 2000);
        assert_eq!(config.max_poll_attempts, 40);
    }

    #[test]
    fn test_poll_result_filled() {
        let result = PollResult::filled(
            Decimal::from(1),
            Some(Decimal::from(50000)),
            5,
            500,
        );
        assert!(result.filled);
        assert_eq!(result.status, OrderStatusType::Filled);
        assert_eq!(result.filled_quantity, Decimal::from(1));
        assert_eq!(result.poll_count, 5);
    }

    #[test]
    fn test_poll_result_not_filled() {
        let result = PollResult::not_filled(OrderStatusType::New, 10, 1000);
        assert!(!result.filled);
        assert_eq!(result.status, OrderStatusType::New);
        assert_eq!(result.poll_count, 10);
    }

    #[test]
    fn test_poll_result_error() {
        let result = PollResult::error("Network error".to_string());
        assert!(!result.filled);
        assert!(result.error.is_some());
        assert_eq!(result.error.unwrap(), "Network error");
    }
}
