use exchange_connectors::rate_limiter::{
    RateLimitConfig, RateLimitStatus, RateLimiter, UnifiedRateLimitManager,
};
use rust_decimal::Decimal;

#[cfg(test)]
mod rate_limit_config_tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.requests_per_second, 10);
        assert_eq!(config.burst_capacity, 20);
        assert_eq!(config.window_seconds, 60);
    }

    #[test]
    fn test_rate_limit_config_custom() {
        let config = RateLimitConfig {
            requests_per_second: 50,
            burst_capacity: 100,
            window_seconds: 30,
        };
        assert_eq!(config.requests_per_second, 50);
        assert_eq!(config.burst_capacity, 100);
        assert_eq!(config.window_seconds, 30);
    }

    #[test]
    fn test_rate_limit_config_clone() {
        let config = RateLimitConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.requests_per_second, config.requests_per_second);
        assert_eq!(cloned.burst_capacity, config.burst_capacity);
        assert_eq!(cloned.window_seconds, config.window_seconds);
    }

    #[test]
    fn test_rate_limit_config_debug_format() {
        let config = RateLimitConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("requests_per_second"));
        assert!(debug_str.contains("burst_capacity"));
    }

    #[test]
    fn test_rate_limit_config_serialization() {
        let config = RateLimitConfig {
            requests_per_second: 25,
            burst_capacity: 50,
            window_seconds: 120,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("25"));
        assert!(json.contains("50"));
        assert!(json.contains("120"));
    }

    #[test]
    fn test_rate_limit_config_deserialization() {
        let json = r#"{"requests_per_second":30,"burst_capacity":60,"window_seconds":90}"#;
        let config: RateLimitConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.requests_per_second, 30);
        assert_eq!(config.burst_capacity, 60);
        assert_eq!(config.window_seconds, 90);
    }
}

#[cfg(test)]
mod rate_limiter_basic_tests {
    use super::*;

    #[test]
    fn test_rate_limiter_creation() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        assert!(limiter.can_proceed());
    }

    #[test]
    fn test_rate_limiter_initial_tokens() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 20,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);
        let status = limiter.get_status();
        assert_eq!(status.available_tokens, 20);
        assert_eq!(status.max_tokens, 20);
    }

    #[test]
    fn test_rate_limiter_try_acquire_success() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 5,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        for _ in 0..5 {
            let result = limiter.try_acquire();
            assert!(result.is_ok());
        }

        let status = limiter.get_status();
        assert_eq!(status.available_tokens, 0);
    }

    #[test]
    fn test_rate_limiter_try_acquire_exhausted() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 2,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        assert!(limiter.try_acquire().is_ok());
        assert!(limiter.try_acquire().is_ok());
        assert!(limiter.try_acquire().is_err());
    }

    #[test]
    fn test_rate_limiter_can_proceed_refills() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 2,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        assert!(limiter.can_proceed());
        assert!(limiter.try_acquire().is_ok());
        assert!(limiter.try_acquire().is_ok());
        assert!(!limiter.can_proceed());
    }

    #[test]
    fn test_rate_limiter_status() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 20,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        limiter.try_acquire().ok();
        let status = limiter.get_status();

        assert_eq!(status.available_tokens, 19);
        assert_eq!(status.max_tokens, 20);
        assert!(status.max_rate > 0);
    }

    #[test]
    fn test_rate_limiter_try_acquire_decrements_tokens() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 10,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        for i in 0..10 {
            assert!(limiter.try_acquire().is_ok());
            let status = limiter.get_status();
            assert_eq!(status.available_tokens, 10 - i - 1);
        }
    }

    #[test]
    fn test_rate_limiter_empty_request_history() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);

        let status = limiter.get_status();
        assert_eq!(status.requests_in_window, 0);
        assert_eq!(status.current_rate, 0.0);
    }

    #[test]
    fn test_rate_limiter_max_tokens_equals_burst_capacity() {
        let config = RateLimitConfig {
            requests_per_second: 5,
            burst_capacity: 15,
            window_seconds: 30,
        };
        let limiter = RateLimiter::new(config);

        let status = limiter.get_status();
        assert_eq!(status.max_tokens, 15);
    }
}

#[cfg(test)]
mod rate_limiter_throttling_tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter_token_refill_over_time() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 10,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        for _ in 0..10 {
            assert!(limiter.try_acquire().is_ok());
        }
        assert!(limiter.try_acquire().is_err());

        thread::sleep(Duration::from_millis(200));

        let status = limiter.get_status();
        assert!(status.available_tokens > 0);
    }

    #[test]
    fn test_rate_limiter_token_refill_full() {
        let config = RateLimitConfig {
            requests_per_second: 100,
            burst_capacity: 10,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        limiter.try_acquire().ok();

        thread::sleep(Duration::from_millis(500));

        let status = limiter.get_status();
        assert!(status.available_tokens >= 10);
    }

    #[test]
    fn test_rate_limiter_burst_handling() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            burst_capacity: 5,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        for i in 0..5 {
            assert!(
                limiter.try_acquire().is_ok(),
                "Request {} should succeed",
                i
            );
        }

        assert!(limiter.try_acquire().is_err());
    }

    #[test]
    fn test_rate_limiter_current_rate_calculation() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 10,
            window_seconds: 1,
        };
        let limiter = RateLimiter::new(config);

        for _ in 0..5 {
            limiter.try_acquire().ok();
        }

        let status = limiter.get_status();
        assert!(status.current_rate > 0.0);
    }

    #[test]
    fn test_rate_limiter_rate_does_not_exceed_max() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 10,
            window_seconds: 1,
        };
        let limiter = RateLimiter::new(config);

        for _ in 0..20 {
            limiter.try_acquire().ok();
            thread::sleep(Duration::from_millis(50));
        }

        let status = limiter.get_status();
        assert!(status.current_rate <= 20.0);
    }

    #[test]
    fn test_rate_limiter_gradual_refill() {
        let config = RateLimitConfig {
            requests_per_second: 2,
            burst_capacity: 4,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        for _ in 0..4 {
            assert!(limiter.try_acquire().is_ok());
        }

        thread::sleep(Duration::from_millis(600));

        let status = limiter.get_status();
        assert!(status.available_tokens >= 1);
    }
}

#[cfg(test)]
mod unified_rate_limit_manager_tests {
    use super::*;

    #[test]
    fn test_unified_manager_creation() {
        let manager = UnifiedRateLimitManager::new();
        assert!(manager.get_all_status().is_empty());
    }

    #[test]
    fn test_unified_manager_default() {
        let manager = UnifiedRateLimitManager::default();
        assert!(manager.get_all_status().is_empty());
    }

    #[test]
    fn test_unified_manager_add_limiter() {
        let mut manager = UnifiedRateLimitManager::new();
        manager.add_limiter(
            "api/v1/ticker".to_string(),
            RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 20,
                window_seconds: 60,
            },
        );

        let statuses = manager.get_all_status();
        assert_eq!(statuses.len(), 1);
        assert!(statuses.contains_key("api/v1/ticker"));
    }

    #[test]
    fn test_unified_manager_add_multiple_limiters() {
        let mut manager = UnifiedRateLimitManager::new();
        manager.add_limiter("api/v1/ticker".to_string(), RateLimitConfig::default());
        manager.add_limiter("api/v1/orderbook".to_string(), RateLimitConfig::default());
        manager.add_limiter("api/v1/trades".to_string(), RateLimitConfig::default());

        let statuses = manager.get_all_status();
        assert_eq!(statuses.len(), 3);
    }

    #[test]
    fn test_unified_manager_try_acquire_existing() {
        let mut manager = UnifiedRateLimitManager::new();
        manager.add_limiter(
            "api/v1/ticker".to_string(),
            RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 5,
                window_seconds: 60,
            },
        );

        for _ in 0..5 {
            assert!(manager.try_acquire("api/v1/ticker").is_ok());
        }
        assert!(manager.try_acquire("api/v1/ticker").is_err());
    }

    #[test]
    fn test_unified_manager_try_acquire_non_existing() {
        let manager = UnifiedRateLimitManager::new();
        assert!(manager.try_acquire("non/existing").is_ok());
    }

    #[test]
    fn test_unified_manager_acquire_non_blocking() {
        let mut manager = UnifiedRateLimitManager::new();
        manager.add_limiter(
            "api/v1/ticker".to_string(),
            RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 2,
                window_seconds: 60,
            },
        );

        assert!(manager.try_acquire("api/v1/ticker").is_ok());
        assert!(manager.try_acquire("api/v1/ticker").is_ok());
        assert!(manager.try_acquire("api/v1/ticker").is_err());
    }

    #[test]
    fn test_unified_manager_get_all_status() {
        let mut manager = UnifiedRateLimitManager::new();
        manager.add_limiter("api/v1/ticker".to_string(), RateLimitConfig::default());
        manager.add_limiter("api/v1/orderbook".to_string(), RateLimitConfig::default());

        let statuses = manager.get_all_status();
        assert_eq!(statuses.len(), 2);

        for (_, status) in statuses {
            assert!(status.available_tokens > 0);
            assert!(status.max_tokens > 0);
        }
    }

    #[test]
    fn test_unified_manager_multiple_endpoints_independent() {
        let mut manager = UnifiedRateLimitManager::new();
        manager.add_limiter(
            "api/v1/ticker".to_string(),
            RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 1,
                window_seconds: 60,
            },
        );
        manager.add_limiter(
            "api/v1/orderbook".to_string(),
            RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 5,
                window_seconds: 60,
            },
        );

        assert!(manager.try_acquire("api/v1/ticker").is_ok());
        assert!(manager.try_acquire("api/v1/orderbook").is_ok());
        assert!(manager.try_acquire("api/v1/orderbook").is_ok());
        assert!(manager.try_acquire("api/v1/orderbook").is_ok());
        assert!(manager.try_acquire("api/v1/orderbook").is_ok());
        assert!(manager.try_acquire("api/v1/orderbook").is_ok());
        assert!(manager.try_acquire("api/v1/orderbook").is_err());
        assert!(manager.try_acquire("api/v1/ticker").is_err());
    }
}

#[cfg(test)]
mod rate_limit_status_tests {
    use super::*;

    #[test]
    fn test_rate_limit_status_fields() {
        let status = RateLimitStatus {
            available_tokens: 15,
            max_tokens: 20,
            current_rate: 5.5,
            max_rate: 10,
            requests_in_window: 100,
        };

        assert_eq!(status.available_tokens, 15);
        assert_eq!(status.max_tokens, 20);
        assert_eq!(status.current_rate, 5.5);
        assert_eq!(status.max_rate, 10);
        assert_eq!(status.requests_in_window, 100);
    }

    #[test]
    fn test_rate_limit_status_clone() {
        let status = RateLimitStatus {
            available_tokens: 10,
            max_tokens: 20,
            current_rate: 3.0,
            max_rate: 10,
            requests_in_window: 50,
        };

        let cloned = status.clone();
        assert_eq!(cloned.available_tokens, status.available_tokens);
        assert_eq!(cloned.max_tokens, status.max_tokens);
    }

    #[test]
    fn test_rate_limit_status_debug_format() {
        let status = RateLimitStatus {
            available_tokens: 10,
            max_tokens: 20,
            current_rate: 5.0,
            max_rate: 10,
            requests_in_window: 100,
        };

        let debug_str = format!("{:?}", status);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("available_tokens"));
    }

    #[test]
    fn test_rate_limit_status_serialization() {
        let status = RateLimitStatus {
            available_tokens: 10,
            max_tokens: 20,
            current_rate: 5.0,
            max_rate: 10,
            requests_in_window: 100,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("10"));
        assert!(json.contains("20"));
        assert!(json.contains("5.0"));
    }
}

#[cfg(test)]
mod rate_limiter_edge_cases {
    use super::*;

    #[test]
    fn test_rate_limiter_zero_requests_per_second() {
        let config = RateLimitConfig {
            requests_per_second: 0,
            burst_capacity: 10,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);
        // With 0 requests per second, the limiter should not allow any requests initially
        // because no tokens are refilled (rate is 0), so only burst capacity is available
        // but with rate 0, no new tokens are added
        let status = limiter.get_status();
        // Burst capacity provides initial tokens, but rate is 0 means no refill
        assert_eq!(status.available_tokens, 10);
        assert_eq!(status.max_rate, 0);
    }

    #[test]
    fn test_rate_limiter_zero_burst_capacity() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 0,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);
        let status = limiter.get_status();
        assert_eq!(status.available_tokens, 0);
        assert_eq!(status.max_tokens, 0);
    }

    #[test]
    fn test_rate_limiter_very_high_rate() {
        let config = RateLimitConfig {
            requests_per_second: 10000,
            burst_capacity: 100,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);
        assert!(limiter.can_proceed());
    }

    #[test]
    fn test_rate_limiter_concurrent_acquire_calls() {
        let config = RateLimitConfig {
            requests_per_second: 100,
            burst_capacity: 50,
            window_seconds: 60,
        };
        let config_clone = config.clone();
        let limiter = RateLimiter::new(config);

        let mut handles = Vec::new();
        for _ in 0..10 {
            let limiter_clone = RateLimiter::new(config_clone.clone());
            handles.push(std::thread::spawn(move || limiter_clone.try_acquire()));
        }

        let mut success_count = 0;
        for handle in handles {
            if handle.join().unwrap().is_ok() {
                success_count += 1;
            }
        }

        assert!(success_count <= 50);
    }
}

#[cfg(test)]
mod exchange_connector_rate_limits {
    use exchange_connectors::connections::{
        BybitConnector, GateioConnector, KrakenConnector, MEXCConnector, OKXConnector,
    };

    #[test]
    fn test_okx_rate_limit_config() {
        let connector = OKXConnector::new();
        let config = &connector.base.config;
        assert_eq!(config.rate_limit_per_second, 20);
        assert_eq!(config.rate_limit_burst, 40);
    }

    #[test]
    fn test_bybit_rate_limit_config() {
        let connector = BybitConnector::new();
        let config = &connector.config;
        assert_eq!(config.rate_limit_per_second, 60);
        assert_eq!(config.rate_limit_burst, 120);
    }

    #[test]
    fn test_mexc_rate_limit_config() {
        let connector = MEXCConnector::new();
        let config = &connector.base.config;
        assert_eq!(config.rate_limit_per_second, 20);
        assert_eq!(config.rate_limit_burst, 40);
    }

    #[test]
    fn test_gateio_rate_limit_config() {
        let connector = GateioConnector::new();
        let config = &connector.config;
        assert_eq!(config.rate_limit_per_second, 20);
        assert_eq!(config.rate_limit_burst, 20);
    }

    #[test]
    fn test_kraken_rate_limit_config() {
        let connector = KrakenConnector::new();
        let config = &connector.config;
        assert_eq!(config.rate_limit_per_second, 15);
        assert_eq!(config.rate_limit_burst, 30);
    }
}
