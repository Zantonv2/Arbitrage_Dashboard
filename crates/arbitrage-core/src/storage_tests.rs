#[cfg(test)]
mod tests {
    use crate::storage::{
        ExecutionStatus, SignalQuery, SignalStatus, StorageConfig, StoredExecution, StoredSignal,
    };
    use crate::types::{ExchangeId, ExecutionInstruction, Order, OrderType, Side, Signal, Symbol};
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn create_test_signal() -> Signal {
        Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
            Utc::now(),
        )
    }

    fn create_test_execution(signal_id: Uuid) -> ExecutionInstruction {
        let buy_order = Order::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Side::Buy,
            OrderType::Market,
            Decimal::from(1),
            None,
        );
        let sell_order = Order::new(
            ExchangeId::ByBit,
            Symbol::new("BTC", "USDT"),
            Side::Sell,
            OrderType::Market,
            Decimal::from(1),
            None,
        );
        ExecutionInstruction::new(signal_id, buy_order, sell_order)
    }

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();

        assert_eq!(config.database_path, "arbitrage.db");
        assert_eq!(config.max_signal_history, 10000);
        assert_eq!(config.max_execution_history, 5000);
        assert!(!config.enable_compression);
    }

    #[test]
    fn test_storage_config_custom() {
        let config = StorageConfig {
            database_path: "/custom/path.db".to_string(),
            max_signal_history: 50000,
            max_execution_history: 25000,
            enable_compression: true,
        };

        assert_eq!(config.database_path, "/custom/path.db");
        assert_eq!(config.max_signal_history, 50000);
        assert_eq!(config.max_execution_history, 25000);
        assert!(config.enable_compression);
    }

    #[test]
    fn test_storage_config_clone() {
        let config = StorageConfig::default();
        let cloned = config.clone();

        assert_eq!(config.database_path, cloned.database_path);
        assert_eq!(config.max_signal_history, cloned.max_signal_history);
    }

    #[test]
    fn test_signal_status_values() {
        assert_eq!(SignalStatus::Detected.to_string(), "detected");
        assert_eq!(SignalStatus::Filtered.to_string(), "filtered");
        assert_eq!(SignalStatus::Executed.to_string(), "executed");
        assert_eq!(SignalStatus::Expired.to_string(), "expired");
        assert_eq!(SignalStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn test_signal_status_from_string() {
        assert_eq!(
            SignalStatus::from_string("detected").unwrap(),
            SignalStatus::Detected
        );
        assert_eq!(
            SignalStatus::from_string("filtered").unwrap(),
            SignalStatus::Filtered
        );
        assert_eq!(
            SignalStatus::from_string("executed").unwrap(),
            SignalStatus::Executed
        );
        assert_eq!(
            SignalStatus::from_string("expired").unwrap(),
            SignalStatus::Expired
        );
        assert_eq!(
            SignalStatus::from_string("failed").unwrap(),
            SignalStatus::Failed
        );
    }

    #[test]
    fn test_signal_status_from_string_invalid() {
        let result = SignalStatus::from_string("invalid_status");
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_status_values() {
        assert_eq!(ExecutionStatus::Pending.to_string(), "pending");
        assert_eq!(
            ExecutionStatus::PartiallyFilled.to_string(),
            "partially_filled"
        );
        assert_eq!(ExecutionStatus::Completed.to_string(), "completed");
        assert_eq!(ExecutionStatus::Failed.to_string(), "failed");
        assert_eq!(ExecutionStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn test_execution_status_from_string() {
        assert_eq!(
            ExecutionStatus::from_string("pending").unwrap(),
            ExecutionStatus::Pending
        );
        assert_eq!(
            ExecutionStatus::from_string("partially_filled").unwrap(),
            ExecutionStatus::PartiallyFilled
        );
        assert_eq!(
            ExecutionStatus::from_string("completed").unwrap(),
            ExecutionStatus::Completed
        );
        assert_eq!(
            ExecutionStatus::from_string("failed").unwrap(),
            ExecutionStatus::Failed
        );
        assert_eq!(
            ExecutionStatus::from_string("cancelled").unwrap(),
            ExecutionStatus::Cancelled
        );
    }

    #[test]
    fn test_execution_status_from_string_invalid() {
        let result = ExecutionStatus::from_string("invalid_status");
        assert!(result.is_err());
    }

    #[test]
    fn test_signal_query_default() {
        let query = SignalQuery::default();

        assert_eq!(query.limit, Some(100));
        assert!(query.status_filter.is_none());
        assert!(query.symbol_filter.is_none());
        assert!(query.exchange_filter.is_none());
        assert!(query.min_confidence.is_none());
        assert!(query.time_range.is_none());
    }

    #[test]
    fn test_signal_query_custom() {
        let query = SignalQuery {
            limit: Some(50),
            status_filter: Some(SignalStatus::Detected),
            symbol_filter: Some("BTC/USDT".to_string()),
            exchange_filter: Some(ExchangeId::OKX),
            min_confidence: Some(Decimal::new(7, 1)),
            time_range: Some((Utc::now() - Duration::hours(1), Utc::now())),
        };

        assert_eq!(query.limit, Some(50));
        assert_eq!(query.status_filter, Some(SignalStatus::Detected));
        assert_eq!(query.symbol_filter, Some("BTC/USDT".to_string()));
        assert_eq!(query.exchange_filter, Some(ExchangeId::OKX));
    }

    #[test]
    fn test_stored_signal_creation() {
        let signal = create_test_signal();
        let timestamp = Utc::now();
        let confidence_score = Decimal::from(85);
        let status = SignalStatus::Detected;

        let stored = StoredSignal {
            signal: signal.clone(),
            timestamp,
            confidence_score,
            status,
        };

        assert_eq!(stored.signal.symbol, signal.symbol);
        assert_eq!(stored.confidence_score, Decimal::from(85));
        assert_eq!(stored.status, SignalStatus::Detected);
    }

    #[test]
    fn test_stored_execution_creation() {
        let signal_id = Uuid::new_v4();
        let instruction = create_test_execution(signal_id);
        let timestamp = Utc::now();
        let status = ExecutionStatus::Pending;
        let actual_profit = Some(Decimal::from(100));
        let execution_time_ms = Some(500);

        let stored = StoredExecution {
            instruction,
            timestamp,
            status,
            actual_profit,
            execution_time_ms,
        };

        assert_eq!(stored.status, ExecutionStatus::Pending);
        assert_eq!(stored.actual_profit, Some(Decimal::from(100)));
        assert_eq!(stored.execution_time_ms, Some(500));
    }

    #[test]
    fn test_stored_execution_no_profit() {
        let signal_id = Uuid::new_v4();
        let instruction = create_test_execution(signal_id);

        let stored = StoredExecution {
            instruction,
            timestamp: Utc::now(),
            status: ExecutionStatus::Pending,
            actual_profit: None,
            execution_time_ms: None,
        };

        assert!(stored.actual_profit.is_none());
        assert!(stored.execution_time_ms.is_none());
    }

    #[test]
    fn test_signal_status_equality() {
        assert_eq!(SignalStatus::Detected, SignalStatus::Detected);
        assert_ne!(SignalStatus::Detected, SignalStatus::Filtered);
        assert_ne!(SignalStatus::Detected, SignalStatus::Executed);
    }

    #[test]
    fn test_execution_status_equality() {
        assert_eq!(ExecutionStatus::Pending, ExecutionStatus::Pending);
        assert_ne!(ExecutionStatus::Pending, ExecutionStatus::Completed);
        assert_ne!(ExecutionStatus::Pending, ExecutionStatus::Failed);
    }

    #[test]
    fn test_storage_config_debug_format() {
        let config = StorageConfig::default();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("database_path"));
        assert!(debug_str.contains("max_signal_history"));
        assert!(debug_str.contains("max_execution_history"));
    }

    #[test]
    fn test_stored_signal_debug_format() {
        let signal = create_test_signal();
        let stored = StoredSignal {
            signal,
            timestamp: Utc::now(),
            confidence_score: Decimal::from(80),
            status: SignalStatus::Detected,
        };

        let debug_str = format!("{:?}", stored);
        assert!(debug_str.contains("signal"));
        assert!(debug_str.contains("confidence_score"));
        assert!(debug_str.contains("status"));
    }

    #[test]
    fn test_stored_execution_debug_format() {
        let signal_id = Uuid::new_v4();
        let instruction = create_test_execution(signal_id);
        let stored = StoredExecution {
            instruction,
            timestamp: Utc::now(),
            status: ExecutionStatus::Pending,
            actual_profit: None,
            execution_time_ms: None,
        };

        let debug_str = format!("{:?}", stored);
        assert!(debug_str.contains("instruction"));
        assert!(debug_str.contains("status"));
    }

    #[test]
    fn test_signal_query_clone() {
        let query = SignalQuery {
            limit: Some(50),
            status_filter: Some(SignalStatus::Detected),
            symbol_filter: Some("BTC/USDT".to_string()),
            exchange_filter: Some(ExchangeId::OKX),
            min_confidence: Some(Decimal::new(7, 1)),
            time_range: Some((Utc::now() - Duration::hours(1), Utc::now())),
        };

        let cloned = query.clone();
        assert_eq!(query.limit, cloned.limit);
        assert_eq!(query.status_filter, cloned.status_filter);
    }

    #[test]
    fn test_all_signal_statuses() {
        let statuses = [
            SignalStatus::Detected,
            SignalStatus::Filtered,
            SignalStatus::Executed,
            SignalStatus::Expired,
            SignalStatus::Failed,
        ];

        for status in &statuses {
            let s = status.to_string();
            assert!(!s.is_empty());
            let parsed = SignalStatus::from_string(&s);
            assert!(parsed.is_ok());
            assert_eq!(&parsed.unwrap(), status);
        }
    }

    #[test]
    fn test_all_execution_statuses() {
        let statuses = [
            ExecutionStatus::Pending,
            ExecutionStatus::PartiallyFilled,
            ExecutionStatus::Completed,
            ExecutionStatus::Failed,
            ExecutionStatus::Cancelled,
        ];

        for status in &statuses {
            let s = status.to_string();
            assert!(!s.is_empty());
            let parsed = ExecutionStatus::from_string(&s);
            assert!(parsed.is_ok());
            assert_eq!(&parsed.unwrap(), status);
        }
    }

    #[test]
    fn test_storage_config_equality() {
        let config1 = StorageConfig::default();
        let config2 = StorageConfig::default();
        assert_eq!(config1, config2);

        let config3 = StorageConfig {
            database_path: "different.db".to_string(),
            ..StorageConfig::default()
        };
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_signal_status_hash() {
        use std::collections::HashMap;

        let mut map: HashMap<SignalStatus, &str> = HashMap::new();
        map.insert(SignalStatus::Detected, "detected");
        map.insert(SignalStatus::Filtered, "filtered");
        map.insert(SignalStatus::Executed, "executed");
        map.insert(SignalStatus::Expired, "expired");
        map.insert(SignalStatus::Failed, "failed");

        assert_eq!(map.get(&SignalStatus::Detected), Some(&"detected"));
        assert_eq!(map.get(&SignalStatus::Filtered), Some(&"filtered"));
    }

    #[test]
    fn test_execution_status_hash() {
        use std::collections::HashMap;

        let mut map: HashMap<ExecutionStatus, &str> = HashMap::new();
        map.insert(ExecutionStatus::Pending, "pending");
        map.insert(ExecutionStatus::PartiallyFilled, "partially_filled");
        map.insert(ExecutionStatus::Completed, "completed");
        map.insert(ExecutionStatus::Failed, "failed");
        map.insert(ExecutionStatus::Cancelled, "cancelled");

        assert_eq!(map.get(&ExecutionStatus::Pending), Some(&"pending"));
        assert_eq!(
            map.get(&ExecutionStatus::PartiallyFilled),
            Some(&"partially_filled")
        );
    }
}
