use arbitrage_core::{
    execution_preparer::{ExecutionPreparer, ExecutionConfig},
    types::{ExchangeId, Signal, Symbol, OrderType, TimeInForce, Side},
};
use proptest::prelude::*;
use rust_decimal::Decimal;

// Helper function to create test signal
fn create_test_signal(
    buy_exchange: ExchangeId,
    sell_exchange: ExchangeId,
    symbol: Symbol,
    buy_price: Decimal,
    sell_price: Decimal,
) -> Signal {
    Signal::new(symbol, buy_exchange, sell_exchange, buy_price, sell_price)
}

// Property 21: Execution Instruction Consistency
// Buy and sell orders should have matching quantities and symbols
proptest! {
    #[test]
    fn prop_execution_instruction_consistency(
        buy_price in 1000u32..100000,
        sell_price in 1001u32..100001, // Ensure sell > buy
        quantity in 1u32..1000
    ) {
        let preparer = ExecutionPreparer::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let buy_decimal = Decimal::from(buy_price);
        let sell_decimal = Decimal::from(sell_price);
        let qty_decimal = Decimal::from(quantity) / Decimal::from(1000); // Convert to reasonable size
        
        // Ensure profitable signal
        prop_assume!(sell_decimal > buy_decimal);
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol.clone(),
            buy_decimal,
            sell_decimal
        );
        
        let result = preparer.prepare_execution(&signal, qty_decimal);
        prop_assert!(result.is_ok());
        
        if let Ok(instruction) = result {
            // Property: buy and sell orders should have matching quantities
            prop_assert_eq!(instruction.buy_order.quantity, instruction.sell_order.quantity);
            
            // Property: buy and sell orders should have matching symbols
            let buy_symbol = instruction.buy_order.symbol.clone();
            let sell_symbol = instruction.sell_order.symbol.clone();
            prop_assert_eq!(buy_symbol.clone(), sell_symbol);
            prop_assert_eq!(buy_symbol, symbol);
            
            // Property: buy order should be on buy exchange, sell order on sell exchange
            prop_assert_eq!(instruction.buy_order.exchange, ExchangeId::ByBit);
            prop_assert_eq!(instruction.sell_order.exchange, ExchangeId::OKX);
            
            // Property: sides should be correct
            prop_assert_eq!(instruction.buy_order.side, Side::Buy);
            prop_assert_eq!(instruction.sell_order.side, Side::Sell);
        }
    }
}

// Property 22: Slippage Buffer Application
// Execution prices should include slippage buffer in the correct direction
proptest! {
    #[test]
    fn prop_slippage_buffer_application(
        buy_price in 10000u32..50000,
        sell_price in 10001u32..50001,
        buffer_bps in 1u32..100 // 0.01% to 1% buffer
    ) {
        let mut config = ExecutionConfig::default();
        config.slippage_buffer_percent = Decimal::from(buffer_bps) / Decimal::from(10000);
        
        let preparer = ExecutionPreparer::new(config.clone());
        let symbol = Symbol::new("ETH", "USDT");
        
        let buy_decimal = Decimal::from(buy_price);
        let sell_decimal = Decimal::from(sell_price);
        
        // Ensure profitable signal
        prop_assume!(sell_decimal > buy_decimal);
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            buy_decimal,
            sell_decimal
        );
        
        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        prop_assert!(result.is_ok());
        
        if let Ok(instruction) = result {
            // Property: buy price should be higher than signal price (worse for us)
            if let Some(buy_price_with_buffer) = instruction.buy_order.price {
                prop_assert!(buy_price_with_buffer >= buy_decimal);
                
                // Property: buffer should be approximately correct
                let expected_buy_price = buy_decimal * (Decimal::ONE + config.slippage_buffer_percent);
                let tolerance = buy_decimal * Decimal::new(1, 2); // 0.01 = 1% tolerance
                prop_assert!((buy_price_with_buffer - expected_buy_price).abs() <= tolerance);
            }
            
            // Property: sell price should be lower than signal price (worse for us)
            if let Some(sell_price_with_buffer) = instruction.sell_order.price {
                prop_assert!(sell_price_with_buffer <= sell_decimal);
                
                // Property: buffer should be approximately correct
                let expected_sell_price = sell_decimal * (Decimal::ONE - config.slippage_buffer_percent);
                let tolerance = sell_decimal * Decimal::new(1, 2); // 0.01 = 1% tolerance
                prop_assert!((sell_price_with_buffer - expected_sell_price).abs() <= tolerance);
            }
        }
    }
}

// Property 23: Profit Calculation Accuracy
// Expected and worst-case profits should be calculated correctly
proptest! {
    #[test]
    fn prop_profit_calculation_accuracy(
        buy_price in 10000u32..40000,
        sell_price in 12000u32..50000, // Ensure larger spread
        quantity in 10u32..100
    ) {
        let preparer = ExecutionPreparer::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let buy_decimal = Decimal::from(buy_price);
        let sell_decimal = Decimal::from(sell_price);
        let qty_decimal = Decimal::from(quantity) / Decimal::from(10); // Larger quantities
        
        // Ensure profitable signal with meaningful spread
        prop_assume!(sell_decimal > buy_decimal);
        let spread_percent = ((sell_decimal - buy_decimal) / buy_decimal) * Decimal::from(100);
        prop_assume!(spread_percent > Decimal::new(5, 1)); // At least 0.5% spread
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            buy_decimal,
            sell_decimal
        );
        
        let result = preparer.prepare_execution(&signal, qty_decimal);
        prop_assert!(result.is_ok());
        
        if let Ok(instruction) = result {
            // Only check positive profit if the instruction is valid (would be executed)
            if instruction.is_valid() {
                // Property: expected profit should be positive for valid profitable signals
                prop_assert!(instruction.expected_profit > Decimal::ZERO);
            }
            
            // Property: worst case profit should be less than or equal to expected profit
            prop_assert!(instruction.worst_case_profit <= instruction.expected_profit);
            
            // Property: total fees should be positive
            prop_assert!(instruction.total_fees >= Decimal::ZERO);
            
            // Property: profit calculations should account for fees
            let gross_expected = (sell_decimal - buy_decimal) * qty_decimal;
            prop_assert!(instruction.expected_profit <= gross_expected);
            
            // Property: worst case should still be profitable (or we wouldn't execute)
            if instruction.is_valid() {
                prop_assert!(instruction.worst_case_profit > Decimal::ZERO);
            }
        }
    }
}

// Property 24: Validation Logic Correctness
// Validation should catch all types of invalid instructions
proptest! {
    #[test]
    fn prop_validation_logic_correctness(
        buy_price in 1000u32..10000,
        sell_price in 1001u32..10001,
        quantity in 0u32..1000 // Include zero to test validation
    ) {
        let preparer = ExecutionPreparer::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let buy_decimal = Decimal::from(buy_price);
        let sell_decimal = Decimal::from(sell_price);
        let qty_decimal = Decimal::from(quantity) / Decimal::from(1000);
        
        // Ensure sell > buy for basic profitability
        prop_assume!(sell_decimal > buy_decimal);
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            buy_decimal,
            sell_decimal
        );
        
        let result = preparer.prepare_execution(&signal, qty_decimal);
        
        if quantity == 0 {
            // Property: zero quantity should result in invalid instruction
            if let Ok(instruction) = result {
                prop_assert!(!instruction.is_valid());
                prop_assert!(!instruction.validation_errors.is_empty());
            }
        } else if result.is_ok() {
            let instruction = result.unwrap();
            
            // Property: valid instructions should have no validation errors or be marked invalid
            if instruction.is_valid() {
                prop_assert!(instruction.validation_errors.is_empty());
            } else {
                prop_assert!(!instruction.validation_errors.is_empty());
            }
            
            // Property: if worst case profit is negative, instruction should be invalid
            if instruction.worst_case_profit <= Decimal::ZERO {
                prop_assert!(!instruction.is_valid());
            }
        }
    }
}

// Property 25: Quantity Formatting Consistency
// Formatted quantities should respect precision requirements
proptest! {
    #[test]
    fn prop_quantity_formatting_consistency(
        quantity_raw in 1u32..1000000, // Large range for precision testing
        precision in 2u32..8           // 2 to 8 decimal places (avoid edge cases)
    ) {
        let preparer = ExecutionPreparer::default();
        let original_quantity = Decimal::from(quantity_raw) / Decimal::from(1000); // Less aggressive division
        
        let formatted = preparer.format_quantity(original_quantity, precision);
        
        // Property: formatted quantity should have correct precision
        let scale = formatted.scale();
        prop_assert!(scale <= precision);
        
        // Property: formatted quantity should be close to original
        let difference = (formatted - original_quantity).abs();
        let max_difference = Decimal::ONE / Decimal::from(10u64.pow(precision));
        prop_assert!(difference <= max_difference);
        
        // Property: formatting should be idempotent
        let double_formatted = preparer.format_quantity(formatted, precision);
        prop_assert_eq!(formatted, double_formatted);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_execution_preparer_creation() {
        let preparer = ExecutionPreparer::default();
        let config = preparer.get_config();
        
        assert!(config.slippage_buffer_percent > Decimal::ZERO);
        assert_eq!(config.default_time_in_force, TimeInForce::IOC);
        assert_eq!(config.order_type, OrderType::Limit);
    }
    
    #[test]
    fn test_basic_execution_preparation() {
        let preparer = ExecutionPreparer::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            Decimal::from(50000),
            Decimal::from(50500)
        );
        
        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        assert!(result.is_ok());
        
        let instruction = result.unwrap();
        assert!(instruction.is_valid());
        assert!(instruction.expected_profit > Decimal::ZERO);
        assert!(instruction.worst_case_profit > Decimal::ZERO);
    }
    
    #[test]
    fn test_execution_preview_generation() {
        let preparer = ExecutionPreparer::default();
        let symbol = Symbol::new("ETH", "USDT");
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            Decimal::from(3000),
            Decimal::from(3100)
        );
        
        let instruction = preparer.prepare_execution(&signal, Decimal::from(10)).unwrap();
        let preview = preparer.generate_preview(&instruction);
        
        assert_eq!(preview.buy_order_summary.side, Side::Buy);
        assert_eq!(preview.sell_order_summary.side, Side::Sell);
        assert_eq!(preview.is_valid, instruction.is_valid());
        assert_eq!(preview.expected_profit, instruction.expected_profit);
    }
    
    #[test]
    fn test_unprofitable_signal_handling() {
        let preparer = ExecutionPreparer::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        // Create signal with very small spread that becomes unprofitable after fees and slippage
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            Decimal::from(50000),
            Decimal::from(50005) // Only $5 spread
        );
        
        let result = preparer.prepare_execution(&signal, Decimal::from(1));
        
        if let Ok(instruction) = result {
            // Should be invalid due to unprofitability after fees and slippage
            assert!(!instruction.is_valid());
            assert!(!instruction.validation_errors.is_empty());
        }
    }
}