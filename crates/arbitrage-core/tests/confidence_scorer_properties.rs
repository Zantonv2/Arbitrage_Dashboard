use arbitrage_core::{
    confidence_scorer::{ConfidenceScorer, ConfidenceConfig, FeeSchedule, ExchangeReliability},
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol, VwapResult},
};
use chrono::{DateTime, Utc, Duration};
use proptest::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

// Helper function to create test VWAP result
fn create_test_vwap(
    filled_quantity: Decimal,
    vwap_price: Decimal,
    slippage_bps: i32,
    is_fully_filled: bool,
) -> VwapResult {
    VwapResult {
        filled_quantity,
        vwap_price,
        slippage_bps,
        is_fully_filled,
        total_cost: filled_quantity * vwap_price,
    }
}

// Helper function to create test order book
fn create_test_order_book(
    exchange: ExchangeId,
    symbol: Symbol,
    timestamp: DateTime<Utc>,
    is_valid: bool,
) -> OrderBook {
    if is_valid {
        let bids = vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))];
        let asks = vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))];
        let mut book = OrderBook::new(exchange, symbol, bids, asks);
        book.timestamp = timestamp;
        book
    } else {
        // Invalid book: bid > ask
        let bids = vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))];
        let asks = vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))];
        let mut book = OrderBook::new(exchange, symbol, bids, asks);
        book.timestamp = timestamp;
        book
    }
}

// Property 16: Net Spread Calculation Accuracy
// Net spread should always be less than or equal to gross spread due to fees
proptest! {
    #[test]
    fn prop_net_spread_less_than_gross(
        buy_price in 1000u32..100000,
        sell_price in 1001u32..100001, // Ensure sell > buy
        buy_fee_bps in 1u32..200,      // 0.01% to 2%
        sell_fee_bps in 1u32..200
    ) {
        let buy_decimal = Decimal::from(buy_price);
        let sell_decimal = Decimal::from(sell_price);
        
        // Ensure sell > buy for valid arbitrage
        prop_assume!(sell_decimal > buy_decimal);
        
        let mut scorer = ConfidenceScorer::default();
        
        // Set up custom fee schedules
        let buy_fee = FeeSchedule::new(ExchangeId::ByBit, Decimal::ZERO, Decimal::from(buy_fee_bps) / Decimal::from(10000));
        let sell_fee = FeeSchedule::new(ExchangeId::OKX, Decimal::ZERO, Decimal::from(sell_fee_bps) / Decimal::from(10000));
        
        scorer.update_fee_schedule(ExchangeId::ByBit, buy_fee);
        scorer.update_fee_schedule(ExchangeId::OKX, sell_fee);
        
        // Calculate gross spread (without fees)
        let gross_spread_bps = ((sell_decimal - buy_decimal) / buy_decimal * Decimal::from(10000))
            .to_i32().unwrap_or(0);
        
        // Calculate net spread (with fees)
        let net_spread_result = scorer.calculate_net_spread_bps(
            buy_decimal,
            sell_decimal,
            ExchangeId::ByBit,
            ExchangeId::OKX
        );
        
        prop_assert!(net_spread_result.is_ok());
        
        if let Ok(net_spread_bps) = net_spread_result {
            // Property: net spread should be less than or equal to gross spread
            prop_assert!(net_spread_bps <= gross_spread_bps);
            
            // Property: if fees are positive, net should be strictly less than gross
            if buy_fee_bps > 0 || sell_fee_bps > 0 {
                prop_assert!(net_spread_bps < gross_spread_bps);
            }
        }
    }
}

// Property 17: Confidence Score Normalization
// Confidence scores should always be between 0 and 100
proptest! {
    #[test]
    fn prop_confidence_score_bounds(
        depth_qty in 1u32..1000,
        slippage_buy in 0i32..1000,
        slippage_sell in 0i32..1000,
        is_book_valid in any::<bool>(),
        is_fully_filled in any::<bool>()
    ) {
        let scorer = ConfidenceScorer::default();
        let symbol = Symbol::new("BTC", "USDT");
        let now = Utc::now();
        
        // Create test VWAP results
        let buy_vwap = create_test_vwap(
            Decimal::from(depth_qty),
            Decimal::from(50000),
            slippage_buy,
            is_fully_filled
        );
        
        let sell_vwap = create_test_vwap(
            Decimal::from(depth_qty),
            Decimal::from(50100),
            slippage_sell,
            is_fully_filled
        );
        
        // Create test order books
        let buy_book = create_test_order_book(ExchangeId::ByBit, symbol.clone(), now, is_book_valid);
        let sell_book = create_test_order_book(ExchangeId::OKX, symbol, now, is_book_valid);
        
        let factors = scorer.calculate_confidence_factors(&buy_vwap, &sell_vwap, &buy_book, &sell_book);
        let confidence = scorer.calculate_confidence(&factors);
        
        // Property: confidence score should be between 0 and 100
        prop_assert!(confidence >= Decimal::ZERO);
        prop_assert!(confidence <= Decimal::from(100));
        
        // Property: individual factors should also be bounded
        prop_assert!(factors.depth_score >= Decimal::ZERO);
        prop_assert!(factors.volatility_score >= Decimal::ZERO);
        prop_assert!(factors.reliability_score >= Decimal::ZERO);
        prop_assert!(factors.spread_stability_score >= Decimal::ZERO);
        prop_assert!(factors.freshness_score >= Decimal::ZERO);
    }
}

// Property 18: Freshness Impact on Confidence
// Stale data should result in lower confidence scores
proptest! {
    #[test]
    fn prop_freshness_impact_on_confidence(
        age_seconds in 0u64..3600 // 0 to 1 hour
    ) {
        let mut config = ConfidenceConfig::default();
        config.max_latency_ms = 1000; // 1 second max latency
        
        let scorer = ConfidenceScorer::new(config);
        let symbol = Symbol::new("BTC", "USDT");
        
        let fresh_time = Utc::now();
        let stale_time = fresh_time - Duration::seconds(age_seconds as i64);
        
        // Create identical VWAP results
        let vwap = create_test_vwap(Decimal::from(100), Decimal::from(50000), 10, true);
        
        // Create fresh and stale order books
        let fresh_buy_book = create_test_order_book(ExchangeId::ByBit, symbol.clone(), fresh_time, true);
        let fresh_sell_book = create_test_order_book(ExchangeId::OKX, symbol.clone(), fresh_time, true);
        
        let stale_buy_book = create_test_order_book(ExchangeId::ByBit, symbol.clone(), stale_time, true);
        let stale_sell_book = create_test_order_book(ExchangeId::OKX, symbol, stale_time, true);
        
        let fresh_factors = scorer.calculate_confidence_factors(&vwap, &vwap, &fresh_buy_book, &fresh_sell_book);
        let stale_factors = scorer.calculate_confidence_factors(&vwap, &vwap, &stale_buy_book, &stale_sell_book);
        
        let fresh_confidence = scorer.calculate_confidence(&fresh_factors);
        let stale_confidence = scorer.calculate_confidence(&stale_factors);
        
        // Property: fresh data should have higher or equal confidence
        if age_seconds > 1 { // If data is older than max latency
            prop_assert!(fresh_confidence >= stale_confidence);
            
            // Property: stale data should have lower freshness score
            prop_assert!(fresh_factors.freshness_score >= stale_factors.freshness_score);
        }
    }
}

// Property 19: Fee Schedule Impact
// Higher fees should result in lower net spreads
proptest! {
    #[test]
    fn prop_fee_schedule_impact(
        price in 10000u32..100000,
        low_fee_bps in 1u32..50,   // 0.01% to 0.5%
        high_fee_bps in 51u32..200 // 0.51% to 2%
    ) {
        let base_price = Decimal::from(price);
        let sell_price = base_price + Decimal::from(100); // $100 spread
        
        let mut scorer = ConfidenceScorer::default();
        
        // Test with low fees
        let low_fee = FeeSchedule::new(ExchangeId::ByBit, Decimal::ZERO, Decimal::from(low_fee_bps) / Decimal::from(10000));
        scorer.update_fee_schedule(ExchangeId::ByBit, low_fee.clone());
        scorer.update_fee_schedule(ExchangeId::OKX, low_fee);
        
        let low_fee_spread = scorer.calculate_net_spread_bps(
            base_price,
            sell_price,
            ExchangeId::ByBit,
            ExchangeId::OKX
        ).unwrap_or(0);
        
        // Test with high fees
        let high_fee = FeeSchedule::new(ExchangeId::ByBit, Decimal::ZERO, Decimal::from(high_fee_bps) / Decimal::from(10000));
        scorer.update_fee_schedule(ExchangeId::ByBit, high_fee.clone());
        scorer.update_fee_schedule(ExchangeId::OKX, high_fee);
        
        let high_fee_spread = scorer.calculate_net_spread_bps(
            base_price,
            sell_price,
            ExchangeId::ByBit,
            ExchangeId::OKX
        ).unwrap_or(0);
        
        // Property: higher fees should result in lower net spreads
        prop_assert!(low_fee_spread >= high_fee_spread);
        
        // Property: both should be non-negative or -1 (unprofitable)
        prop_assert!(low_fee_spread >= -1);
        prop_assert!(high_fee_spread >= -1);
    }
}

// Property 20: Exchange Reliability Impact
// Better exchange reliability should improve confidence scores
proptest! {
    #[test]
    fn prop_exchange_reliability_impact(
        uptime_percent in 50u32..100,
        error_rate_bps in 0u32..1000 // 0% to 10% error rate
    ) {
        let mut scorer = ConfidenceScorer::default();
        let symbol = Symbol::new("BTC", "USDT");
        let now = Utc::now();
        
        // Set up exchange reliability
        let reliability = ExchangeReliability {
            uptime_percent: Decimal::from(uptime_percent),
            error_rate: Decimal::from(error_rate_bps) / Decimal::from(10000),
            avg_latency_ms: 100,
            last_updated: now,
        };
        
        scorer.update_exchange_reliability(ExchangeId::ByBit, reliability.clone());
        scorer.update_exchange_reliability(ExchangeId::OKX, reliability);
        
        // Create test data
        let vwap = create_test_vwap(Decimal::from(100), Decimal::from(50000), 10, true);
        let buy_book = create_test_order_book(ExchangeId::ByBit, symbol.clone(), now, true);
        let sell_book = create_test_order_book(ExchangeId::OKX, symbol, now, true);
        
        let factors = scorer.calculate_confidence_factors(&vwap, &vwap, &buy_book, &sell_book);
        
        // Property: reliability score should reflect exchange quality
        if uptime_percent >= 95 && error_rate_bps <= 100 { // High quality exchanges
            prop_assert!(factors.reliability_score >= Decimal::from(80));
        }
        
        // Property: reliability score should be bounded
        prop_assert!(factors.reliability_score >= Decimal::ZERO);
        prop_assert!(factors.reliability_score <= Decimal::from(100));
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_confidence_scorer_creation() {
        let scorer = ConfidenceScorer::default();
        
        // Should have default fee schedules
        let okx_fee = scorer.get_fee_rate(ExchangeId::OKX, false);
        assert!(okx_fee > Decimal::ZERO);
        assert!(okx_fee < Decimal::new(5, 3)); // Less than 0.5%
    }
    
    #[test]
    fn test_net_spread_calculation() {
        let scorer = ConfidenceScorer::default();
        
        let result = scorer.calculate_net_spread_bps(
            Decimal::from(50000), // Buy at $50k
            Decimal::from(50500), // Sell at $50.5k
            ExchangeId::ByBit,
            ExchangeId::OKX
        );
        
        assert!(result.is_ok());
        let spread_bps = result.unwrap();
        
        // Should be positive but less than gross spread (1000 bps = 1%)
        assert!(spread_bps > 0);
        assert!(spread_bps < 1000); // Less than 1% due to fees
    }
    
    #[test]
    fn test_unprofitable_spread() {
        let scorer = ConfidenceScorer::default();
        
        // Very small spread that becomes unprofitable after fees
        let result = scorer.calculate_net_spread_bps(
            Decimal::from(50000), // Buy at $50k
            Decimal::from(50010), // Sell at $50.01k (only 0.02% gross)
            ExchangeId::ByBit,
            ExchangeId::OKX
        );
        
        assert!(result.is_ok());
        let spread_bps = result.unwrap();
        
        // Should be -1 indicating unprofitable
        assert_eq!(spread_bps, -1);
    }
    
    #[test]
    fn test_data_freshness_check() {
        let scorer = ConfidenceScorer::default();
        
        let fresh_time = Utc::now();
        let stale_time = fresh_time - Duration::seconds(10); // 10 seconds old
        
        assert!(scorer.is_data_fresh(fresh_time));
        assert!(!scorer.is_data_fresh(stale_time)); // Should be stale (> 500ms)
    }
}