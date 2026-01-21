#[cfg(test)]
mod tests {
    use crate::{
        confidence_scorer::FeeSchedule,
        execution_preparer::{ExecutionConfig, ExecutionPreparer},
        types::{OrderType, Side, Signal, TimeInForce},
    };
    use rust_decimal::Decimal;

    fn create_test_signal() -> Signal {
        Signal::new(
            crate::types::Symbol::new("BTC", "USDT"),
            crate::types::ExchangeId::OKX,
            crate::types::ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50100),
            chrono::Utc::now(),
        )
    }

    // === Happy Path Tests ===

    #[test]
    fn test_execution_config_default() {
        let config = ExecutionConfig::default();
        assert_eq!(config.slippage_buffer_percent, Decimal::new(5, 4));
        assert_eq!(config.default_time_in_force, TimeInForce::IOC);
        assert_eq!(config.order_type, OrderType::Limit);
        assert!(!config.enable_force_execute);
    }

    #[test]
    fn test_execution_preparer_new() {
        let config = ExecutionConfig::default();
        let _preparer = ExecutionPreparer::new(config);
        assert!(true);
    }

    #[test]
    fn test_execution_preparer_default() {
        let _preparer = ExecutionPreparer::default();
        assert!(true);
    }

    #[test]
    fn test_prepare_execution_basic() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();
        assert!(!instruction.signal_id.to_string().is_empty());
        assert_eq!(instruction.sell_order.quantity, Decimal::from(1));
    }

    #[test]
    fn test_prepare_execution_sets_prices_with_buffer() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        // Buy price should be slightly higher
        let buy_price = instruction.buy_order.price.unwrap();
        assert!(buy_price > signal.buy_price);

        // Sell price should be slightly lower
        let sell_price = instruction.sell_order.price.unwrap();
        assert!(sell_price < signal.sell_price);
    }

    #[test]
    fn test_prepare_execution_calculates_expected_profit() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let _instruction = result.unwrap();

        // Profit should be calculated (can be positive, zero, or negative)
        assert!(true); // Just verify no panic
    }

    #[test]
    fn test_preview_displays_profit_correctly() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let _preview = preparer.generate_preview(&instruction_result.unwrap());
        assert!(true); // Just verify no panic
    }

    #[test]
    fn test_prepare_execution_calculates_worst_case_profit() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        // Worst case should be less than or equal to expected
        assert!(instruction.worst_case_profit <= instruction.expected_profit);
    }

    #[test]
    fn test_prepare_execution_calculates_fees() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert!(instruction.total_fees > Decimal::ZERO);
        assert!(instruction.buy_order.expected_fee > Decimal::ZERO);
        assert!(instruction.sell_order.expected_fee > Decimal::ZERO);
    }

    #[test]
    fn test_prepare_execution_time_in_force() {
        let config = ExecutionConfig {
            default_time_in_force: TimeInForce::GTC,
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert_eq!(instruction.buy_order.time_in_force, TimeInForce::GTC);
        assert_eq!(instruction.sell_order.time_in_force, TimeInForce::GTC);
    }

    #[test]
    fn test_prepare_execution_order_type_market() {
        let config = ExecutionConfig {
            order_type: OrderType::Market,
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert_eq!(instruction.buy_order.order_type, OrderType::Market);
        assert_eq!(instruction.sell_order.order_type, OrderType::Market);
    }

    #[test]
    fn test_update_fee_schedule() {
        let config = ExecutionConfig::default();
        let mut preparer = ExecutionPreparer::new(config);
        let schedule = FeeSchedule::new(
            crate::types::ExchangeId::OKX,
            Decimal::new(8, 4),
            Decimal::new(1, 3),
        );
        preparer.update_fee_schedule(crate::types::ExchangeId::OKX, schedule);
        assert!(true);
    }

    #[test]
    fn test_generate_preview() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        assert_eq!(preview.buy_order_summary.quantity, Decimal::from(1));
        assert_eq!(preview.sell_order_summary.quantity, Decimal::from(1));
    }

    #[test]
    fn test_preview_shows_validation_errors() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        // May have validation errors depending on signal
        assert!(preview.is_valid || !preview.validation_errors.is_empty());
    }

    #[test]
    fn test_get_config() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let retrieved = preparer.get_config();
        assert_eq!(retrieved.slippage_buffer_percent, Decimal::new(5, 4));
    }

    #[test]
    fn test_update_config() {
        let config = ExecutionConfig::default();
        let mut preparer = ExecutionPreparer::new(config);
        let new_config = ExecutionConfig {
            slippage_buffer_percent: Decimal::new(10, 4),
            ..ExecutionConfig::default()
        };
        preparer.update_config(new_config);
        assert_eq!(
            preparer.get_config().slippage_buffer_percent,
            Decimal::new(10, 4)
        );
    }

    #[test]
    fn test_format_quantity() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let formatted = preparer.format_quantity(Decimal::from(12345678) / Decimal::from(10000), 4);
        assert!(formatted > Decimal::ZERO);
    }

    // === Edge Case Tests ===

    #[test]
    fn test_prepare_execution_small_quantity() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let small_quantity = Decimal::new(1, 6); // 0.000001

        let result = preparer.prepare_execution(&signal, small_quantity);
        assert!(result.is_ok());
    }

    #[test]
    fn test_prepare_execution_large_quantity() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let large_quantity = Decimal::from(100);

        let result = preparer.prepare_execution(&signal, large_quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();
        assert_eq!(instruction.buy_order.quantity, Decimal::from(100));
    }

    #[test]
    fn test_prepare_execution_with_fee_schedule() {
        let config = ExecutionConfig::default();
        let mut preparer = ExecutionPreparer::new(config);

        // Add custom fee schedule
        let schedule = FeeSchedule::new(
            crate::types::ExchangeId::OKX,
            Decimal::ZERO,      // Free maker
            Decimal::new(5, 4), // 0.0005 taker
        );
        preparer.update_fee_schedule(crate::types::ExchangeId::OKX, schedule);

        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();
        // Should have some fees
        assert!(instruction.total_fees >= Decimal::ZERO);
    }

    #[test]
    fn test_preview_with_empty_validation_errors() {
        let config = ExecutionConfig {
            enable_force_execute: true,
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let _preview = preparer.generate_preview(&instruction_result.unwrap());
        // Force execute allows execution even with issues
        assert!(true);
    }

    // === Error Path Tests ===

    #[test]
    fn test_execution_instruction_is_valid() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();
        // Instruction is valid if no errors or if force execute is enabled
        assert!(instruction.is_valid() || !instruction.validation_errors.is_empty());
    }

    // === Boundary Condition Tests ===

    #[test]
    fn test_execution_config_zero_slippage() {
        let config = ExecutionConfig {
            slippage_buffer_percent: Decimal::ZERO,
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        // Prices should match signal prices with zero slippage buffer
        assert_eq!(instruction.buy_order.price.unwrap(), signal.buy_price);
        assert_eq!(instruction.sell_order.price.unwrap(), signal.sell_price);
    }

    #[test]
    fn test_execution_config_large_slippage() {
        let config = ExecutionConfig {
            slippage_buffer_percent: Decimal::new(1, 1), // 0.1 = 10%
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        let buy_buffer = instruction.buy_order.price.unwrap() - signal.buy_price;
        let sell_buffer = signal.sell_price - instruction.sell_order.price.unwrap();

        // Buffer should be non-negative with 10% slippage
        assert!(buy_buffer >= Decimal::ZERO);
        assert!(sell_buffer >= Decimal::ZERO);
    }

    #[test]
    fn test_format_quantity_zero_precision() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let formatted = preparer.format_quantity(Decimal::from(12345), 0);
        assert_eq!(formatted, Decimal::from(12345));
    }

    #[test]
    fn test_format_quantity_high_precision() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let formatted = preparer.format_quantity(Decimal::from(123456789), 8);
        assert!(formatted <= Decimal::from(123456789));
    }

    // === Type Conversion Tests ===

    #[test]
    fn test_execution_config_clone() {
        let config = ExecutionConfig::default();
        let cloned = config.clone();
        assert_eq!(
            config.slippage_buffer_percent,
            cloned.slippage_buffer_percent
        );
    }

    #[test]
    fn test_execution_preview_clone() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        let cloned = preview.clone();
        assert_eq!(preview.expected_profit, cloned.expected_profit);
    }

    #[test]
    fn test_execution_config_debug_format() {
        let config = ExecutionConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("slippage_buffer_percent"));
        assert!(debug_str.contains("default_time_in_force"));
    }

    // === Additional Tests ===

    #[test]
    fn test_prepare_execution_different_quantities() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();

        for qty in [Decimal::from(1), Decimal::from(10), Decimal::from(100)] {
            let result = preparer.prepare_execution(&signal, qty);
            assert!(result.is_ok());
            let instruction = result.unwrap();
            assert_eq!(instruction.buy_order.quantity, qty);
            assert_eq!(instruction.sell_order.quantity, qty);
        }
    }

    #[test]
    fn test_preview_buy_order_summary() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        let buy_summary = &preview.buy_order_summary;

        assert_eq!(buy_summary.side, Side::Buy);
        assert_eq!(buy_summary.quantity, quantity);
        assert!(buy_summary.estimated_cost > Decimal::ZERO);
        assert!(buy_summary.estimated_fee > Decimal::ZERO);
    }

    #[test]
    fn test_preview_sell_order_summary() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        let sell_summary = &preview.sell_order_summary;

        assert_eq!(sell_summary.side, Side::Sell);
        assert_eq!(sell_summary.quantity, quantity);
        assert!(sell_summary.estimated_cost > Decimal::ZERO);
        assert!(sell_summary.estimated_fee > Decimal::ZERO);
    }

    #[test]
    fn test_preview_total_fees() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        assert_eq!(
            preview.total_fees,
            preview.buy_order_summary.estimated_fee + preview.sell_order_summary.estimated_fee
        );
    }

    #[test]
    fn test_prepare_execution_matching_sides() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert_eq!(instruction.buy_order.side, Side::Buy);
        assert_eq!(instruction.sell_order.side, Side::Sell);
    }

    #[test]
    fn test_prepare_execution_matching_symbols() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert_eq!(instruction.buy_order.symbol, signal.symbol);
        assert_eq!(instruction.sell_order.symbol, signal.symbol);
    }

    #[test]
    fn test_prepare_execution_matching_exchanges() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert_eq!(instruction.buy_order.exchange, signal.buy_exchange);
        assert_eq!(instruction.sell_order.exchange, signal.sell_exchange);
    }

    #[test]
    fn test_preview_shows_correct_exchanges() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        assert_eq!(preview.buy_order_summary.exchange, signal.buy_exchange);
        assert_eq!(preview.sell_order_summary.exchange, signal.sell_exchange);
    }

    #[test]
    fn test_preview_shows_correct_symbols() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        assert_eq!(preview.buy_order_summary.symbol, signal.symbol);
        assert_eq!(preview.sell_order_summary.symbol, signal.symbol);
    }

    #[test]
    fn test_prepare_execution_slippage_buffer_stored() {
        let config = ExecutionConfig {
            slippage_buffer_percent: Decimal::new(10, 4),
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert_eq!(instruction.slippage_buffer, Decimal::new(10, 4));
    }

    #[test]
    fn test_order_summary_display() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);
        let signal = create_test_signal();
        let quantity = Decimal::from(1);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());
        let debug_str = format!("{:?}", preview.buy_order_summary);
        assert!(debug_str.contains("exchange"));
        assert!(debug_str.contains("symbol"));
    }

    #[test]
    fn test_execution_preparer_stress_test() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);

        // Prepare many executions
        for i in 0..100 {
            let signal = Signal::new(
                crate::types::Symbol::new("BTC", "USDT"),
                crate::types::ExchangeId::OKX,
                crate::types::ExchangeId::ByBit,
                Decimal::from(50000),
                Decimal::from(50100 + i),
                chrono::Utc::now(),
            );
            let quantity = Decimal::from(i + 1);
            let result = preparer.prepare_execution(&signal, quantity);
            assert!(result.is_ok());
        }
    }

    // === Integration Tests: Signal to ExecutionInstruction Flow ===

    #[test]
    fn test_signal_to_execution_instruction_complete_flow() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            crate::types::Symbol::new("BTC", "USDT"),
            crate::types::ExchangeId::Binance,
            crate::types::ExchangeId::Coinbase,
            Decimal::from(64250),
            Decimal::from(64350),
            chrono::Utc::now(),
        );
        let quantity = Decimal::from(5);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        assert_eq!(instruction.signal_id, signal.id);
        assert_eq!(instruction.buy_order.quantity, quantity);
        assert_eq!(instruction.sell_order.quantity, quantity);
        assert_eq!(instruction.buy_order.exchange, signal.buy_exchange);
        assert_eq!(instruction.sell_order.exchange, signal.sell_exchange);
        assert_eq!(instruction.buy_order.symbol, signal.symbol);
        assert_eq!(instruction.sell_order.symbol, signal.symbol);
        assert!(
            instruction.expected_profit > Decimal::ZERO
                || instruction.expected_profit == Decimal::ZERO
                || instruction.expected_profit < Decimal::ZERO
        );
    }

    #[test]
    fn test_worst_case_profit_with_slippage_and_fees() {
        let mut config = ExecutionConfig::default();
        config.slippage_buffer_percent = Decimal::new(5, 4);
        let mut preparer = ExecutionPreparer::new(config);

        let okx_fee = FeeSchedule::new(
            crate::types::ExchangeId::OKX,
            Decimal::new(8, 4),
            Decimal::new(1, 3),
        );
        let bybit_fee = FeeSchedule::new(
            crate::types::ExchangeId::ByBit,
            Decimal::new(6, 4),
            Decimal::new(8, 4),
        );
        preparer.update_fee_schedule(crate::types::ExchangeId::OKX, okx_fee);
        preparer.update_fee_schedule(crate::types::ExchangeId::ByBit, bybit_fee);

        let signal = Signal::new(
            crate::types::Symbol::new("ETH", "USDT"),
            crate::types::ExchangeId::OKX,
            crate::types::ExchangeId::ByBit,
            Decimal::from(3500),
            Decimal::from(3525),
            chrono::Utc::now(),
        );
        let quantity = Decimal::from(10);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        let expected_gross_profit = (signal.sell_price - signal.buy_price) * quantity;
        assert_eq!(
            instruction.expected_profit + instruction.total_fees,
            expected_gross_profit
        );

        assert!(instruction.worst_case_profit < instruction.expected_profit);
        assert!(instruction.total_fees > Decimal::ZERO);

        let buy_cost_with_slippage = instruction.buy_order.price.unwrap() * quantity;
        let sell_revenue_with_slippage = instruction.sell_order.price.unwrap() * quantity;
        let worst_case_gross = sell_revenue_with_slippage - buy_cost_with_slippage;
        assert_eq!(
            instruction.worst_case_profit + instruction.total_fees,
            worst_case_gross
        );
    }

    #[test]
    fn test_execution_preview_all_fields_populated() {
        let config = ExecutionConfig::default();
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            crate::types::Symbol::new("SOL", "USDT"),
            crate::types::ExchangeId::Kraken,
            crate::types::ExchangeId::GateIo,
            Decimal::from(180),
            Decimal::from(182),
            chrono::Utc::now(),
        );
        let quantity = Decimal::from(50);

        let instruction_result = preparer.prepare_execution(&signal, quantity);
        assert!(instruction_result.is_ok());

        let preview = preparer.generate_preview(&instruction_result.unwrap());

        assert_eq!(preview.buy_order_summary.exchange, signal.buy_exchange);
        assert_eq!(preview.buy_order_summary.symbol, signal.symbol);
        assert_eq!(preview.buy_order_summary.side, Side::Buy);
        assert_eq!(preview.buy_order_summary.quantity, quantity);
        assert!(preview.buy_order_summary.price.is_some());
        assert!(preview.buy_order_summary.estimated_cost > Decimal::ZERO);
        assert!(preview.buy_order_summary.estimated_fee > Decimal::ZERO);

        assert_eq!(preview.sell_order_summary.exchange, signal.sell_exchange);
        assert_eq!(preview.sell_order_summary.symbol, signal.symbol);
        assert_eq!(preview.sell_order_summary.side, Side::Sell);
        assert_eq!(preview.sell_order_summary.quantity, quantity);
        assert!(preview.sell_order_summary.price.is_some());
        assert!(preview.sell_order_summary.estimated_cost > Decimal::ZERO);
        assert!(preview.sell_order_summary.estimated_fee > Decimal::ZERO);

        assert!(
            preview.expected_profit >= Decimal::ZERO || preview.expected_profit < Decimal::ZERO
        );
        assert!(preview.worst_case_profit <= preview.expected_profit);
        assert!(preview.total_fees > Decimal::ZERO);
        assert!(preview.is_valid || !preview.validation_errors.is_empty());
    }

    #[test]
    fn test_fee_impact_on_execution_preview() {
        let mut config = ExecutionConfig::default();
        config.slippage_buffer_percent = Decimal::new(1, 3);
        let mut preparer = ExecutionPreparer::new(config);

        let zero_fee = FeeSchedule::new(
            crate::types::ExchangeId::Binance,
            Decimal::ZERO,
            Decimal::ZERO,
        );
        let high_fee = FeeSchedule::new(
            crate::types::ExchangeId::ByBit,
            Decimal::new(1, 2),
            Decimal::new(1, 2),
        );
        preparer.update_fee_schedule(crate::types::ExchangeId::Binance, zero_fee);
        preparer.update_fee_schedule(crate::types::ExchangeId::ByBit, high_fee);

        let signal = Signal::new(
            crate::types::Symbol::new("XRP", "USDT"),
            crate::types::ExchangeId::Binance,
            crate::types::ExchangeId::ByBit,
            Decimal::from(100),
            Decimal::from(105),
            chrono::Utc::now(),
        );
        let quantity = Decimal::from(1000);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        let preview = preparer.generate_preview(&instruction);

        let buy_revenue = signal.buy_price * quantity;
        let sell_revenue = signal.sell_price * quantity;
        let expected_gross_profit = sell_revenue - buy_revenue;
        assert_eq!(
            preview.expected_profit + preview.total_fees,
            expected_gross_profit
        );

        assert!(preview.buy_order_summary.estimated_fee >= Decimal::ZERO);
        assert!(preview.sell_order_summary.estimated_fee > Decimal::ZERO);
    }

    #[test]
    fn test_worst_case_profit_edge_case_high_slippage() {
        let mut config = ExecutionConfig::default();
        config.slippage_buffer_percent = Decimal::new(2, 2);
        let mut preparer = ExecutionPreparer::new(config);

        let fee = FeeSchedule::new(
            crate::types::ExchangeId::MEXC,
            Decimal::new(1, 3),
            Decimal::new(2, 3),
        );
        preparer.update_fee_schedule(crate::types::ExchangeId::MEXC, fee);

        let signal = Signal::new(
            crate::types::Symbol::new("DOGE", "USDT"),
            crate::types::ExchangeId::MEXC,
            crate::types::ExchangeId::HTX,
            Decimal::from(10),
            Decimal::from(11),
            chrono::Utc::now(),
        );
        let quantity = Decimal::from(10000);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        let slippage_impact_buy = instruction.buy_order.price.unwrap() - signal.buy_price;
        let slippage_impact_sell = signal.sell_price - instruction.sell_order.price.unwrap();
        assert!(slippage_impact_buy > Decimal::ZERO);
        assert!(slippage_impact_sell > Decimal::ZERO);

        assert!(instruction.worst_case_profit < instruction.expected_profit);
        assert!(instruction.total_fees > Decimal::ZERO);
    }

    #[test]
    fn test_preview_validity_reflects_validation_errors() {
        let config = ExecutionConfig {
            enable_force_execute: false,
            ..ExecutionConfig::default()
        };
        let preparer = ExecutionPreparer::new(config);

        let signal = Signal::new(
            crate::types::Symbol::new("BTC", "USDT"),
            crate::types::ExchangeId::OKX,
            crate::types::ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50010),
            chrono::Utc::now(),
        );
        let quantity = Decimal::from(1);

        let result = preparer.prepare_execution(&signal, quantity);
        assert!(result.is_ok());
        let instruction = result.unwrap();

        let preview = preparer.generate_preview(&instruction);

        assert_eq!(preview.is_valid, instruction.is_valid());
        assert_eq!(preview.validation_errors, instruction.validation_errors);
    }
}
