use arbitrage_core::confidence_scorer::{ConfidenceConfig, ConfidenceScorer, FeeSchedule};
use arbitrage_core::config::TradingConfig;
use arbitrage_core::execution_preparer::{ExecutionConfig, ExecutionPreparer};
use arbitrage_core::types::{ExchangeId, OrderBook, OrderBookLevel, Signal, Symbol};
use rust_decimal::Decimal;
use std::str::FromStr;

mod division_by_zero_tests {
    use super::*;

    #[test]
    fn test_effective_buy_zero_prevents_division_by_zero() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);

        let result = scorer.calculate_net_spread_bps(
            Decimal::ZERO,
            Decimal::from(50000),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Zero buy price"));
    }

    #[test]
    fn test_effective_buy_negative_prevents_calculation() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);

        let result = scorer.calculate_net_spread_bps(
            Decimal::from(-100),
            Decimal::from(50000),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("effective_buy must be positive"));
    }

    #[test]
    fn test_net_spread_division_by_zero_when_effective_equals() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);

        let result = scorer.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50050),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        assert_eq!(result.unwrap(), -1);
    }

    #[test]
    fn test_high_fee_rates_prevent_profitable_spread() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        let high_fee_schedule = FeeSchedule::new(
            ExchangeId::OKX,
            Decimal::from_str("0.1").unwrap(),
            Decimal::from_str("0.5").unwrap(),
        );
        scorer.update_fee_schedule(ExchangeId::OKX, high_fee_schedule);

        let result = scorer.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50250),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        assert_eq!(result.unwrap(), -1);
    }
}

mod empty_orderbook_tests {
    use super::*;

    #[test]
    fn test_empty_bids_is_valid() {
        let symbol = Symbol::new("BTC", "USDT");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![],
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        );

        assert!(orderbook.is_valid());
    }

    #[test]
    fn test_empty_asks_is_valid() {
        let symbol = Symbol::new("BTC", "USDT");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49900), Decimal::from(1))],
            vec![],
        );

        assert!(orderbook.is_valid());
    }

    #[test]
    fn test_completely_empty_orderbook_is_valid() {
        let symbol = Symbol::new("BTC", "USDT");
        let orderbook = OrderBook::new(ExchangeId::OKX, symbol.clone(), vec![], vec![]);

        assert!(orderbook.is_valid());
    }

    #[test]
    fn test_orderbook_with_empty_middle_is_valid() {
        let symbol = Symbol::new("BTC", "USDT");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49900), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        );

        assert!(orderbook.is_valid());
    }
}

mod negative_fee_tests {
    use super::*;

    #[test]
    fn test_negative_maker_fee_is_rejected_in_calculation() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        let negative_fee_schedule = FeeSchedule::new(
            ExchangeId::OKX,
            Decimal::from_str("-0.001").unwrap(),
            Decimal::from_str("0.001").unwrap(),
        );
        scorer.update_fee_schedule(ExchangeId::OKX, negative_fee_schedule);

        let result = scorer.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50100),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        let spread = result.unwrap();
        assert!(spread > 0);
    }

    #[test]
    fn test_negative_taker_fee_is_rejected_in_calculation() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        let negative_fee_schedule = FeeSchedule::new(
            ExchangeId::OKX,
            Decimal::from_str("0.001").unwrap(),
            Decimal::from_str("-0.001").unwrap(),
        );
        scorer.update_fee_schedule(ExchangeId::OKX, negative_fee_schedule);

        let result = scorer.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50100),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        let spread = result.unwrap();
        assert!(spread > 0);
    }

    #[test]
    fn test_completely_negative_fees_increases_spread() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        let rebate_schedule = FeeSchedule::new(
            ExchangeId::OKX,
            Decimal::from_str("-0.002").unwrap(),
            Decimal::from_str("-0.001").unwrap(),
        );
        scorer.update_fee_schedule(ExchangeId::OKX, rebate_schedule);

        let result_with_rebate = scorer.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50100),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        let config2 = ConfidenceConfig::default();
        let scorer2 = ConfidenceScorer::new(config2);

        let result_standard = scorer2.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50100),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        assert!(result_with_rebate.unwrap() > result_standard.unwrap());
    }
}

mod future_timestamp_tests {
    use chrono::{Duration, Utc};

    #[test]
    fn test_signal_with_future_created_at_has_zero_age() {
        let future_time = Utc::now() + Duration::hours(1);
        let symbol = Symbol::new("BTC", "USDT");

        let signal = Signal::new(
            symbol,
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
        );

        let mut signal_with_future_time = signal;
        signal_with_future_time.created_at = future_time;

        let age = signal_with_future_time.age_seconds();
        assert_eq!(age, 0);
    }

    #[test]
    fn test_signal_age_is_calculated_correctly() {
        let symbol = Symbol::new("BTC", "USDT");
        let signal = Signal::new(
            symbol,
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
        );

        let age = signal.age_seconds();
        assert_eq!(age, 0);
    }

    #[test]
    fn test_signal_expires_at_is_set_by_default() {
        let symbol = Symbol::new("BTC", "USDT");
        let signal = Signal::new(
            symbol,
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
        );

        let expires_in = signal.expires_at - signal.created_at;
        let expires_minutes = expires_in.num_minutes();
        assert_eq!(expires_minutes, 5);
    }

    #[test]
    fn test_signal_is_expired_with_past_expiry() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut signal = Signal::new(
            symbol,
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
        );

        signal.expires_at = Utc::now() - Duration::minutes(10);

        assert!(signal.is_expired());
    }

    #[test]
    fn test_signal_is_not_expired_with_future_expiry() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut signal = Signal::new(
            symbol,
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
        );

        signal.expires_at = Utc::now() + Duration::minutes(10);

        assert!(!signal.is_expired());
    }
}

mod negative_profit_tests {
    use super::*;

    #[test]
    fn test_validate_instruction_rejects_negative_worst_case_profit() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(49900),
        );

        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());

        let instruction = result.unwrap();
        assert!(!instruction.is_valid());

        let has_negative_profit_error = instruction
            .validation_errors
            .iter()
            .any(|e| e.contains("not positive") || e.contains("negative") || e.contains("profit"));
        assert!(has_negative_profit_error);
    }

    #[test]
    fn test_validate_instruction_with_zero_profit() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50000),
        );

        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());

        let instruction = result.unwrap();
        assert!(!instruction.is_valid());
    }

    #[test]
    fn test_validate_instruction_negative_profit_error_message_is_clear() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(49000),
        );

        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());

        let instruction = result.unwrap();

        let negative_profit_error = instruction
            .validation_errors
            .iter()
            .find(|e| e.contains("not positive") || e.contains("negative") || e.contains("profit"));

        assert!(negative_profit_error.is_some());
        let error_msg = negative_profit_error.unwrap();
        assert!(!error_msg.is_empty());
        assert!(
            error_msg.contains("profit") || error_msg.contains("positive"),
            "Error message should mention profit or positive: {}",
            error_msg
        );
    }
}

mod force_execute_tests {
    use super::*;

    #[test]
    fn test_force_execute_config_allows_negative_profit() {
        let config = ExecutionConfig {
            enable_force_execute: true,
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(49900),
        );

        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());

        let instruction = result.unwrap();
        assert!(instruction.is_valid());
    }

    #[test]
    fn test_force_execute_disabled_by_default() {
        let config = ExecutionConfig::default();
        assert!(!config.enable_force_execute);
    }

    #[test]
    fn test_force_execute_enabled_manually() {
        let config = ExecutionConfig {
            enable_force_execute: true,
            ..ExecutionConfig::default()
        };
        assert!(config.enable_force_execute);
    }

    #[test]
    fn test_trading_config_force_execute_default() {
        let trading_config = TradingConfig::default();
        assert!(!trading_config.force_execute_production);
    }

    #[test]
    fn test_force_execute_in_production_mode_is_guard() {
        let trading_config = TradingConfig {
            force_execute_production: false,
            ..TradingConfig::default()
        };

        let config = ExecutionConfig {
            enable_force_execute: true,
            ..ExecutionConfig::default()
        };

        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(49900),
        );

        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());

        let instruction = result.unwrap();
        assert!(!instruction.is_valid());

        let has_guard_error = instruction.validation_errors.iter().any(|e| {
            e.contains("force_execute") || e.contains("production") || e.contains("override")
        });
        assert!(has_guard_error);
    }
}

mod validation_error_message_tests {
    use super::*;

    #[test]
    fn test_zero_buy_price_error_message_is_clear() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);

        let result = scorer.calculate_net_spread_bps(
            Decimal::ZERO,
            Decimal::from(50000),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("Zero") || err_str.contains("buy") || err_str.contains("price"),
            "Error message should mention zero, buy, or price: {}",
            err_str
        );
    }

    #[test]
    fn test_negative_profit_error_message_contains_profit() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(49900),
        );

        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());

        let instruction = result.unwrap();

        let profit_error = instruction
            .validation_errors
            .iter()
            .find(|e| e.contains("profit"));
        assert!(
            profit_error.is_some(),
            "Should have an error mentioning profit, got: {:?}",
            instruction.validation_errors
        );
    }

    #[test]
    fn test_force_execute_error_message_is_clear() {
        let trading_config = TradingConfig {
            force_execute_production: false,
            ..TradingConfig::default()
        };

        let config = ExecutionConfig {
            enable_force_execute: true,
            ..ExecutionConfig::default()
        };

        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            Symbol::new("BTC", "USDT"),
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(49900),
        );

        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());

        let instruction = result.unwrap();

        let force_execute_error = instruction
            .validation_errors
            .iter()
            .find(|e| e.contains("force_execute") || e.contains("production"));
        assert!(
            force_execute_error.is_some(),
            "Should have an error mentioning force_execute or production, got: {:?}",
            instruction.validation_errors
        );
    }
}
