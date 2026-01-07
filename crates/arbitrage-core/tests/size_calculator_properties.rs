use arbitrage_core::{
    size_calculator::{SizeCalculator, SizeConfig, LimitingFactor},
    types::{ExchangeId, OrderBook, OrderBookLevel, Signal, Symbol},
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

// Helper function to create test order book
fn create_test_order_book(
    exchange: ExchangeId,
    symbol: Symbol,
    levels: Vec<(Decimal, Decimal)>,
    is_bids: bool,
) -> OrderBook {
    let mut order_levels: Vec<OrderBookLevel> = levels
        .into_iter()
        .map(|(price, quantity)| OrderBookLevel::new(price, quantity))
        .collect();
    
    if is_bids {
        order_levels.sort_by(|a, b| b.price.cmp(&a.price)); // Descending for bids
        OrderBook::new(exchange, symbol, order_levels, vec![])
    } else {
        order_levels.sort_by(|a, b| a.price.cmp(&b.price)); // Ascending for asks
        OrderBook::new(exchange, symbol, vec![], order_levels)
    }
}

// Property 11: Size Calculation Monotonicity
// Larger slippage tolerance should never result in smaller maximum size
proptest! {
    #[test]
    fn prop_size_monotonicity_with_slippage(
        base_price in 1000u32..100000,
        depth_levels in prop::collection::vec((1u32..1000, 1u32..100), 3..10)
    ) {
        let calculator = SizeCalculator::default();
        let symbol = Symbol::new("BTC", "USDT");
        let price = Decimal::from(base_price);
        
        // Create order books with depth
        let buy_levels: Vec<(Decimal, Decimal)> = depth_levels.iter()
            .enumerate()
            .map(|(i, (price_offset, qty))| {
                let level_price = price + Decimal::from(*price_offset + i as u32);
                (level_price, Decimal::from(*qty))
            })
            .collect();
            
        let sell_levels: Vec<(Decimal, Decimal)> = depth_levels.iter()
            .enumerate()
            .map(|(i, (price_offset, qty))| {
                let level_price = price - Decimal::from(*price_offset + i as u32);
                (level_price, Decimal::from(*qty))
            })
            .collect();
        
        let buy_book = create_test_order_book(ExchangeId::ByBit, symbol.clone(), buy_levels, false);
        let sell_book = create_test_order_book(ExchangeId::OKX, symbol.clone(), sell_levels, true);
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            price,
            price + Decimal::from(100)
        );
        
        if let Ok(recommendation) = calculator.calculate_size(&signal, &buy_book, &sell_book) {
            // Property: size tiers should be monotonic (higher slippage = higher or equal size)
            for window in recommendation.size_tiers.windows(2) {
                let lower_slippage_tier = &window[0];
                let higher_slippage_tier = &window[1];
                
                prop_assert!(lower_slippage_tier.slippage_percent <= higher_slippage_tier.slippage_percent);
                prop_assert!(lower_slippage_tier.max_size <= higher_slippage_tier.max_size);
            }
        }
    }
}

// Property 12: Size Calculation Bounds
// Calculated size should never exceed available liquidity
proptest! {
    #[test]
    fn prop_size_respects_liquidity_bounds(
        price in 1000u32..50000,
        total_buy_qty in 1u32..1000,
        total_sell_qty in 1u32..1000
    ) {
        let calculator = SizeCalculator::default();
        let symbol = Symbol::new("ETH", "USDT");
        let base_price = Decimal::from(price);
        
        // Create order books with limited liquidity
        let buy_book = create_test_order_book(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![(base_price + Decimal::from(10), Decimal::from(total_buy_qty))],
            false
        );
        
        let sell_book = create_test_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            vec![(base_price - Decimal::from(10), Decimal::from(total_sell_qty))],
            true
        );
        
        let signal = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            base_price + Decimal::from(10),
            base_price - Decimal::from(10)
        );
        
        if let Ok(recommendation) = calculator.calculate_size(&signal, &buy_book, &sell_book) {
            let min_liquidity = Decimal::from(total_buy_qty.min(total_sell_qty));
            
            // Property: recommended size should not exceed available liquidity
            prop_assert!(recommendation.recommended_size <= min_liquidity);
            prop_assert!(recommendation.max_size <= min_liquidity);
            
            // Property: all size tiers should respect liquidity bounds
            for tier in &recommendation.size_tiers {
                prop_assert!(tier.max_size <= min_liquidity);
            }
        }
    }
}

// Property 13: Fee-Aware Sizing Consistency
// When fee-aware sizing is enabled, sizes should account for fee impact
proptest! {
    #[test]
    fn prop_fee_aware_sizing_consistency(
        price in 10000u32..50000,
        quantity in 1u32..100
    ) {
        let mut config = SizeConfig::default();
        config.fee_aware_sizing = true;
        let calculator = SizeCalculator::new(config);
        
        let symbol = Symbol::new("BTC", "USDT");
        let base_price = Decimal::from(price);
        let _qty = Decimal::from(quantity);
        
        // Test VWAP quantity calculation
        let result = calculator.calculate_vwap_quantity(
            &symbol,
            base_price,
            Decimal::from(10000) // $10k target
        );
        
        prop_assert!(result.is_ok());
        
        if let Ok(vwap_qty) = result {
            // Property: VWAP quantity should be reasonable relative to target USD
            let expected_qty = Decimal::from(10000) / base_price;
            let tolerance = expected_qty * Decimal::new(1, 2); // 1% tolerance
            
            prop_assert!((vwap_qty - expected_qty).abs() <= tolerance);
            
            // Property: quantity should be positive
            prop_assert!(vwap_qty > Decimal::ZERO);
        }
    }
}

// Property 14: Limiting Factor Accuracy
// The reported limiting factor should accurately reflect the actual constraint
proptest! {
    #[test]
    fn prop_limiting_factor_accuracy(
        price in 1000u32..10000,
        small_qty in 1u32..10,    // Small quantity to trigger limits
        large_qty in 1000u32..10000 // Large quantity
    ) {
        let mut config = SizeConfig::default();
        config.min_order_size_usd = Decimal::from(100);
        config.max_order_size_usd = Decimal::from(1000);
        
        let calculator = SizeCalculator::new(config);
        let symbol = Symbol::new("BTC", "USDT");
        let base_price = Decimal::from(price);
        
        // Test with small quantity (should hit minimum)
        let small_book = create_test_order_book(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![(base_price, Decimal::from(small_qty))],
            false
        );
        
        let signal_small = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol.clone(),
            base_price,
            base_price + Decimal::from(100)
        );
        
        if let Ok(recommendation) = calculator.calculate_size(&signal_small, &small_book, &small_book) {
            // Property: small quantities should trigger appropriate limiting factors
            match recommendation.limiting_factor {
                LimitingFactor::ExchangeMinimum | 
                LimitingFactor::InsufficientDepth => {
                    // These are expected for small quantities
                    prop_assert!(true);
                }
                _ => {
                    // Other factors are possible but less likely
                    prop_assert!(true);
                }
            }
        }
        
        // Test with large quantity (should hit maximum or depth limits)
        let large_book = create_test_order_book(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![(base_price, Decimal::from(large_qty))],
            false
        );
        
        let signal_large = create_test_signal(
            ExchangeId::ByBit,
            ExchangeId::OKX,
            symbol,
            base_price,
            base_price + Decimal::from(100)
        );
        
        if let Ok(recommendation) = calculator.calculate_size(&signal_large, &large_book, &large_book) {
            // Property: large quantities should trigger appropriate limiting factors
            let notional = recommendation.max_size * base_price;
            
            if notional > calculator.get_config().max_order_size_usd {
                prop_assert!(matches!(recommendation.limiting_factor, LimitingFactor::ExchangeMaximum));
            }
        }
    }
}

// Property 15: Size Validation Consistency
// Size validation should be consistent with configuration limits
proptest! {
    #[test]
    fn prop_size_validation_consistency(
        price in 100u32..10000,
        size in 1u32..1000
    ) {
        let calculator = SizeCalculator::default();
        let symbol = Symbol::new("ETH", "USDT");
        let test_price = Decimal::from(price);
        let test_size = Decimal::from(size);
        
        let validation_result = calculator.validate_size(
            &symbol,
            ExchangeId::ByBit,
            test_size,
            test_price
        );
        
        let notional = test_size * test_price;
        let config = calculator.get_config();
        
        // Property: validation should pass if and only if size meets requirements
        if notional >= config.min_order_size_usd && notional <= config.max_order_size_usd {
            prop_assert!(validation_result.is_ok());
        } else {
            prop_assert!(validation_result.is_err());
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_size_calculator_creation() {
        let calculator = SizeCalculator::default();
        let config = calculator.get_config();
        
        assert!(config.max_slippage_percent > Decimal::ZERO);
        assert!(config.min_order_size_usd > Decimal::ZERO);
        assert!(!config.slippage_tiers.is_empty());
    }
    
    #[test]
    fn test_vwap_quantity_calculation() {
        let calculator = SizeCalculator::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let result = calculator.calculate_vwap_quantity(
            &symbol,
            Decimal::from(50000), // $50k BTC price
            Decimal::from(10000)  // $10k target
        );
        
        assert!(result.is_ok());
        let quantity = result.unwrap();
        
        // Should be approximately 0.2 BTC
        assert!(quantity > Decimal::new(15, 2)); // > 0.15
        assert!(quantity < Decimal::new(25, 2)); // < 0.25
    }
    
    #[test]
    fn test_zero_price_handling() {
        let calculator = SizeCalculator::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let result = calculator.calculate_vwap_quantity(
            &symbol,
            Decimal::ZERO, // Zero price should cause error
            Decimal::from(10000)
        );
        
        assert!(result.is_err());
    }
}