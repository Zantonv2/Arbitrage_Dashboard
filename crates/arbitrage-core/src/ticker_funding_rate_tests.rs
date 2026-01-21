#[cfg(test)]
mod tests {
    use crate::strategies::{FundingRate, Ticker};
    use crate::types::{ExchangeId, Symbol};
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;

    fn create_test_ticker() -> Ticker {
        Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        )
    }

    fn create_test_funding_rate() -> FundingRate {
        FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4), // 0.001 = 0.1%
            Utc::now() + Duration::hours(1),
        )
    }

    // === Ticker Tests ===

    #[test]
    fn test_ticker_new() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );

        assert_eq!(ticker.exchange, ExchangeId::OKX);
        assert_eq!(ticker.bid, Decimal::from(50000));
        assert_eq!(ticker.ask, Decimal::from(50010));
        assert_eq!(ticker.last, Decimal::from(50005));
        assert_eq!(ticker.volume_24h, Decimal::ZERO);
        assert_eq!(ticker.change_24h, Decimal::ZERO);
    }

    #[test]
    fn test_ticker_with_volume() {
        let mut ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );
        ticker.volume_24h = Decimal::from(1000);
        ticker.change_24h = Decimal::new(250, 2); // 2.5%

        assert_eq!(ticker.volume_24h, Decimal::from(1000));
        assert_eq!(ticker.change_24h, Decimal::new(250, 2));
    }

    #[test]
    fn test_ticker_spread() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );

        let spread = ticker.spread();
        assert_eq!(spread, Decimal::from(10));
    }

    #[test]
    fn test_ticker_spread_zero_prices() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        );

        let spread = ticker.spread();
        assert_eq!(spread, Decimal::ZERO);
    }

    #[test]
    fn test_ticker_mid_price() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );

        let mid_price = ticker.mid_price();
        assert!(mid_price.is_ok());
        assert_eq!(mid_price.unwrap(), Decimal::from(50005));
    }

    #[test]
    fn test_ticker_mid_price_zero_bid() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::ZERO,
            Decimal::from(50010),
            Decimal::from(50005),
        );

        let mid_price = ticker.mid_price();
        assert!(mid_price.is_ok());
        assert_eq!(mid_price.unwrap(), Decimal::from(25005));
    }

    #[test]
    fn test_ticker_mid_price_zero_ask() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::ZERO,
            Decimal::from(50005),
        );

        let mid_price = ticker.mid_price();
        assert!(mid_price.is_ok());
        assert_eq!(mid_price.unwrap(), Decimal::from(25000));
    }

    #[test]
    fn test_ticker_mid_price_both_zero() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        );

        let mid_price = ticker.mid_price();
        assert!(mid_price.is_err());
        assert!(mid_price.unwrap_err().to_string().contains("zero"));
    }

    #[test]
    fn test_ticker_mid_price_large_values() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(1_000_000_000),
            Decimal::from(1_000_000_010),
            Decimal::from(1_000_000_005),
        );

        let mid_price = ticker.mid_price();
        assert!(mid_price.is_ok());
        assert_eq!(mid_price.unwrap(), Decimal::from(1_000_000_005));
    }

    #[test]
    fn test_ticker_mid_price_small_values() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("PEPE", "USDT"),
            Decimal::new(1, 8),  // 0.00000001
            Decimal::new(2, 8),  // 0.00000002
            Decimal::new(15, 9), // 0.0000000015
        );

        let mid_price = ticker.mid_price();
        assert!(mid_price.is_ok());
        let expected = (Decimal::new(1, 8) + Decimal::new(2, 8)) / Decimal::from(2);
        assert_eq!(mid_price.unwrap(), expected);
    }

    #[test]
    fn test_ticker_spread_bps() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );

        let spread_bps = ticker.spread_bps();
        assert!(spread_bps.is_ok());
        // Spread is 10, mid price is 50005, so spread_bps = 10/50005 * 10000 ≈ 2 bps
        let bps_value = spread_bps.unwrap();
        assert!(bps_value > Decimal::ZERO);
        assert!(bps_value < Decimal::from(100));
    }

    #[test]
    fn test_ticker_spread_bps_zero_mid_price() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        );

        let spread_bps = ticker.spread_bps();
        assert!(spread_bps.is_err());
    }

    #[test]
    fn test_ticker_spread_bps_narrow_spread() {
        // 1 basis point spread
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50005), // 5 = 1 bps of 50000
            Decimal::from(500025),
        );

        let spread_bps = ticker.spread_bps();
        assert!(spread_bps.is_ok());
        let bps = spread_bps.unwrap();
        // Spread is 5, mid is 50002.5, so 5/50002.5 * 10000 ≈ 0.99995 bps
        // Allow values slightly less than 1
        assert!(bps >= Decimal::new(9, 1)); // At least 0.9 bps
        assert!(bps <= Decimal::from(2)); // At most 2 bps
    }

    #[test]
    fn test_ticker_spread_bps_wide_spread() {
        // 1000 basis point spread (10%)
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("ALT", "USDT"),
            Decimal::from(10000),
            Decimal::from(11000), // 1000 = 10% of 10000
            Decimal::from(10500),
        );

        let spread_bps = ticker.spread_bps();
        assert!(spread_bps.is_ok());
        let bps = spread_bps.unwrap();
        // Should be approximately 1000 bps
        assert!(bps >= Decimal::from(950));
        assert!(bps <= Decimal::from(1050));
    }

    #[test]
    fn test_ticker_is_valid() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );

        assert!(ticker.is_valid());
    }

    #[test]
    fn test_ticker_is_valid_zero_bid() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::ZERO,
            Decimal::from(50010),
            Decimal::from(50005),
        );

        assert!(!ticker.is_valid());
    }

    #[test]
    fn test_ticker_is_valid_zero_ask() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::ZERO,
            Decimal::from(50005),
        );

        assert!(!ticker.is_valid());
    }

    #[test]
    fn test_ticker_is_valid_bid_greater_than_ask() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50020),
            Decimal::from(50010),
            Decimal::from(50015),
        );

        assert!(!ticker.is_valid());
    }

    #[test]
    fn test_ticker_is_valid_equal_prices() {
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50000),
            Decimal::from(50000),
        );

        assert!(!ticker.is_valid());
    }

    #[test]
    fn test_ticker_clone() {
        let ticker1 = create_test_ticker();
        let ticker2 = ticker1.clone();

        assert_eq!(ticker1.bid, ticker2.bid);
        assert_eq!(ticker1.ask, ticker2.ask);
        assert_eq!(ticker1.last, ticker2.last);
    }

    // === FundingRate Tests ===

    #[test]
    fn test_funding_rate_new() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4), // 0.001 = 0.1%
            Utc::now() + Duration::hours(1),
        );

        assert_eq!(funding_rate.exchange, ExchangeId::OKX);
        assert_eq!(funding_rate.rate, Decimal::new(10, 4));
        assert!(funding_rate.next_funding > Utc::now());
        assert!(funding_rate.predicted_rate.is_none());
    }

    #[test]
    fn test_funding_rate_with_predicted() {
        let mut funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4),
            Utc::now() + Duration::hours(1),
        );
        funding_rate.predicted_rate = Some(Decimal::new(12, 4));

        assert_eq!(funding_rate.predicted_rate, Some(Decimal::new(12, 4)));
    }

    #[test]
    fn test_funding_rate_annualized_rate() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4), // 0.001 = 0.1% every 8 hours
            Utc::now() + Duration::hours(1),
        );

        let annual_rate = funding_rate.annualized_rate();
        assert!(annual_rate.is_ok());
        // 0.001 * 3 * 365 = 1.095 (109.5% annualized)
        let expected = Decimal::new(1095, 3); // 1.095
        assert_eq!(annual_rate.unwrap(), expected);
    }

    #[test]
    fn test_funding_rate_annualized_rate_zero() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::ZERO,
            Utc::now() + Duration::hours(1),
        );

        let annual_rate = funding_rate.annualized_rate();
        assert!(annual_rate.is_ok());
        assert_eq!(annual_rate.unwrap(), Decimal::ZERO);
    }

    #[test]
    fn test_funding_rate_annualized_rate_negative() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(-10, 4), // -0.1%
            Utc::now() + Duration::hours(1),
        );

        let annual_rate = funding_rate.annualized_rate();
        assert!(annual_rate.is_ok());
        // Should be negative
        assert!(annual_rate.unwrap() < Decimal::ZERO);
    }

    #[test]
    fn test_funding_rate_annualized_rate_large() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(100, 2), // 1.0%
            Utc::now() + Duration::hours(1),
        );

        let annual_rate = funding_rate.annualized_rate();
        assert!(annual_rate.is_ok());
        // 1% * 3 * 365 = 1095% (10.95x)
        let rate = annual_rate.unwrap();
        assert!(rate > Decimal::from(1000));
    }

    #[test]
    fn test_funding_rate_is_valid() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4),
            Utc::now() + Duration::hours(1),
        );

        assert!(funding_rate.is_valid());
    }

    #[test]
    fn test_funding_rate_is_valid_negative_boundary() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(-100, 2), // -1.0%
            Utc::now() + Duration::hours(1),
        );

        assert!(funding_rate.is_valid());
    }

    #[test]
    fn test_funding_rate_is_valid_positive_boundary() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(100, 2), // 1.0%
            Utc::now() + Duration::hours(1),
        );

        assert!(funding_rate.is_valid());
    }

    #[test]
    fn test_funding_rate_is_valid_too_positive() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(101, 2), // 1.01% - exceeds 1.0% limit
            Utc::now() + Duration::hours(1),
        );

        assert!(!funding_rate.is_valid());
    }

    #[test]
    fn test_funding_rate_is_valid_too_negative() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(-101, 2), // -1.01% - exceeds -1.0% limit
            Utc::now() + Duration::hours(1),
        );

        assert!(!funding_rate.is_valid());
    }

    #[test]
    fn test_funding_rate_is_positive() {
        let positive_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4),
            Utc::now() + Duration::hours(1),
        );

        assert!(positive_rate.is_positive());

        let negative_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(-10, 4),
            Utc::now() + Duration::hours(1),
        );

        assert!(!negative_rate.is_positive());

        let zero_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::ZERO,
            Utc::now() + Duration::hours(1),
        );

        assert!(!zero_rate.is_positive());
    }

    #[test]
    fn test_funding_rate_time_to_funding() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4),
            Utc::now() + Duration::hours(1),
        );

        let time_seconds = funding_rate.time_to_funding();
        // Should be approximately 1 hour (3600 seconds)
        assert!(time_seconds > 3500);
        assert!(time_seconds < 3700);
    }

    #[test]
    fn test_funding_rate_time_to_funding_future() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4),
            Utc::now() + Duration::hours(8),
        );

        let time_seconds = funding_rate.time_to_funding();
        // Should be approximately 8 hours (28800 seconds)
        assert!(time_seconds > 28000);
        assert!(time_seconds < 29600);
    }

    #[test]
    fn test_funding_rate_time_to_funding_past() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(10, 4),
            Utc::now() - Duration::hours(1),
        );

        let time_seconds = funding_rate.time_to_funding();
        // Should be negative (past funding)
        assert!(time_seconds < 0);
    }

    #[test]
    fn test_funding_rate_clone() {
        let rate1 = create_test_funding_rate();
        let rate2 = rate1.clone();

        assert_eq!(rate1.exchange, rate2.exchange);
        assert_eq!(rate1.rate, rate2.rate);
        assert_eq!(rate1.next_funding, rate2.next_funding);
    }

    #[test]
    fn test_funding_rate_extreme_positive_rate() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(9999, 4), // 0.9999 = 99.99%
            Utc::now() + Duration::hours(1),
        );

        assert!(funding_rate.is_valid());
        let annual_rate = funding_rate.annualized_rate();
        assert!(annual_rate.is_ok());
        // Should be very large (0.9999 * 3 * 365 ≈ 1094.89)
        assert!(annual_rate.unwrap() > Decimal::from(1000));
    }

    #[test]
    fn test_funding_rate_extreme_negative_rate() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(-9999, 4), // -0.9999 = -99.99%
            Utc::now() + Duration::hours(1),
        );

        assert!(funding_rate.is_valid());
        let annual_rate = funding_rate.annualized_rate();
        assert!(annual_rate.is_ok());
        // Should be very negative
        assert!(annual_rate.unwrap() < Decimal::from(-1000));
    }

    #[test]
    fn test_funding_rate_very_small_rate() {
        let funding_rate = FundingRate::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::new(1, 8), // 0.00000001
            Utc::now() + Duration::hours(1),
        );

        assert!(funding_rate.is_valid());
        let annual_rate = funding_rate.annualized_rate();
        assert!(annual_rate.is_ok());
        assert!(annual_rate.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn test_ticker_boundary_prices() {
        // Minimum non-zero prices
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("SHIB", "USDT"),
            Decimal::new(1, 10), // 0.0000000001
            Decimal::new(2, 10),
            Decimal::new(15, 11),
        );

        assert!(ticker.is_valid());
        let mid = ticker.mid_price();
        assert!(mid.is_ok());
    }

    #[test]
    fn test_ticker_extreme_spread() {
        // Extreme spread for testing
        let ticker = Ticker::new(
            ExchangeId::OKX,
            Symbol::new("VOLATILE", "USDT"),
            Decimal::from(1000),
            Decimal::from(5000), // 400% spread
            Decimal::from(3000),
        );

        let spread_bps = ticker.spread_bps();
        assert!(spread_bps.is_ok());
        let bps = spread_bps.unwrap();
        // Spread is 4000, mid is 3000, so 4000/3000 * 10000 = 13333.33 bps
        assert!(bps >= Decimal::from(10000));
        assert!(bps <= Decimal::from(15000));
    }
}
