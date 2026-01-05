use arbitrage_core::{
    config::SymbolMapping,
    normalizer::Normalizer,
    types::{ExchangeId, OrderBookLevel, Symbol},
};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::collections::HashMap;

// Property 5: Symbol Mapping Correctness
// For any valid symbol mapping, the normalizer should correctly map exchange symbols to canonical format
proptest! {
    #[test]
    fn prop_symbol_mapping_correctness(
        base in "[A-Z]{3,5}",
        quote in "[A-Z]{3,5}",
        exchange_symbol in "[A-Z0-9-/]{3,10}"
    ) {
        let canonical = Symbol::new(&base, &quote);
        let mut normalizer = Normalizer::new();
        
        // Create a mapping
        let mut mapping = SymbolMapping::new(canonical.clone());
        mapping.exchange_symbols.insert(ExchangeId::ByBit, exchange_symbol.clone());
        
        normalizer.load_symbol_mappings(vec![mapping]);
        
        // Test that mapping works correctly
        let result = normalizer.map_symbol(ExchangeId::ByBit, &exchange_symbol);
        prop_assert_eq!(result, Some(canonical));
    }
}

// Property 6: Unknown Symbol Handling
// For any symbol not in the mapping, the normalizer should return None
proptest! {
    #[test]
    fn prop_unknown_symbol_handling(
        unknown_symbol in "[A-Z0-9-/]{3,10}"
    ) {
        let normalizer = Normalizer::new(); // Empty normalizer
        
        // Test that unknown symbols return None
        let result = normalizer.map_symbol(ExchangeId::ByBit, &unknown_symbol);
        prop_assert_eq!(result, None);
    }
}

// Property 7: Precision Minimum Selection
// The normalizer should always use the minimum precision across exchanges
proptest! {
    #[test]
    fn prop_precision_minimum_selection(
        precision1 in 0u32..8,
        precision2 in 0u32..8,
        precision3 in 0u32..8
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let mut mapping = SymbolMapping::new(symbol.clone());
        
        mapping.precision.insert(ExchangeId::ByBit, precision1);
        mapping.precision.insert(ExchangeId::BingX, precision2);
        mapping.precision.insert(ExchangeId::Hyperliquid, precision3);
        
        let mut normalizer = Normalizer::new();
        normalizer.load_symbol_mappings(vec![mapping]);
        
        let min_precision = normalizer.get_min_precision(&symbol);
        let expected_min = [precision1, precision2, precision3].iter().min().copied();
        
        prop_assert_eq!(min_precision, expected_min);
    }
}

// Property 1: Order Book Normalization Validity
// Any normalized order book should maintain the bid < ask invariant
proptest! {
    #[test]
    fn prop_order_book_normalization_validity(
        bids in prop::collection::vec((1u32..100000, 1u32..1000), 1..10),
        asks in prop::collection::vec((100001u32..200000, 1u32..1000), 1..10)
    ) {
        let normalizer = Normalizer::new();
        
        // Convert to (Decimal, Decimal) tuples
        let bid_levels: Vec<(Decimal, Decimal)> = bids.into_iter()
            .map(|(p, q)| (Decimal::from(p), Decimal::from(q)))
            .collect();
        let ask_levels: Vec<(Decimal, Decimal)> = asks.into_iter()
            .map(|(p, q)| (Decimal::from(p), Decimal::from(q)))
            .collect();
        
        // Create a symbol mapping for testing
        let symbol = Symbol::new("BTC", "USDT");
        let mut mapping = SymbolMapping::new(symbol.clone());
        mapping.exchange_symbols.insert(ExchangeId::ByBit, "BTCUSDT".to_string());
        
        let mut test_normalizer = Normalizer::new();
        test_normalizer.load_symbol_mappings(vec![mapping]);
        
        let result = test_normalizer.normalize_order_book(
            ExchangeId::ByBit,
            "BTCUSDT",
            bid_levels,
            ask_levels,
            None,
            None
        );
        
        if let Ok(order_book) = result {
            // Property: normalized order book should be valid
            prop_assert!(order_book.is_valid());
            
            // Property: bids should be sorted descending
            for window in order_book.bids.windows(2) {
                prop_assert!(window[0].price >= window[1].price);
            }
            
            // Property: asks should be sorted ascending
            for window in order_book.asks.windows(2) {
                prop_assert!(window[0].price <= window[1].price);
            }
            
            // Property: best bid should be less than best ask
            if let (Some(best_bid), Some(best_ask)) = (order_book.best_bid(), order_book.best_ask()) {
                prop_assert!(best_bid.price < best_ask.price);
            }
        }
    }
}

// Property 9: Timestamp UTC Normalization
// All timestamps should be normalized to UTC milliseconds
proptest! {
    #[test]
    fn prop_timestamp_utc_normalization(
        timestamp_ms in 1000000000000i64..2000000000000i64 // Valid timestamp range
    ) {
        let normalizer = Normalizer::new();
        let normalized = normalizer.normalize_timestamp(timestamp_ms);
        
        // Property: normalized timestamp should be valid UTC
        prop_assert!(normalized.timestamp_millis() > 0);
        
        // Property: should be close to input (within reasonable bounds)
        let diff = (normalized.timestamp_millis() - timestamp_ms).abs();
        prop_assert!(diff < 1000); // Within 1 second tolerance for processing time
    }
}

// Property 10: Invalid Order Book Rejection
// Order books with bid >= ask should be rejected
proptest! {
    #[test]
    fn prop_invalid_order_book_rejection(
        bid_price in 50000u32..60000,
        ask_price in 40000u32..50000 // ask < bid (invalid)
    ) {
        let normalizer = Normalizer::new();
        
        // Create a symbol mapping
        let symbol = Symbol::new("BTC", "USDT");
        let mut mapping = SymbolMapping::new(symbol.clone());
        mapping.exchange_symbols.insert(ExchangeId::ByBit, "BTCUSDT".to_string());
        
        let mut test_normalizer = Normalizer::new();
        test_normalizer.load_symbol_mappings(vec![mapping]);
        
        let bids = vec![(Decimal::from(bid_price), Decimal::from(1))];
        let asks = vec![(Decimal::from(ask_price), Decimal::from(1))];
        
        let result = test_normalizer.normalize_order_book(
            ExchangeId::ByBit,
            "BTCUSDT",
            bids,
            asks,
            None,
            None
        );
        
        // Property: invalid order books should be rejected
        prop_assert!(result.is_err());
    }
}

// Property 8: Stablecoin Equivalence
// Stablecoins in the same group should be considered equivalent
proptest! {
    #[test]
    fn prop_stablecoin_equivalence(
        stablecoin1 in prop::sample::select(vec!["USDT", "USDC", "BUSD", "DAI"]),
        stablecoin2 in prop::sample::select(vec!["USDT", "USDC", "BUSD", "DAI"])
    ) {
        let normalizer = Normalizer::new();
        
        // Property: stablecoins should be equivalent to themselves
        prop_assert!(normalizer.are_symbols_equivalent(&stablecoin1, &stablecoin1));
        
        // Property: all USD stablecoins should be equivalent to each other
        prop_assert!(normalizer.are_symbols_equivalent(&stablecoin1, &stablecoin2));
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_property_framework_works() {
        // Simple test to ensure proptest is working
        let normalizer = Normalizer::new();
        assert!(normalizer.are_symbols_equivalent("USDT", "USDT"));
    }
}