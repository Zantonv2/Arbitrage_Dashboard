#[cfg(test)]
mod server_tests {
    use super::super::server::{
        create_jwt, sanitize_path, validate_jwt, Claims, RateLimitState, RATE_LIMIT_MAX_REQUESTS,
        RATE_LIMIT_WINDOW_SECS,
    };
    use std::time::Instant;

    #[test]
    fn test_sanitize_path_valid() {
        let result = sanitize_path("/tmp/test_file.txt");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("test_file"));
    }

    #[test]
    fn test_sanitize_path_traversal_attempt() {
        let result = sanitize_path("../etc/passwd");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Path traversal sequences not allowed");
    }

    #[test]
    fn test_sanitize_path_double_dot_middle() {
        let result = sanitize_path("/safe/../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_path_encoded_dots() {
        let result = sanitize_path("/safe/%2e%2e/etc/passwd");
        assert!(result.is_ok());
    }

    #[test]
    fn test_rate_limit_new_is_empty() {
        let state = RateLimitState::new();
        let requests = state.requests.lock().unwrap();
        assert!(requests.is_empty());
    }

    #[test]
    fn test_rate_limit_allows_first_request() {
        let state = RateLimitState::new();
        let allowed =
            state.check_rate_limit("test_ip", RATE_LIMIT_MAX_REQUESTS, RATE_LIMIT_WINDOW_SECS);
        assert!(allowed);
    }

    #[test]
    fn test_rate_limit_tracks_request_count() {
        let state = RateLimitState::new();
        let key = "test_ip";

        for i in 0..5 {
            let allowed = state.check_rate_limit(key, 5, RATE_LIMIT_WINDOW_SECS);
            assert!(allowed, "Request {} should be allowed", i + 1);
        }

        let allowed = state.check_rate_limit(key, 5, RATE_LIMIT_WINDOW_SECS);
        assert!(!allowed, "6th request should be blocked");
    }

    #[test]
    fn test_rate_limit_window_expiration() {
        let state = RateLimitState::new();
        let key = "test_ip_window";

        assert!(state.check_rate_limit(key, 2, 1));

        let requests = state.requests.lock().unwrap();
        if let Some((first_request, count)) = requests.get(key) {
            let elapsed = Instant::now().duration_since(*first_request);
            assert_eq!(*count, 1);
        }
    }

    #[test]
    fn test_rate_limit_different_keys_independent() {
        let state = RateLimitState::new();
        let key1 = "ip_1";
        let key2 = "ip_2";

        for _ in 0..5 {
            assert!(state.check_rate_limit(key1, 5, RATE_LIMIT_WINDOW_SECS));
        }

        assert!(!state.check_rate_limit(key1, 5, RATE_LIMIT_WINDOW_SECS));
        assert!(state.check_rate_limit(key2, 5, RATE_LIMIT_WINDOW_SECS));
    }

    #[test]
    fn test_jwt_creation_and_validation() {
        let secret = "test_secret_key_for_jwt";
        let user_id = "test_user";

        let token = create_jwt(user_id, secret);
        assert!(token.is_ok());
        let token = token.unwrap();

        let is_valid = validate_jwt(&token, secret);
        assert!(is_valid, "JWT should be valid with correct secret");
    }

    #[test]
    fn test_jwt_invalid_secret() {
        let secret = "correct_secret";
        let wrong_secret = "wrong_secret";
        let user_id = "test_user";

        let token = create_jwt(user_id, secret).unwrap();
        let is_valid = validate_jwt(&token, wrong_secret);
        assert!(!is_valid, "JWT should be invalid with wrong secret");
    }

    #[test]
    fn test_jwt_malformed_token() {
        let is_valid = validate_jwt("not_a_valid_token", "any_secret");
        assert!(!is_valid);
    }

    #[test]
    fn test_jwt_empty_token() {
        let is_valid = validate_jwt("", "any_secret");
        assert!(!is_valid);
    }

    #[test]
    fn test_claims_structure() {
        let claims = Claims {
            sub: "user123".to_string(),
            exp: 1234567890,
            iat: 1234560000,
        };

        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.exp, 1234567890);
        assert_eq!(claims.iat, 1234560000);
    }
}

#[cfg(test)]
mod websocket_tests {
    use super::super::websocket::validate_ws_token;
    use crate::server::create_jwt;

    #[test]
    fn test_validate_ws_token_valid() {
        let secret = "ws_test_secret";
        let user_id = "websocket_user";

        let token = create_jwt(user_id, secret).unwrap();
        let is_valid = validate_ws_token(&token, secret);
        assert!(is_valid);
    }

    #[test]
    fn test_validate_ws_token_invalid() {
        let secret = "ws_test_secret";
        let wrong_secret = "wrong_secret";

        let token = create_jwt("user", secret).unwrap();
        let is_valid = validate_ws_token(&token, wrong_secret);
        assert!(!is_valid);
    }

    #[test]
    fn test_validate_ws_token_empty() {
        let is_valid = validate_ws_token("", "any_secret");
        assert!(!is_valid);
    }

    #[test]
    fn test_validate_ws_token_malformed() {
        let is_valid = validate_ws_token("malformed.token.here", "any_secret");
        assert!(!is_valid);
    }
}

#[cfg(test)]
mod routes_tests {
    use super::super::routes::{LoginRequest, PrepareExecutionRequest};
    use uuid::Uuid;

    #[test]
    fn test_login_request_valid() {
        let request = LoginRequest {
            username: "admin123".to_string(),
            password: "securepassword123".to_string(),
        };

        let result = request.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_login_request_username_too_short() {
        let request = LoginRequest {
            username: "ab".to_string(),
            password: "securepassword123".to_string(),
        };

        let result = request.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("at least 3 characters")));
    }

    #[test]
    fn test_login_request_username_too_long() {
        let request = LoginRequest {
            username: "a".repeat(51),
            password: "securepassword123".to_string(),
        };

        let result = request.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("at most 50 characters")));
    }

    #[test]
    fn test_login_request_username_invalid_chars() {
        let request = LoginRequest {
            username: "admin@#$".to_string(),
            password: "securepassword123".to_string(),
        };

        let result = request.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("alphanumeric")));
    }

    #[test]
    fn test_login_request_password_too_short() {
        let request = LoginRequest {
            username: "admin123".to_string(),
            password: "short".to_string(),
        };

        let result = request.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("at least 8 characters")));
    }

    #[test]
    fn test_prepare_execution_valid_uuid() {
        let signal_id = Uuid::new_v4().to_string();
        let request = PrepareExecutionRequest {
            signal_id,
            quantity: Some(0.5),
        };

        let result = request.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_prepare_execution_invalid_uuid() {
        let request = PrepareExecutionRequest {
            signal_id: "not-a-valid-uuid".to_string(),
            quantity: Some(0.5),
        };

        let result = request.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("UUID")));
    }

    #[test]
    fn test_prepare_execution_negative_quantity() {
        let signal_id = Uuid::new_v4().to_string();
        let request = PrepareExecutionRequest {
            signal_id,
            quantity: Some(-1.0),
        };

        let result = request.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("positive")));
    }

    #[test]
    fn test_prepare_execution_zero_quantity() {
        let signal_id = Uuid::new_v4().to_string();
        let request = PrepareExecutionRequest {
            signal_id,
            quantity: Some(0.0),
        };

        let result = request.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_prepare_execution_missing_quantity() {
        let signal_id = Uuid::new_v4().to_string();
        let request = PrepareExecutionRequest {
            signal_id,
            quantity: None,
        };

        let result = request.validate();
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod key_store_tests {
    use super::super::key_store::{ExchangeCredentials, KeyStore, KeyStoreError};
    use arbitrage_core::types::ExchangeId;

    #[test]
    fn test_keystore_new_is_empty() {
        let store = KeyStore::new().unwrap();
        let exchanges = store.list_exchanges();
        assert!(exchanges.is_empty());
    }

    #[test]
    fn test_keystore_with_master_key() {
        let store = KeyStore::with_master_key("master_password".to_string()).unwrap();
        let exchanges = store.list_exchanges();
        assert!(exchanges.is_empty());
    }

    #[test]
    fn test_keystore_store_and_retrieve_credentials() {
        let mut store = KeyStore::with_master_key("test_master_key".to_string()).unwrap();

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "test_api_key_12345".to_string(),
            api_secret: "test_api_secret_67890".to_string(),
            passphrase: None,
            sandbox: true,
            enabled: true,
        };

        let result = store.store_credentials(credentials.clone());
        assert!(result.is_ok());

        let retrieved = store.get_credentials(&ExchangeId::MEXC);
        assert!(retrieved.is_ok());
        let retrieved = retrieved.unwrap();

        assert_eq!(retrieved.api_key, credentials.api_key);
        assert_eq!(retrieved.api_secret, credentials.api_secret);
        assert_eq!(retrieved.sandbox, credentials.sandbox);
        assert_eq!(retrieved.enabled, credentials.enabled);
    }

    #[test]
    fn test_keystore_get_nonexistent_credentials() {
        let store = KeyStore::new().unwrap();
        let result = store.get_credentials(&ExchangeId::MEXC);
        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                KeyStoreError::KeyNotFound(_) => {}
                _ => panic!("Expected KeyNotFound error"),
            }
        }
    }

    #[test]
    fn test_keystore_has_credentials() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        assert!(!store.has_credentials(&ExchangeId::MEXC));

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };
        store.store_credentials(credentials).unwrap();

        assert!(store.has_credentials(&ExchangeId::MEXC));
        assert!(!store.has_credentials(&ExchangeId::OKX));
    }

    #[test]
    fn test_keystore_remove_credentials() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };
        store.store_credentials(credentials).unwrap();
        assert!(store.has_credentials(&ExchangeId::MEXC));

        let result = store.remove_credentials(&ExchangeId::MEXC);
        assert!(result.is_ok());
        assert!(!store.has_credentials(&ExchangeId::MEXC));
    }

    #[test]
    fn test_keystore_list_exchanges() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let mexc_credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };
        store.store_credentials(mexc_credentials).unwrap();

        let okx_credentials = ExchangeCredentials {
            exchange: ExchangeId::OKX,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: Some("passphrase".to_string()),
            sandbox: false,
            enabled: true,
        };
        store.store_credentials(okx_credentials).unwrap();

        let exchanges = store.list_exchanges();
        assert_eq!(exchanges.len(), 2);
        assert!(exchanges.contains(&ExchangeId::MEXC));
        assert!(exchanges.contains(&ExchangeId::OKX));
    }

    #[test]
    fn test_keystore_set_enabled() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };
        store.store_credentials(credentials).unwrap();

        let result = store.set_enabled(&ExchangeId::MEXC, false);
        assert!(result.is_ok());

        let enabled = store.get_enabled_exchanges();
        assert!(!enabled.contains(&ExchangeId::MEXC));
    }

    #[test]
    fn test_keystore_get_enabled_exchanges() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let enabled_creds = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };
        store.store_credentials(enabled_creds).unwrap();

        let disabled_creds = ExchangeCredentials {
            exchange: ExchangeId::OKX,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: Some("pass".to_string()),
            sandbox: false,
            enabled: false,
        };
        store.store_credentials(disabled_creds).unwrap();

        let enabled = store.get_enabled_exchanges();
        assert_eq!(enabled.len(), 1);
        assert!(enabled.contains(&ExchangeId::MEXC));
    }

    #[test]
    fn test_keystore_clear() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };
        store.store_credentials(credentials).unwrap();
        assert!(!store.list_exchanges().is_empty());

        store.clear();
        assert!(store.list_exchanges().is_empty());
    }

    #[test]
    fn test_keystore_empty_api_key_rejected() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };

        let result = store.store_credentials(credentials);
        assert!(result.is_err());
    }

    #[test]
    fn test_keystore_empty_api_secret_rejected() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::MEXC,
            api_key: "key".to_string(),
            api_secret: "".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };

        let result = store.store_credentials(credentials);
        assert!(result.is_err());
    }

    #[test]
    fn test_keystore_okx_requires_passphrase() {
        let mut store = KeyStore::with_master_key("master".to_string()).unwrap();

        let credentials = ExchangeCredentials {
            exchange: ExchangeId::OKX,
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            passphrase: None,
            sandbox: false,
            enabled: true,
        };

        let result = store.store_credentials(credentials);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod config_manager_tests {
    use super::super::config_manager::{sanitize_path, ConfigError, ConfigManager};
    use std::path::Path;

    #[test]
    fn test_sanitize_path_valid() {
        let result = sanitize_path("/tmp");
        assert!(result.is_ok());
        assert!(result.unwrap().is_absolute());
    }

    #[test]
    fn test_sanitize_path_traversal() {
        let result = sanitize_path("../etc/passwd");
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("Path traversal"));
        }
    }

    #[test]
    fn test_sanitize_path_with_dots_in_middle() {
        let result = sanitize_path("/safe/../unsafe");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_manager_nonexistent_file() {
        let result = ConfigManager::new("/nonexistent/path/config.toml");
        assert!(result.is_ok());
        let manager = result.unwrap();
        let config = manager.get_config();
        assert!(config.server.port > 0);
    }

    #[test]
    fn test_config_manager_get_config() {
        let manager = ConfigManager::new("/nonexistent/config.toml").unwrap();
        let config = manager.get_config();
        assert!(config.server.port > 0);
    }

    #[test]
    fn test_login_request_valid() {
        let request = LoginRequest {
            username: "admin123".to_string(),
            password: "securepassword123".to_string(),
        };

        let result = request.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_validation_negative_profit_threshold() {
        let mut manager = ConfigManager::new("/nonexistent/config.toml").unwrap();
        let mut config = manager.get_config().clone();
        config.trading.min_profit_threshold_percent = rust_decimal::Decimal::new(-10, 0);

        let result = manager.update_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_zero_position_size() {
        let mut manager = ConfigManager::new("/nonexistent/config.toml").unwrap();
        let mut config = manager.get_config().clone();
        config.risk.max_position_size_usd = rust_decimal::Decimal::ZERO;

        let result = manager.update_config(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_manager_reload() {
        let mut manager = ConfigManager::new("/nonexistent/config.toml").unwrap();
        let result = manager.reload();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_manager_create_default() {
        let temp_path = "/tmp/test_config_manager_default.toml";
        if Path::new(temp_path).exists() {
            std::fs::remove_file(temp_path).unwrap();
        }

        let result = ConfigManager::create_default_config(temp_path);
        assert!(result.is_ok());
        assert!(Path::new(temp_path).exists());

        if Path::new(temp_path).exists() {
            std::fs::remove_file(temp_path).unwrap();
        }
    }

    #[test]
    fn test_config_manager_create_default_exists() {
        let temp_path = "/tmp/test_config_manager_exists.toml";
        std::fs::write(temp_path, "existing = true").unwrap();

        let result = ConfigManager::create_default_config(temp_path);
        assert!(result.is_ok());

        if Path::new(temp_path).exists() {
            std::fs::remove_file(temp_path).unwrap();
        }
    }
}

#[cfg(test)]
mod audit_logger_tests {
    use super::super::audit_logger::AuditLogger;

    #[test]
    fn test_audit_logger_new() {
        let logger = AuditLogger::new();
        assert!(logger.0.is_empty());
    }

    #[test]
    fn test_audit_logger_default() {
        let logger = AuditLogger::default();
        assert!(logger.0.is_empty());
    }
}

#[cfg(test)]
mod bridge_tests {
    use super::super::bridge::BridgeStats;

    #[test]
    fn test_bridge_stats_default() {
        let stats = BridgeStats::default();
        assert_eq!(stats.connected_exchanges, 0);
        assert_eq!(stats.total_exchanges, 0);
        assert_eq!(stats.total_messages_received, 0);
        assert_eq!(stats.total_errors, 0);
        assert_eq!(stats.active_symbols, 0);
        assert_eq!(stats.cached_signals, 0);
        assert_eq!(stats.order_books_cached, 0);
    }

    #[test]
    fn test_bridge_stats_clone() {
        let stats = BridgeStats {
            connected_exchanges: 5,
            total_exchanges: 6,
            total_messages_received: 1000,
            total_errors: 5,
            active_symbols: 50,
            cached_signals: 100,
            order_books_cached: 200,
        };

        let cloned = stats.clone();
        assert_eq!(cloned.connected_exchanges, 5);
        assert_eq!(cloned.total_exchanges, 6);
        assert_eq!(cloned.total_messages_received, 1000);
    }

    #[test]
    fn test_bridge_stats_debug() {
        let stats = BridgeStats::default();
        let debug_output = format!("{:?}", stats);
        assert!(debug_output.contains("BridgeStats"));
    }
}

#[cfg(test)]
mod order_executor_tests {
    use super::super::order_executor::{ExecutionResult, ExecutorConfig};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert_eq!(config.execution_timeout_ms, 5000);
        assert_eq!(config.max_slippage_percent, Decimal::new(50, 4));
        assert!(config.enable_rollback);
        assert_eq!(config.min_profit_threshold, Decimal::new(10, 4));
        assert_eq!(config.max_position_size, Decimal::new(10000, 0));
    }

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult {
            signal_id: Uuid::new_v4(),
            success: true,
            buy_order: None,
            sell_order: None,
            actual_profit: Some(Decimal::new(100, 2)),
            execution_time_ms: 150,
            error_message: None,
            rollback_performed: false,
        };

        assert!(result.success);
        assert!(result.actual_profit.is_some());
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult {
            signal_id: Uuid::new_v4(),
            success: false,
            buy_order: None,
            sell_order: None,
            actual_profit: None,
            execution_time_ms: 200,
            error_message: Some("Order failed".to_string()),
            rollback_performed: true,
        };

        assert!(!result.success);
        assert!(result.actual_profit.is_none());
        assert!(result.error_message.is_some());
        assert!(result.rollback_performed);
    }

    #[test]
    fn test_executor_config_clone() {
        let config = ExecutorConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.execution_timeout_ms, config.execution_timeout_ms);
    }

    #[test]
    fn test_executor_config_debug() {
        let config = ExecutorConfig::default();
        let debug_output = format!("{:?}", config);
        assert!(debug_output.contains("ExecutorConfig"));
    }
}
