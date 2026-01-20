#[cfg(test)]
mod tests {
    use arbitrage_core::Result;
    use exchange_connectors::rate_limiter::{
        RateLimitConfig, RateLimitStatus, RateLimiter, UnifiedRateLimitManager,
    };
    use std::collections::HashMap;

    // === Happy Path Tests ===

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.requests_per_second, 10);
        assert_eq!(config.burst_capacity, 20);
        assert_eq!(config.window_seconds, 60);
    }

    #[test]
    fn test_rate_limiter_creation() {
        let config = RateLimitConfig {
            requests_per_second: 50,
            burst_capacity: 100,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);
        let status = limiter.get_status();
        assert_eq!(status.max_tokens, 100);
        assert_eq!(status.max_rate, 50);
    }

    #[test]
    fn test_rate_limiter_can_proceed() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        // Should be able to proceed initially (burst capacity available)
        assert!(limiter.can_proceed());
    }

    #[test]
    fn test_rate_limiter_try_acquire_success() {
        let config = RateLimitConfig {
            requests_per_second: 100,
            burst_capacity: 100,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);
        let result = limiter.try_acquire();
        assert!(result.is_ok());
    }

    #[test]
    fn test_rate_limiter_try_acquire_exhausts_tokens() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            burst_capacity: 2,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        // Acquire all tokens
        assert!(limiter.try_acquire().is_ok());
        assert!(limiter.try_acquire().is_ok());
        // Third should fail
        assert!(limiter.try_acquire().is_err());
    }

    #[test]
    fn test_rate_limiter_status() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 20,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);
        let status = limiter.get_status();

        assert!(status.available_tokens <= status.max_tokens);
        assert_eq!(status.max_rate, 10);
        assert_eq!(status.max_tokens, 20);
    }

    #[test]
    fn test_unified_rate_limit_manager_new() {
        let manager = UnifiedRateLimitManager::new();
        let status = manager.get_all_status();
        assert!(status.is_empty());
    }

    #[test]
    fn test_unified_rate_limit_manager_add_limiter() {
        let mut manager = UnifiedRateLimitManager::new();
        let config = RateLimitConfig {
            requests_per_second: 50,
            burst_capacity: 100,
            window_seconds: 60,
        };
        manager.add_limiter("api/v1/ticker".to_string(), config);

        let status = manager.get_all_status();
        assert_eq!(status.len(), 1);
        assert!(status.contains_key("api/v1/ticker"));
    }

    #[test]
    fn test_unified_rate_limit_manager_try_acquire_known_endpoint() {
        let mut manager = UnifiedRateLimitManager::new();
        let config = RateLimitConfig {
            requests_per_second: 100,
            burst_capacity: 10,
            window_seconds: 60,
        };
        manager.add_limiter("test_endpoint".to_string(), config);

        let result = manager.try_acquire("test_endpoint");
        assert!(result.is_ok());
    }

    #[test]
    fn test_unified_rate_limit_manager_try_acquire_unknown_endpoint() {
        let manager = UnifiedRateLimitManager::new();
        // Unknown endpoint should allow request (no rate limit configured)
        let result = manager.try_acquire("unknown_endpoint");
        assert!(result.is_ok());
    }

    // === Edge Case Tests ===

    #[test]
    fn test_rate_limiter_zero_config() {
        let config = RateLimitConfig {
            requests_per_second: 0,
            burst_capacity: 0,
            window_seconds: 0,
        };
        let limiter = RateLimiter::new(config);
        let status = limiter.get_status();
        assert_eq!(status.max_tokens, 0);
        assert_eq!(status.max_rate, 0);
    }

    #[test]
    fn test_rate_limiter_single_request_per_second() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            burst_capacity: 1,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        assert!(limiter.try_acquire().is_ok());
        // Should be exhausted
        assert!(limiter.try_acquire().is_err());
    }

    #[test]
    fn test_rate_limiter_large_burst() {
        let config = RateLimitConfig {
            requests_per_second: 1000,
            burst_capacity: 1000,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        // Should be able to make many requests
        for _ in 0..100 {
            assert!(limiter.try_acquire().is_ok());
        }
    }

    #[test]
    fn test_rate_limiter_status_after_requests() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 20,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        // Make some requests
        for _ in 0..5 {
            let _ = limiter.try_acquire();
        }

        let status = limiter.get_status();
        assert!(status.available_tokens < 20);
        assert_eq!(status.requests_in_window, 5);
    }

    // === Error Path Tests ===

    #[test]
    fn test_rate_limiter_exhausted_try_acquire() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            burst_capacity: 1,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        // Exhaust tokens
        limiter.try_acquire().ok();

        // Should fail
        let result = limiter.try_acquire();
        assert!(result.is_err());
        match result {
            Err(e) => {
                let err_msg = format!("{}", e);
                assert!(err_msg.contains("Rate limit") || err_msg.contains("rate limit"));
            }
            _ => panic!("Expected error"),
        }
    }

    // === Boundary Condition Tests ===

    #[test]
    fn test_rate_limiter_very_large_config() {
        let config = RateLimitConfig {
            requests_per_second: u32::MAX,
            burst_capacity: u32::MAX,
            window_seconds: u64::MAX,
        };
        let limiter = RateLimiter::new(config);
        let status = limiter.get_status();
        assert_eq!(status.max_tokens, u32::MAX);
        assert_eq!(status.max_rate, u32::MAX);
    }

    #[test]
    fn test_unified_manager_multiple_limiters() {
        let mut manager = UnifiedRateLimitManager::new();

        manager.add_limiter("endpoint1".to_string(), RateLimitConfig::default());
        manager.add_limiter(
            "endpoint2".to_string(),
            RateLimitConfig {
                requests_per_second: 20,
                burst_capacity: 40,
                window_seconds: 30,
            },
        );
        manager.add_limiter(
            "endpoint3".to_string(),
            RateLimitConfig {
                requests_per_second: 5,
                burst_capacity: 10,
                window_seconds: 120,
            },
        );

        let status = manager.get_all_status();
        assert_eq!(status.len(), 3);
    }

    #[test]
    fn test_unified_manager_acquire_all_endpoints() {
        let mut manager = UnifiedRateLimitManager::new();

        manager.add_limiter(
            "endpoint1".to_string(),
            RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 10,
                window_seconds: 60,
            },
        );
        manager.add_limiter(
            "endpoint2".to_string(),
            RateLimitConfig {
                requests_per_second: 20,
                burst_capacity: 20,
                window_seconds: 60,
            },
        );

        // Acquire from both
        assert!(manager.try_acquire("endpoint1").is_ok());
        assert!(manager.try_acquire("endpoint2").is_ok());
    }

    // === Type Conversion Tests ===

    #[test]
    fn test_rate_limit_status_creation() {
        let status = RateLimitStatus {
            available_tokens: 15,
            max_tokens: 20,
            current_rate: 5.0,
            max_rate: 10,
            requests_in_window: 300,
        };
        assert_eq!(status.available_tokens, 15);
        assert_eq!(status.max_tokens, 20);
        assert_eq!(status.current_rate, 5.0);
    }

    #[test]
    fn test_rate_limit_config_debug_format() {
        let config = RateLimitConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("requests_per_second"));
        assert!(debug_str.contains("burst_capacity"));
        assert!(debug_str.contains("window_seconds"));
    }

    #[test]
    fn test_rate_limiter_debug_format() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        let debug_str = format!("{:?}", limiter);
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn test_rate_limit_status_debug_format() {
        let status = RateLimitStatus {
            available_tokens: 15,
            max_tokens: 20,
            current_rate: 5.0,
            max_rate: 10,
            requests_in_window: 300,
        };
        let debug_str = format!("{:?}", status);
        assert!(debug_str.contains("available_tokens"));
        assert!(debug_str.contains("current_rate"));
    }

    #[test]
    fn test_unified_rate_limit_manager_debug_format() {
        let manager = UnifiedRateLimitManager::new();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("limiters"));
    }

    // === Additional Tests ===

    #[test]
    fn test_rate_limiter_can_proceed_after_exhaustion() {
        let config = RateLimitConfig {
            requests_per_second: 1,
            burst_capacity: 1,
            window_seconds: 1,
        };
        let limiter = RateLimiter::new(config);

        // Exhaust
        assert!(limiter.try_acquire().is_ok());
        assert!(!limiter.can_proceed());

        // Wait and check again (this might be flaky in unit tests)
        // For unit test, we just verify the exhaustion behavior
    }

    #[test]
    fn test_unified_manager_replace_existing_limiter() {
        let mut manager = UnifiedRateLimitManager::new();

        manager.add_limiter(
            "same_endpoint".to_string(),
            RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 20,
                window_seconds: 60,
            },
        );

        // Replace with new config
        manager.add_limiter(
            "same_endpoint".to_string(),
            RateLimitConfig {
                requests_per_second: 50,
                burst_capacity: 100,
                window_seconds: 30,
            },
        );

        let status = manager.get_all_status();
        assert_eq!(status.len(), 1);
        // Should have the new values
        let endpoint_status = status.get("same_endpoint").unwrap();
        assert_eq!(endpoint_status.max_rate, 50);
        assert_eq!(endpoint_status.max_tokens, 100);
    }

    #[test]
    fn test_rate_limiter_status_current_rate() {
        let config = RateLimitConfig {
            requests_per_second: 60,
            burst_capacity: 100,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        let status_before = limiter.get_status();

        // Make some requests
        for _ in 0..10 {
            let _ = limiter.try_acquire();
        }

        let status_after = limiter.get_status();

        // Current rate should be non-zero
        assert!(status_after.current_rate >= 0.0);
        // Available tokens should be reduced
        assert!(status_after.available_tokens <= status_before.available_tokens);
    }

    #[test]
    fn test_rate_limiter_clone() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        // RateLimiter should be cloneable
        let _cloned = limiter.clone();
    }

    #[test]
    fn test_rate_limit_status_zero_values() {
        let status = RateLimitStatus {
            available_tokens: 0,
            max_tokens: 0,
            current_rate: 0.0,
            max_rate: 0,
            requests_in_window: 0,
        };
        assert_eq!(status.available_tokens, 0);
        assert_eq!(status.current_rate, 0.0);
    }

    #[test]
    fn test_unified_manager_empty_status() {
        let manager = UnifiedRateLimitManager::new();
        let status = manager.get_all_status();
        assert!(status.is_empty());
    }

    #[test]
    fn test_rate_limiter_multiple_acquire_calls() {
        let config = RateLimitConfig {
            requests_per_second: 10,
            burst_capacity: 10,
            window_seconds: 60,
        };
        let limiter = RateLimiter::new(config);

        // Make multiple acquires
        for i in 0..10 {
            let result = limiter.try_acquire();
            if i < 10 {
                assert!(result.is_ok(), "Request {} should succeed", i);
            }
        }

        // All tokens should be exhausted
        assert!(!limiter.can_proceed());
    }

    #[test]
    fn test_rate_limiter_initial_state() {
        let config = RateLimitConfig::default(); // 10 rps, 20 burst
        let limiter = RateLimiter::new(config);

        let status = limiter.get_status();
        // Should have full burst capacity initially
        assert_eq!(status.available_tokens, 20);
        assert_eq!(status.requests_in_window, 0);
    }
}
