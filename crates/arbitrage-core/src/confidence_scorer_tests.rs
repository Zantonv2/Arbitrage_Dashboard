#[cfg(test)]
mod tests {
    use crate::{
        confidence_scorer::{
            ConfidenceConfig, ConfidenceFactors, ConfidenceScorer, ExchangeReliability, FeeSchedule,
        },
        types::{ExchangeId, OrderBook, OrderBookLevel, Signal, Symbol, VwapResult},
    };
    use chrono::{Duration, Utc};
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

    fn create_test_order_books() -> (OrderBook, OrderBook) {
        let symbol = Symbol::new("BTC", "USDT");
        let buy_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![
                OrderBookLevel::new(Decimal::from(49990), Decimal::from(2)),
                OrderBookLevel::new(Decimal::from(49980), Decimal::from(3)),
            ],
            vec![
                OrderBookLevel::new(Decimal::from(50010), Decimal::from(1)),
                OrderBookLevel::new(Decimal::from(50020), Decimal::from(2)),
            ],
        );
        let sell_book = OrderBook::new(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![
                OrderBookLevel::new(Decimal::from(50100), Decimal::from(2)),
                OrderBookLevel::new(Decimal::from(50110), Decimal::from(3)),
            ],
            vec![
                OrderBookLevel::new(Decimal::from(50120), Decimal::from(1)),
                OrderBookLevel::new(Decimal::from(50130), Decimal::from(2)),
            ],
        );
        (buy_book, sell_book)
    }

    fn create_test_vwap_result() -> VwapResult {
        VwapResult {
            vwap_price: Decimal::from(50050),
            filled_quantity: Decimal::from(1),
            total_cost: Decimal::from(50050),
            is_fully_filled: true,
            slippage_bps: 10,
        }
    }

    // === Happy Path Tests ===

    #[test]
    fn test_confidence_scorer_new() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let threshold = scorer.get_min_confidence_threshold();
        assert!(threshold > Decimal::ZERO);
    }

    #[test]
    fn test_confidence_config_default() {
        let config = ConfidenceConfig::default();
        assert_eq!(config.depth_weight, Decimal::new(3, 1));
        assert_eq!(config.volatility_weight, Decimal::new(2, 1));
        assert_eq!(config.reliability_weight, Decimal::new(2, 1));
        assert_eq!(config.min_confidence_threshold, Decimal::new(3, 1));
    }

    #[test]
    fn test_fee_schedule_new() {
        let exchange = ExchangeId::OKX;
        let maker_fee = Decimal::new(8, 4);
        let taker_fee = Decimal::new(1, 3);
        let schedule = FeeSchedule::new(exchange, maker_fee, taker_fee);
        assert_eq!(schedule.exchange, ExchangeId::OKX);
        assert_eq!(schedule.maker_fee, Decimal::new(8, 4));
        assert_eq!(schedule.taker_fee, Decimal::new(1, 3));
    }

    #[test]
    fn test_fee_schedule_get_fee_rate_maker() {
        let schedule = FeeSchedule::new(ExchangeId::OKX, Decimal::new(8, 4), Decimal::new(1, 3));
        assert_eq!(schedule.get_fee_rate(true), Decimal::new(8, 4));
    }

    #[test]
    fn test_fee_schedule_get_fee_rate_taker() {
        let schedule = FeeSchedule::new(ExchangeId::OKX, Decimal::new(8, 4), Decimal::new(1, 3));
        assert_eq!(schedule.get_fee_rate(false), Decimal::new(1, 3));
    }

    #[test]
    fn test_calculate_net_spread_bps_positive() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(50200);
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        assert!(result.is_profitable());
    }

    #[test]
    fn test_calculate_net_spread_bps_with_fees() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        // Larger spread that should be profitable after fees
        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(50300);
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        assert!(result.is_profitable());
    }

    #[test]
    fn test_is_data_fresh_recent() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let recent_time = Utc::now();
        assert!(scorer.is_data_fresh(recent_time));
    }

    #[test]
    fn test_is_data_fresh_old() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let old_time = Utc::now() - Duration::milliseconds(1000);
        assert!(!scorer.is_data_fresh(old_time));
    }

    #[test]
    fn test_calculate_confidence_factors() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let (buy_book, sell_book) = create_test_order_books();
        let buy_vwap = create_test_vwap_result();
        let sell_vwap = create_test_vwap_result();

        let factors =
            scorer.calculate_confidence_factors(&buy_vwap, &sell_vwap, &buy_book, &sell_book);
        assert!(factors.depth_score >= Decimal::ZERO);
        assert!(factors.volatility_score >= Decimal::ZERO);
        assert!(factors.reliability_score >= Decimal::ZERO);
        assert!(factors.spread_stability_score >= Decimal::ZERO);
        assert!(factors.freshness_score >= Decimal::ZERO);
    }

    #[test]
    fn test_calculate_confidence_basic() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(80),
            volatility_score: Decimal::from(90),
            reliability_score: Decimal::from(100),
            spread_stability_score: Decimal::from(85),
            freshness_score: Decimal::from(100),
        };
        let confidence = scorer.calculate_confidence(&factors);
        assert!(confidence >= Decimal::ZERO);
        assert!(confidence <= Decimal::from(100));
    }

    #[test]
    fn test_update_exchange_reliability() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);
        let reliability = ExchangeReliability {
            uptime_percent: Decimal::from(99),
            error_rate: Decimal::new(1, 3),
            avg_latency_ms: 100,
            last_updated: Utc::now(),
        };
        scorer.update_exchange_reliability(ExchangeId::OKX, reliability.clone());
        let retrieved = scorer.get_exchange_reliability(ExchangeId::OKX);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().uptime_percent, Decimal::from(99));
    }

    #[test]
    fn test_get_exchange_reliability_none() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let result = scorer.get_exchange_reliability(ExchangeId::MEXC);
        assert!(result.is_none());
    }

    #[test]
    fn test_update_fee_schedule() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);
        let schedule = FeeSchedule::new(ExchangeId::MEXC, Decimal::new(2, 3), Decimal::new(2, 3));
        scorer.update_fee_schedule(ExchangeId::MEXC, schedule.clone());
        let fee_rate = scorer.get_fee_rate(ExchangeId::MEXC, false);
        assert_eq!(fee_rate, Decimal::new(2, 3));
    }

    #[test]
    fn test_get_fee_rate_default() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        // Unknown exchange should return default fee of 0.001
        let fee_rate = scorer.get_fee_rate(ExchangeId::Bitstamp, false);
        assert_eq!(fee_rate, Decimal::new(1, 3));
    }

    #[test]
    fn test_confidence_scorer_default() {
        let scorer = ConfidenceScorer::default();
        let threshold = scorer.get_min_confidence_threshold();
        assert!(threshold > Decimal::ZERO);
    }

    // === Edge Case Tests ===

    #[test]
    fn test_calculate_net_spread_bps_zero_buy_price() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let result = scorer.calculate_net_spread_bps(
            Decimal::ZERO,
            Decimal::from(50200),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        assert!(result.is_unprofitable());
    }

    #[test]
    fn test_calculate_net_spread_bps_unprofitable() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        // Very small spread that becomes negative after fees
        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(50050);
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        assert!(result.is_unprofitable());
    }

    #[test]
    fn test_calculate_confidence_zero_weights() {
        let config = ConfidenceConfig {
            depth_weight: Decimal::ZERO,
            volatility_weight: Decimal::ZERO,
            reliability_weight: Decimal::ZERO,
            spread_stability_weight: Decimal::ZERO,
            freshness_weight: Decimal::ZERO,
            ..ConfidenceConfig::default()
        };
        let scorer = ConfidenceScorer::new(config);
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(100),
            volatility_score: Decimal::from(100),
            reliability_score: Decimal::from(100),
            spread_stability_score: Decimal::from(100),
            freshness_score: Decimal::from(100),
        };
        let confidence = scorer.calculate_confidence(&factors);
        assert_eq!(confidence, Decimal::ZERO);
    }

    #[test]
    fn test_calculate_confidence_negative_factors() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(-10),
            volatility_score: Decimal::from(-10),
            reliability_score: Decimal::from(-10),
            spread_stability_score: Decimal::from(-10),
            freshness_score: Decimal::from(-10),
        };
        let confidence = scorer.calculate_confidence(&factors);
        assert_eq!(confidence, Decimal::ZERO);
    }

    #[test]
    fn test_calculate_confidence_very_high() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(200),
            volatility_score: Decimal::from(200),
            reliability_score: Decimal::from(200),
            spread_stability_score: Decimal::from(200),
            freshness_score: Decimal::from(200),
        };
        let confidence = scorer.calculate_confidence(&factors);
        assert!(confidence <= Decimal::from(100));
    }

    #[test]
    fn test_is_data_fresh_boundary() {
        let config = ConfidenceConfig {
            max_latency_ms: 500,
            ..ConfidenceConfig::default()
        };
        let scorer = ConfidenceScorer::new(config);
        let boundary_time = Utc::now() - Duration::milliseconds(500);
        assert!(scorer.is_data_fresh(boundary_time));
    }

    #[test]
    fn test_is_data_fresh_exactly_over() {
        let config = ConfidenceConfig {
            max_latency_ms: 500,
            ..ConfidenceConfig::default()
        };
        let scorer = ConfidenceScorer::new(config);
        let over_time = Utc::now() - Duration::milliseconds(501);
        assert!(!scorer.is_data_fresh(over_time));
    }

    #[test]
    fn test_confidence_factors_with_empty_books() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let symbol = Symbol::new("BTC", "USDT");
        let empty_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        let sell_book = OrderBook::new(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(1))],
        );
        let buy_vwap = VwapResult {
            vwap_price: Decimal::from(50000),
            filled_quantity: Decimal::ZERO,
            total_cost: Decimal::ZERO,
            is_fully_filled: false,
            slippage_bps: 50,
        };
        let sell_vwap = create_test_vwap_result();

        let factors =
            scorer.calculate_confidence_factors(&buy_vwap, &sell_vwap, &empty_book, &sell_book);
        assert_eq!(factors.depth_score, Decimal::ZERO);
    }

    // === Error Path Tests ===

    #[test]
    fn test_calculate_net_spread_bps_negative_prices() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let result = scorer.calculate_net_spread_bps(
            Decimal::from(-100),
            Decimal::from(50000),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        assert!(result.is_profitable() || result.is_unprofitable());
    }

    #[test]
    fn test_confidence_factors_invalid_book() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let symbol = Symbol::new("BTC", "USDT");
        // Invalid book: bids >= asks
        let invalid_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        );
        let sell_book = OrderBook::new(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(1))],
        );
        let buy_vwap = create_test_vwap_result();
        let sell_vwap = create_test_vwap_result();

        let factors =
            scorer.calculate_confidence_factors(&buy_vwap, &sell_vwap, &invalid_book, &sell_book);
        // Should still produce a result
        assert!(factors.reliability_score < Decimal::from(100));
    }

    // === Boundary Condition Tests ===

    #[test]
    fn test_calculate_confidence_boundary_min() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let factors = ConfidenceFactors {
            depth_score: Decimal::ZERO,
            volatility_score: Decimal::ZERO,
            reliability_score: Decimal::ZERO,
            spread_stability_score: Decimal::ZERO,
            freshness_score: Decimal::ZERO,
        };
        let confidence = scorer.calculate_confidence(&factors);
        assert_eq!(confidence, Decimal::ZERO);
    }

    #[test]
    fn test_calculate_confidence_boundary_max() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(100),
            volatility_score: Decimal::from(100),
            reliability_score: Decimal::from(100),
            spread_stability_score: Decimal::from(100),
            freshness_score: Decimal::from(100),
        };
        let confidence = scorer.calculate_confidence(&factors);
        assert!(confidence <= Decimal::from(100));
    }

    #[test]
    fn test_exchange_reliability_boundary_values() {
        let reliability = ExchangeReliability {
            uptime_percent: Decimal::from(100),
            error_rate: Decimal::ZERO,
            avg_latency_ms: 0,
            last_updated: Utc::now(),
        };
        assert_eq!(reliability.uptime_percent, Decimal::from(100));
        assert_eq!(reliability.error_rate, Decimal::ZERO);
    }

    #[test]
    fn test_fee_schedule_extreme_values() {
        let schedule = FeeSchedule::new(ExchangeId::OKX, Decimal::from(1), Decimal::from(1));
        assert_eq!(schedule.get_fee_rate(true), Decimal::from(1));
        assert_eq!(schedule.get_fee_rate(false), Decimal::from(1));
    }

    #[test]
    fn test_confidence_config_extreme_weights() {
        let config = ConfidenceConfig {
            depth_weight: Decimal::from(100),
            volatility_weight: Decimal::from(100),
            reliability_weight: Decimal::from(100),
            spread_stability_weight: Decimal::from(100),
            freshness_weight: Decimal::from(100),
            ..ConfidenceConfig::default()
        };
        let scorer = ConfidenceScorer::new(config);
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(100),
            volatility_score: Decimal::from(100),
            reliability_score: Decimal::from(100),
            spread_stability_score: Decimal::from(100),
            freshness_score: Decimal::from(100),
        };
        let confidence = scorer.calculate_confidence(&factors);
        assert!(confidence > Decimal::ZERO);
    }

    // === Type Conversion Tests ===

    #[test]
    fn test_confidence_config_clone() {
        let config = ConfidenceConfig::default();
        let cloned = config.clone();
        assert_eq!(config.depth_weight, cloned.depth_weight);
    }

    #[test]
    fn test_confidence_factors_clone() {
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(50),
            volatility_score: Decimal::from(60),
            reliability_score: Decimal::from(70),
            spread_stability_score: Decimal::from(80),
            freshness_score: Decimal::from(90),
        };
        let cloned = factors.clone();
        assert_eq!(factors.depth_score, cloned.depth_score);
    }

    #[test]
    fn test_exchange_reliability_clone() {
        let reliability = ExchangeReliability {
            uptime_percent: Decimal::from(99),
            error_rate: Decimal::new(1, 4),
            avg_latency_ms: 150,
            last_updated: Utc::now(),
        };
        let cloned = reliability.clone();
        assert_eq!(reliability.uptime_percent, cloned.uptime_percent);
    }

    #[test]
    fn test_fee_schedule_clone() {
        let schedule = FeeSchedule::new(ExchangeId::OKX, Decimal::new(8, 4), Decimal::new(1, 3));
        let cloned = schedule.clone();
        assert_eq!(schedule.maker_fee, cloned.maker_fee);
    }

    #[test]
    fn test_confidence_config_debug_format() {
        let config = ConfidenceConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("depth_weight"));
        assert!(debug_str.contains("volatility_weight"));
    }

    #[test]
    fn test_confidence_factors_debug_format() {
        let factors = ConfidenceFactors {
            depth_score: Decimal::from(50),
            volatility_score: Decimal::from(60),
            reliability_score: Decimal::from(70),
            spread_stability_score: Decimal::from(80),
            freshness_score: Decimal::from(90),
        };
        let debug_str = format!("{:?}", factors);
        assert!(debug_str.contains("depth_score"));
    }

    // === Additional Tests ===

    #[test]
    fn test_calculate_net_spread_bps_large_spread() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(55000);
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        let spread = result.profit_value().unwrap();
        assert!(spread > 0);
    }

    #[test]
    fn test_calculate_net_spread_bps_different_exchanges() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);

        let result1 = scorer.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50200),
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );

        let result2 = scorer.calculate_net_spread_bps(
            Decimal::from(50000),
            Decimal::from(50500),
            ExchangeId::MEXC,
            ExchangeId::GateIo,
        );

        assert!(result1.is_profitable());
        assert!(result2.is_profitable());
    }

    #[test]
    fn test_confidence_factors_vwap_comparison() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let (buy_book, sell_book) = create_test_order_books();

        let fully_filled = VwapResult {
            vwap_price: Decimal::from(50050),
            filled_quantity: Decimal::from(10),
            total_cost: Decimal::from(500500),
            is_fully_filled: true,
            slippage_bps: 5,
        };

        let partially_filled = VwapResult {
            vwap_price: Decimal::from(50050),
            filled_quantity: Decimal::from(1),
            total_cost: Decimal::from(50050),
            is_fully_filled: false,
            slippage_bps: 50,
        };

        let factors1 = scorer.calculate_confidence_factors(
            &fully_filled,
            &fully_filled,
            &buy_book,
            &sell_book,
        );

        let factors2 = scorer.calculate_confidence_factors(
            &partially_filled,
            &partially_filled,
            &buy_book,
            &sell_book,
        );

        // Both should produce valid results
        assert!(factors1.depth_score >= Decimal::ZERO);
        assert!(factors2.depth_score >= Decimal::ZERO);
    }

    #[test]
    fn test_confidence_factors_slippage_impact() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);
        let (buy_book, sell_book) = create_test_order_books();

        let low_slippage = VwapResult {
            vwap_price: Decimal::from(50050),
            filled_quantity: Decimal::from(1),
            total_cost: Decimal::from(50050),
            is_fully_filled: true,
            slippage_bps: 5,
        };

        let high_slippage = VwapResult {
            vwap_price: Decimal::from(50050),
            filled_quantity: Decimal::from(1),
            total_cost: Decimal::from(50050),
            is_fully_filled: true,
            slippage_bps: 100,
        };

        let factors1 = scorer.calculate_confidence_factors(
            &low_slippage,
            &low_slippage,
            &buy_book,
            &sell_book,
        );

        let factors2 = scorer.calculate_confidence_factors(
            &high_slippage,
            &high_slippage,
            &buy_book,
            &sell_book,
        );

        assert!(factors1.volatility_score > factors2.volatility_score);
        assert!(factors1.spread_stability_score > factors2.spread_stability_score);
    }

    #[test]
    fn test_confidence_factors_freshness() {
        let config = ConfidenceConfig {
            max_latency_ms: 500,
            ..ConfidenceConfig::default()
        };
        let scorer = ConfidenceScorer::new(config);
        let symbol = Symbol::new("BTC", "USDT");
        let buy_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        let sell_book = OrderBook::new(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(1))],
        );

        let fresh_book = OrderBook {
            timestamp: Utc::now(),
            ..buy_book.clone()
        };

        let stale_book = OrderBook {
            timestamp: Utc::now() - Duration::milliseconds(1000),
            ..buy_book.clone()
        };

        let vwap = create_test_vwap_result();

        let factors_fresh =
            scorer.calculate_confidence_factors(&vwap, &vwap, &fresh_book, &sell_book);

        let factors_stale =
            scorer.calculate_confidence_factors(&vwap, &vwap, &stale_book, &sell_book);

        assert!(factors_fresh.freshness_score > factors_stale.freshness_score);
    }

    #[test]
    fn test_confidence_factors_reliability() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        let high_reliability = ExchangeReliability {
            uptime_percent: Decimal::from(99),
            error_rate: Decimal::new(1, 4),
            avg_latency_ms: 50,
            last_updated: Utc::now(),
        };

        let low_reliability = ExchangeReliability {
            uptime_percent: Decimal::from(90),
            error_rate: Decimal::new(5, 3),
            avg_latency_ms: 500,
            last_updated: Utc::now(),
        };

        scorer.update_exchange_reliability(ExchangeId::OKX, high_reliability);
        scorer.update_exchange_reliability(ExchangeId::ByBit, low_reliability);

        let (buy_book, sell_book) = create_test_order_books();
        let vwap = create_test_vwap_result();

        let factors = scorer.calculate_confidence_factors(&vwap, &vwap, &buy_book, &sell_book);

        // Reliability score should be a valid score
        assert!(factors.reliability_score >= Decimal::ZERO);
        assert!(factors.reliability_score <= Decimal::from(100));
    }

    #[test]
    fn test_all_default_fee_schedules_populated() {
        let config = ConfidenceConfig::default();
        let scorer = ConfidenceScorer::new(config);

        // Check that default exchanges have fee schedules
        assert_eq!(
            scorer.get_fee_rate(ExchangeId::OKX, false),
            Decimal::new(1, 3)
        );
        assert_eq!(
            scorer.get_fee_rate(ExchangeId::ByBit, false),
            Decimal::new(1, 3)
        );
        assert_eq!(
            scorer.get_fee_rate(ExchangeId::MEXC, false),
            Decimal::new(2, 3)
        );
        assert_eq!(
            scorer.get_fee_rate(ExchangeId::GateIo, false),
            Decimal::new(2, 3)
        );
    }

    // === Fee Overflow Protection Tests ===

    #[test]
    fn test_calculate_net_spread_bps_high_fee_schedule() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        // Set extremely high fees (50% each = 100% total, should make it unprofitable)
        scorer.update_fee_schedule(
            ExchangeId::MEXC,
            FeeSchedule::new(ExchangeId::MEXC, Decimal::from(5), Decimal::from(5)),
        );
        scorer.update_fee_schedule(
            ExchangeId::GateIo,
            FeeSchedule::new(ExchangeId::GateIo, Decimal::from(5), Decimal::from(5)),
        );

        // Large spread but 100% fees should still make it unprofitable
        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(60000); // 20% spread
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
        );
        // With 100% fees, effective buy = 2*buy_price, effective sell = 0, so unprofitable
        assert!(result.is_unprofitable());
    }

    #[test]
    fn test_calculate_net_spread_bps_very_high_spread() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        // Very high spread that should be profitable even with high fees
        let buy_price = Decimal::from(100);
        let sell_price = Decimal::from(500); // 400% spread
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
        );
        // Even with 0.2% + 0.2% = 0.4% fees, this should still be massively profitable
        assert!(result.is_profitable());
        if let Some(bps) = result.profit_value() {
            assert!(bps > 0);
        }
    }

    #[test]
    fn test_calculate_net_spread_bps_extreme_fees() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        // Set fees so high that effective prices become invalid
        scorer.update_fee_schedule(
            ExchangeId::OKX,
            FeeSchedule::new(ExchangeId::OKX, Decimal::from(1), Decimal::from(1)),
        );
        scorer.update_fee_schedule(
            ExchangeId::ByBit,
            FeeSchedule::new(ExchangeId::ByBit, Decimal::from(1), Decimal::from(1)),
        );

        // 100% fees on each side: effective_buy = 2*price, effective_sell = 0
        // This should result in unprofitable (or at least not crash)
        let buy_price = Decimal::from(100);
        let sell_price = Decimal::from(300);
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        // effective_buy = 200, effective_sell = 0, so unprofitable
        assert!(result.is_unprofitable());
    }

    #[test]
    fn test_calculate_net_spread_bps_zero_fees() {
        let config = ConfidenceConfig::default();
        let mut scorer = ConfidenceScorer::new(config);

        // Set zero fees
        scorer.update_fee_schedule(
            ExchangeId::OKX,
            FeeSchedule::new(ExchangeId::OKX, Decimal::ZERO, Decimal::ZERO),
        );
        scorer.update_fee_schedule(
            ExchangeId::ByBit,
            FeeSchedule::new(ExchangeId::ByBit, Decimal::ZERO, Decimal::ZERO),
        );

        let buy_price = Decimal::from(50000);
        let sell_price = Decimal::from(50100); // 2 bps spread
        let result = scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            ExchangeId::OKX,
            ExchangeId::ByBit,
        );
        assert!(result.is_profitable());
    }

    #[test]
    fn test_fee_schedule_fee_calculation_safety() {
        // Test that extreme fee values don't cause panics
        let schedule = FeeSchedule::new(
            ExchangeId::OKX,
            Decimal::from(100), // 10000% maker fee
            Decimal::from(100), // 10000% taker fee
        );
        // Should not panic
        assert_eq!(schedule.get_fee_rate(true), Decimal::from(100));
        assert_eq!(schedule.get_fee_rate(false), Decimal::from(100));
    }
}
