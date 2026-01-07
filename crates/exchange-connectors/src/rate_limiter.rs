use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

use crate::connector::ConnectorError;

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per second
    pub requests_per_second: u32,
    /// Burst capacity (max requests in burst)
    pub burst_capacity: u32,
    /// Window size in seconds for rate calculation
    pub window_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 10,
            burst_capacity: 20,
            window_seconds: 60,
        }
    }
}

/// Token bucket rate limiter implementation
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimitConfig,
    tokens: Arc<Mutex<f64>>,
    last_refill: Arc<Mutex<DateTime<Utc>>>,
    request_history: Arc<Mutex<VecDeque<DateTime<Utc>>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(config.burst_capacity as f64)),
            last_refill: Arc::new(Mutex::new(Utc::now())),
            request_history: Arc::new(Mutex::new(VecDeque::new())),
            config,
        }
    }

    /// Check if a request can be made (non-blocking)
    pub fn can_proceed(&self) -> bool {
        self.refill_tokens();
        
        let tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        *tokens >= 1.0
    }

    /// Wait until a request can be made (blocking)
    pub async fn acquire(&self) -> Result<(), ConnectorError> {
        loop {
            self.refill_tokens();
            
            {
                let mut tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
                if *tokens >= 1.0 {
                    *tokens -= 1.0;
                    self.record_request();
                    return Ok(());
                }
            }
            
            // Calculate wait time
            let wait_ms = 1000 / self.config.requests_per_second as u64;
            sleep(Duration::from_millis(wait_ms)).await;
        }
    }

    /// Try to acquire without waiting
    pub fn try_acquire(&self) -> Result<(), ConnectorError> {
        self.refill_tokens();
        
        let mut tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            self.record_request();
            Ok(())
        } else {
            Err(ConnectorError::RateLimitExceeded(
                "Rate limit exceeded, try again later".to_string()
            ))
        }
    }

    /// Get current rate limit status
    pub fn get_status(&self) -> RateLimitStatus {
        self.refill_tokens();
        
        let tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        let history = self.request_history.lock().unwrap_or_else(|e| e.into_inner());
        
        let current_rate = self.calculate_current_rate(&history);
        
        RateLimitStatus {
            available_tokens: *tokens as u32,
            max_tokens: self.config.burst_capacity,
            current_rate,
            max_rate: self.config.requests_per_second,
            requests_in_window: history.len() as u32,
        }
    }

    /// Refill tokens based on elapsed time
    fn refill_tokens(&self) {
        let now = Utc::now();
        let mut last_refill = self.last_refill.lock().unwrap_or_else(|e| e.into_inner());
        let mut tokens = self.tokens.lock().unwrap_or_else(|e| e.into_inner());
        
        let elapsed = (now - *last_refill).num_milliseconds() as f64 / 1000.0;
        let tokens_to_add = elapsed * self.config.requests_per_second as f64;
        
        *tokens = (*tokens + tokens_to_add).min(self.config.burst_capacity as f64);
        *last_refill = now;
    }

    /// Record a request in history
    fn record_request(&self) {
        let now = Utc::now();
        let mut history = self.request_history.lock().unwrap_or_else(|e| e.into_inner());
        
        // Add current request
        history.push_back(now);
        
        // Remove old requests outside the window
        let window_start = now - chrono::Duration::seconds(self.config.window_seconds as i64);
        while let Some(&front_time) = history.front() {
            if front_time < window_start {
                history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculate current request rate
    fn calculate_current_rate(&self, history: &VecDeque<DateTime<Utc>>) -> f64 {
        if history.is_empty() {
            return 0.0;
        }
        
        let now = Utc::now();
        let window_start = now - chrono::Duration::seconds(self.config.window_seconds as i64);
        
        let requests_in_window = history.iter()
            .filter(|&&time| time >= window_start)
            .count();
            
        requests_in_window as f64 / self.config.window_seconds as f64
    }
}

/// Current rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub available_tokens: u32,
    pub max_tokens: u32,
    pub current_rate: f64,
    pub max_rate: u32,
    pub requests_in_window: u32,
}

/// Unified rate limit manager for multiple endpoints
#[derive(Debug)]
pub struct UnifiedRateLimitManager {
    limiters: std::collections::HashMap<String, RateLimiter>,
}

impl UnifiedRateLimitManager {
    pub fn new() -> Self {
        Self {
            limiters: std::collections::HashMap::new(),
        }
    }

    /// Add a rate limiter for a specific endpoint
    pub fn add_limiter(&mut self, endpoint: String, config: RateLimitConfig) {
        self.limiters.insert(endpoint, RateLimiter::new(config));
    }

    /// Acquire rate limit for specific endpoint
    pub async fn acquire(&self, endpoint: &str) -> Result<(), ConnectorError> {
        if let Some(limiter) = self.limiters.get(endpoint) {
            limiter.acquire().await
        } else {
            // No rate limiter configured, allow request
            Ok(())
        }
    }

    /// Try to acquire without waiting
    pub fn try_acquire(&self, endpoint: &str) -> Result<(), ConnectorError> {
        if let Some(limiter) = self.limiters.get(endpoint) {
            limiter.try_acquire()
        } else {
            // No rate limiter configured, allow request
            Ok(())
        }
    }

    /// Get status for all endpoints
    pub fn get_all_status(&self) -> std::collections::HashMap<String, RateLimitStatus> {
        self.limiters.iter()
            .map(|(endpoint, limiter)| (endpoint.clone(), limiter.get_status()))
            .collect()
    }
}

impl Default for UnifiedRateLimitManager {
    fn default() -> Self {
        Self::new()
    }
}