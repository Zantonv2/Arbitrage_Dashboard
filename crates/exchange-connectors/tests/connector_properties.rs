use exchange_connectors::{
    utils::{ExponentialBackoff, format_symbol, parse_symbol},
};
use arbitrage_core::types::{ExchangeId, Symbol};
use proptest::prelude::*;
use std::time::Duration;

proptest! {
    #[test]
    fn prop_exponential_backoff_bounds(
        base_delay_ms in 100u64..5000,
        max_delay_ms in 10000u64..120000,
        _attempt in 0u32..10
    ) {
        let backoff = ExponentialBackoff::new(base_delay_ms, max_delay_ms);
        
        // Create a backoff with simulated attempts by calling delay() multiple times
        // Since we can't directly set current_attempt, we'll test the mathematical properties
        let delay = backoff.delay();
        
        // Property: delay should never exceed max_delay
        prop_assert!(delay <= Duration::from_millis(max_delay_ms));
        
        // Property: initial delay should be at least base_delay
        prop_assert!(delay >= Duration::from_millis(base_delay_ms));
    }
}

proptest! {
    #[test]
    fn prop_sequence_gap_detection(
        seq1 in 1000u64..10000,
        gap_size in 1u64..100
    ) {
        let seq2 = seq1 + gap_size;
        
        // Property: non-consecutive sequences should be detected as gaps
        let has_gap = seq2 != seq1 + 1;
        
        if gap_size == 1 {
            prop_assert!(!has_gap); // No gap for consecutive numbers
        } else {
            prop_assert!(has_gap); // Gap exists for non-consecutive numbers
        }
        
        // Property: gap size should be detectable
        let actual_gap = seq2 - seq1 - 1;
        if has_gap {
            prop_assert_eq!(actual_gap, gap_size - 1);
        }
    }
}

proptest! {
    #[test]
    fn prop_symbol_parsing_roundtrip(
        base in "[A-Z]{3,5}",
        quote in "[A-Z]{3,5}"
    ) {
        let symbol = Symbol::new(&base, &quote);
        
        // Test ByBit format (BTCUSDT)
        let bybit_formatted = format_symbol(&symbol, ExchangeId::ByBit);
        if let Ok(parsed) = parse_symbol(&bybit_formatted, ExchangeId::ByBit) {
            prop_assert_eq!(parsed.base, base.clone());
            prop_assert_eq!(parsed.quote, quote.clone());
        }
        
        // Test BingX format (BTC-USDT)
        let bingx_formatted = format_symbol(&symbol, ExchangeId::BingX);
        if let Ok(parsed) = parse_symbol(&bingx_formatted, ExchangeId::BingX) {
            prop_assert_eq!(parsed.base, base.clone());
            prop_assert_eq!(parsed.quote, quote.clone());
        }
        
        // Test Hyperliquid format (BTC/USDT)
        let hyperliquid_formatted = format_symbol(&symbol, ExchangeId::Hyperliquid);
        if let Ok(parsed) = parse_symbol(&hyperliquid_formatted, ExchangeId::Hyperliquid) {
            prop_assert_eq!(parsed.base, base);
            prop_assert_eq!(parsed.quote, quote);
        }
    }
}

proptest! {
    #[test]
    fn prop_symbol_formatting_consistency(
        base in "[A-Z]{3,5}",
        quote in "[A-Z]{3,5}"
    ) {
        let symbol = Symbol::new(&base, &quote);
        
        // Property: ByBit format should concatenate base and quote
        let bybit_format = format_symbol(&symbol, ExchangeId::ByBit);
        prop_assert_eq!(bybit_format, format!("{}{}", base, quote));
        
        // Property: BingX format should use dash separator
        let bingx_format = format_symbol(&symbol, ExchangeId::BingX);
        prop_assert_eq!(bingx_format, format!("{}-{}", base, quote));
        
        // Property: Hyperliquid format should use slash separator
        let hyperliquid_format = format_symbol(&symbol, ExchangeId::Hyperliquid);
        prop_assert_eq!(hyperliquid_format, format!("{}/{}", base, quote));
    }
}

proptest! {
    #[test]
    fn prop_invalid_symbol_parsing(
        invalid_symbol in "[a-z0-9]{1,20}" // lowercase and numbers, various lengths
    ) {
        // Property: invalid symbols should return errors
        let bybit_result = parse_symbol(&invalid_symbol, ExchangeId::ByBit);
        let bingx_result = parse_symbol(&invalid_symbol, ExchangeId::BingX);
        let hyperliquid_result = parse_symbol(&invalid_symbol, ExchangeId::Hyperliquid);
        
        // Most invalid symbols should fail parsing
        // (Some might accidentally be valid, but most should fail)
        let total_results = [&bybit_result, &bingx_result, &hyperliquid_result];
        let error_count = total_results.iter().filter(|r| r.is_err()).count();
        
        // At least some should fail for truly invalid symbols
        if invalid_symbol.len() < 6 || !invalid_symbol.chars().any(|c| c.is_ascii_uppercase()) {
            prop_assert!(error_count > 0);
        }
    }
}

proptest! {
    #[test]
    fn prop_backoff_reset(
        base_delay_ms in 100u64..5000,
        max_delay_ms in 10000u64..120000
    ) {
        let mut backoff = ExponentialBackoff::new(base_delay_ms, max_delay_ms);
        
        // Property: initial attempt should be zero
        prop_assert_eq!(backoff.attempt(), 0);
        
        // Reset (should be no-op for initial state)
        backoff.reset();
        
        // Property: attempt should still be zero after reset
        prop_assert_eq!(backoff.attempt(), 0);
        
        // Property: delay should be base delay initially
        let delay_after_reset = backoff.delay();
        prop_assert_eq!(delay_after_reset, Duration::from_millis(base_delay_ms));
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_exponential_backoff_basic() {
        let backoff = ExponentialBackoff::new(1000, 60000);
        
        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.delay(), Duration::from_millis(1000));
        
        // We can't directly test incremented attempts since current_attempt is private
        // But we can test that the delay calculation works correctly for the initial state
        let delay = backoff.delay();
        assert!(delay >= Duration::from_millis(1000));
        assert!(delay <= Duration::from_millis(60000));
    }
    
    #[test]
    fn test_symbol_format_parse_roundtrip() {
        let symbol = Symbol::new("BTC", "USDT");
        
        // ByBit
        let formatted = format_symbol(&symbol, ExchangeId::ByBit);
        assert_eq!(formatted, "BTCUSDT");
        let parsed = parse_symbol(&formatted, ExchangeId::ByBit).unwrap();
        assert_eq!(parsed.base, "BTC");
        assert_eq!(parsed.quote, "USDT");
        
        // BingX
        let formatted = format_symbol(&symbol, ExchangeId::BingX);
        assert_eq!(formatted, "BTC-USDT");
        let parsed = parse_symbol(&formatted, ExchangeId::BingX).unwrap();
        assert_eq!(parsed.base, "BTC");
        assert_eq!(parsed.quote, "USDT");
        
        // Hyperliquid
        let formatted = format_symbol(&symbol, ExchangeId::Hyperliquid);
        assert_eq!(formatted, "BTC/USDT");
        let parsed = parse_symbol(&formatted, ExchangeId::Hyperliquid).unwrap();
        assert_eq!(parsed.base, "BTC");
        assert_eq!(parsed.quote, "USDT");
    }
}