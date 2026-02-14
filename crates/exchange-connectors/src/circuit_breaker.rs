//! # Circuit Breaker Module
//!
//! Provides circuit breaker pattern implementation for exchange connectors.
//! Protects against cascading failures by tracking errors and temporarily
//! disabling exchanges that are experiencing issues.
//!
//! # Circuit States
//!
//! - **Closed**: Normal operation, requests pass through
//! - **Open**: Circuit is open, requests fail fast without attempting
//! - **Half-Open**: Testing if exchange has recovered, limited requests allowed

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation, requests pass through
    Closed,
    /// Circuit is open, requests fail fast
    Open,
    /// Testing if exchange has recovered
    HalfOpen,
}

impl Default for CircuitState {
    fn default() -> Self {
        CircuitState::Closed
    }
}

/// Configuration for circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Number of successes needed to close the circuit from half-open
    pub success_threshold: u32,
    /// Duration to wait before trying half-open
    pub open_duration: Duration,
    /// Maximum requests per second allowed in half-open state
    pub half_open_rate_limit: u32,
    /// Whether circuit breaker is enabled
    pub enabled: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_duration: Duration::from_secs(30),
            half_open_rate_limit: 1,
            enabled: true,
        }
    }
}

/// Circuit breaker for exchange connectors
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    /// Current circuit state
    state: RwLock<CircuitState>,
    /// Number of consecutive failures
    failures: AtomicU32,
    /// Number of consecutive successes (for half-open recovery)
    successes: AtomicU32,
    /// Timestamp when circuit was opened
    last_failure_time: AtomicU64,
    /// Last attempt time for half-open rate limiting
    last_attempt_time: AtomicU64,
    /// Total requests handled
    total_requests: AtomicU64,
    /// Total successful requests
    total_successes: AtomicU64,
    /// Total failed requests
    total_failures: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failures: AtomicU32::new(0),
            successes: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
            last_attempt_time: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            total_successes: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
        }
    }

    /// Create a new circuit breaker with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Check if a request can be executed
    pub async fn can_execute(&self) -> bool {
        if !self.config.enabled {
            return true;
        }

        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let last_failure = self.last_failure_time.load(Ordering::Relaxed);
                if last_failure > 0 {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    let elapsed = Duration::from_secs(now.saturating_sub(last_failure));
                    if elapsed >= self.config.open_duration {
                        let mut state = self.state.write().await;
                        *state = CircuitState::HalfOpen;
                        self.successes.store(0, Ordering::Relaxed);
                        info!("Circuit breaker transitioning to half-open state");
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                let last_attempt = self.last_attempt_time.load(Ordering::Relaxed);
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                let elapsed = now.saturating_sub(last_attempt);

                if elapsed >= (60 / self.config.half_open_rate_limit) as u64 {
                    self.last_attempt_time.store(now, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful request
    pub async fn record_success(&self) {
        if !self.config.enabled {
            return;
        }

        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_successes.fetch_add(1, Ordering::Relaxed);

        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                self.failures.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let successes = self.successes.fetch_add(1, Ordering::Relaxed) + 1;
                debug!(
                    "Circuit breaker half-open: {}/{} successes",
                    successes, self.config.success_threshold
                );
                if successes >= self.config.success_threshold {
                    let mut state = self.state.write().await;
                    *state = CircuitState::Closed;
                    self.failures.store(0, Ordering::Relaxed);
                    info!("Circuit breaker closed after successful recovery");
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self) {
        if !self.config.enabled {
            return;
        }

        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_failures.fetch_add(1, Ordering::Relaxed);

        let failures = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.last_failure_time.store(now, Ordering::Relaxed);

        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => {
                if failures >= self.config.failure_threshold {
                    let mut state = self.state.write().await;
                    *state = CircuitState::Open;
                    warn!(
                        "Circuit breaker opened after {} consecutive failures",
                        failures
                    );
                }
            }
            CircuitState::HalfOpen => {
                let mut state = self.state.write().await;
                *state = CircuitState::Open;
                self.successes.store(0, Ordering::Relaxed);
                warn!("Circuit breaker reopened after failure in half-open state");
            }
            CircuitState::Open => {}
        }
    }

    /// Get the current circuit state
    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Get circuit breaker statistics
    pub fn get_stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            state: *self.state.blocking_read(),
            consecutive_failures: self.failures.load(Ordering::Relaxed),
            consecutive_successes: self.successes.load(Ordering::Relaxed),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_successes: self.total_successes.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
        }
    }

    /// Manually reset the circuit breaker to closed state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failures.store(0, Ordering::Relaxed);
        self.successes.store(0, Ordering::Relaxed);
        self.last_failure_time.store(0, Ordering::Relaxed);
        info!("Circuit breaker manually reset to closed state");
    }

    /// Manually force the circuit open
    pub async fn force_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.last_failure_time.store(now, Ordering::Relaxed);
        warn!("Circuit breaker manually forced to open state");
    }
}

/// Thread-safe wrapper for circuit breaker
pub type CircuitBreakerRef = Arc<CircuitBreaker>;

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    /// Current state
    pub state: CircuitState,
    /// Consecutive failure count
    pub consecutive_failures: u32,
    /// Consecutive success count
    pub consecutive_successes: u32,
    /// Total requests handled
    pub total_requests: u64,
    /// Total successful requests
    pub total_successes: u64,
    /// Total failed requests
    pub total_failures: u64,
}

impl Default for CircuitBreakerStats {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            total_requests: 0,
            total_successes: 0,
            total_failures: 0,
        }
    }
}

/// Circuit breaker manager for multiple exchanges
#[derive(Default)]
pub struct CircuitBreakerManager {
    breakers: std::collections::HashMap<String, CircuitBreakerRef>,
}

impl CircuitBreakerManager {
    /// Create a new circuit breaker manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a circuit breaker for an exchange
    pub fn get_or_create(&mut self, exchange_id: &str, config: CircuitBreakerConfig) -> CircuitBreakerRef {
        if let Some(breaker) = self.breakers.get(exchange_id) {
            return Arc::clone(breaker);
        }

        let breaker = Arc::new(CircuitBreaker::new(config));
        self.breakers.insert(exchange_id.to_string(), Arc::clone(&breaker));
        breaker
    }

    /// Get circuit breaker for an exchange if it exists
    pub fn get(&self, exchange_id: &str) -> Option<&CircuitBreakerRef> {
        self.breakers.get(exchange_id)
    }

    /// Get all circuit breaker statistics
    pub fn get_all_stats(&self) -> std::collections::HashMap<String, CircuitBreakerStats> {
        self.breakers
            .iter()
            .map(|(id, breaker)| (id.clone(), breaker.get_stats()))
            .collect()
    }

    /// Reset all circuit breakers
    pub async fn reset_all(&self) {
        for breaker in self.breakers.values() {
            breaker.reset().await;
        }
    }
}
