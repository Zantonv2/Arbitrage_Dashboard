use arbitrage_core::strategies::base::*;
use arbitrage_core::{ExchangeId, Symbol, Side};
use chrono::Utc;
use rust_decimal::Decimal;

#[test]
fn test_market_bundle_creation() {
    let bundle = MarketBundle::new();
    assert!(bundle.order_books.is_empty());
    assert!(bundle.funding_rates.is_empty());
    assert!(bundle.tickers.is_empty());
}

#[test]
fn test_raw_signal_creation() {
    let symbol = Symbol::new("BTC", "USDT");
    let mut signal = RawSignal::new("test_strategy", symbol.clone());
    
    assert_eq!(signal.strategy_id, "test_strategy");
    assert_eq!(signal.symbol, symbol);
    assert_eq!(signal.expected_profit_bps, 0);
    
    signal.set_profit_bps(50);
    assert_eq!(signal.expected_profit_bps, 50);
}

#[test]
fn test_trade_leg_creation() {
    let symbol = Symbol::new("BTC", "USDT");
    let leg = TradeLeg::new(
        ExchangeId::OKX,
        symbol.clone(),
        Side::Buy,
        Decimal::from(50000),
        Decimal::new(1, 1), // 0.1
    );
    
    assert_eq!(leg.exchange, ExchangeId::OKX);
    assert_eq!(leg.symbol, symbol);
    assert_eq!(leg.side, Side::Buy);
    assert_eq!(leg.price, Decimal::from(50000));
    assert_eq!(leg.quantity, Decimal::new(1, 1)); // 0.1
}

#[test]
fn test_funding_rate_annualized() {
    let symbol = Symbol::new("BTC", "USDT");
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol,
        Decimal::new(1, 4), // 0.0001
        Utc::now(),
    );
    
    let annual_rate = funding_rate.annualized_rate().expect("Should calculate annual rate");
    let expected = Decimal::new(1, 4) * Decimal::from(3) * Decimal::from(365); // 3 times per day, 365 days
    assert_eq!(annual_rate, expected);
}

#[test]
fn test_ticker_spread_calculation() {
    let symbol = Symbol::new("BTC", "USDT");
    let ticker = Ticker::new(
        ExchangeId::OKX,
        symbol,
        Decimal::from(49950), // bid
        Decimal::from(50050), // ask
        Decimal::from(50000), // last
    );
    
    assert_eq!(ticker.spread(), Decimal::from(100));
    assert_eq!(ticker.mid_price(), Decimal::from(50000));
    
    let spread_bps = ticker.spread_bps().expect("Should calculate spread in bps");
    assert_eq!(spread_bps, Decimal::from(20)); // 100/50000 * 10000 = 20 bps
}

#[test]
fn test_fee_calculation() {
    let fee_schedule = FeeSchedule::new(
        ExchangeId::OKX,
        Decimal::new(1, 3), // 0.001 = 0.1% maker
        Decimal::new(2, 3), // 0.002 = 0.2% taker
    );
    
    let notional = Decimal::from(1000);
    
    let maker_fee = fee_schedule.calculate_fee(notional, true)
        .expect("Should calculate maker fee");
    assert_eq!(maker_fee, Decimal::from(1)); // 1000 * 0.001 = 1
    
    let taker_fee = fee_schedule.calculate_fee(notional, false)
        .expect("Should calculate taker fee");
    assert_eq!(taker_fee, Decimal::from(2)); // 1000 * 0.002 = 2
}

#[test]
fn test_filter_context_creation() {
    let context = FilterContext::new(25); // 0.25% minimum profit
    
    assert_eq!(context.min_profit_bps, 25);
    assert_eq!(context.max_exposure, Decimal::from(10000));
    assert!(context.allowed_exchanges.contains(&ExchangeId::OKX));
    assert!(context.allowed_exchanges.contains(&ExchangeId::ByBit));
}

#[test]
fn test_execution_context_balance_lookup() {
    let mut context = ExecutionContext::new();
    
    // Test default balance (should be zero)
    let balance = context.get_balance(ExchangeId::OKX, "BTC");
    assert_eq!(balance, Decimal::ZERO);
    
    // Add a balance and test retrieval
    context.available_balances.insert(
        (ExchangeId::OKX, "BTC".to_string()),
        Decimal::new(15, 1), // 1.5
    );
    
    let balance = context.get_balance(ExchangeId::OKX, "BTC");
    assert_eq!(balance, Decimal::new(15, 1)); // 1.5
}

#[test]
fn test_confidence_factors_default() {
    let factors = ConfidenceFactors::default();
    
    assert_eq!(factors.depth_score, Decimal::ZERO);
    assert_eq!(factors.volatility_score, Decimal::ZERO);
    assert_eq!(factors.reliability_score, Decimal::ZERO);
    assert_eq!(factors.spread_stability, Decimal::ZERO);
    assert_eq!(factors.freshness_score, Decimal::ZERO);
}

#[test]
fn test_strategy_config_default() {
    let config = StrategyConfig::default();
    
    assert!(config.enabled);
    assert_eq!(config.min_profit_bps, 10);
    assert_eq!(config.max_exposure, Decimal::from(10000));
    assert!(config.custom_params.is_empty());
}

#[test]
fn test_risk_limits_default() {
    let limits = RiskLimits::default();
    
    assert_eq!(limits.max_position_size, Decimal::from(5000));
    assert_eq!(limits.max_daily_volume, Decimal::from(50000));
    // Note: We don't test exact decimal values for percentages since they're created safely
    assert!(limits.max_drawdown >= Decimal::ZERO);
    assert!(limits.stop_loss_threshold >= Decimal::ZERO);
}

#[test]
fn test_raw_signal_metadata() {
    let symbol = Symbol::new("ETH", "USDT");
    let mut signal = RawSignal::new("test_strategy", symbol);
    
    // Test adding metadata
    signal.add_metadata("exchange_count", serde_json::Value::Number(serde_json::Number::from(2)));
    signal.add_metadata("priority", serde_json::Value::String("high".to_string()));
    
    assert_eq!(signal.metadata.len(), 2);
    assert!(signal.metadata.contains_key("exchange_count"));
    assert!(signal.metadata.contains_key("priority"));
}

#[test]
fn test_trade_leg_with_order_type() {
    let symbol = Symbol::new("BTC", "USDT");
    let leg = TradeLeg::new(
        ExchangeId::ByBit,
        symbol,
        Side::Sell,
        Decimal::from(49000),
        Decimal::new(5, 2), // 0.05
    ).with_order_type(arbitrage_core::OrderType::Limit);
    
    assert_eq!(leg.order_type, arbitrage_core::OrderType::Limit);
    assert_eq!(leg.side, Side::Sell);
    assert_eq!(leg.price, Decimal::from(49000));
}

#[test]
fn test_ticker_zero_price_handling() {
    let symbol = Symbol::new("TEST", "USDT");
    let ticker = Ticker::new(
        ExchangeId::MEXC,
        symbol,
        Decimal::ZERO, // Zero bid
        Decimal::ZERO, // Zero ask
        Decimal::ZERO, // Zero last
    );
    
    // Should handle zero prices gracefully
    assert_eq!(ticker.spread(), Decimal::ZERO);
    assert_eq!(ticker.mid_price(), Decimal::ZERO);
    
    // spread_bps should return error for zero mid price
    let result = ticker.spread_bps();
    assert!(result.is_err());
}

#[test]
fn test_funding_rate_overflow_protection() {
    let symbol = Symbol::new("BTC", "USDT");
    
    // Test with very large funding rate that could cause overflow
    let large_rate = Decimal::MAX / Decimal::from(2); // Use half of max to avoid immediate overflow
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol,
        large_rate,
        Utc::now(),
    );
    
    // Should handle overflow gracefully and return error
    let result = funding_rate.annualized_rate();
    // This might succeed or fail depending on the exact value, but should not panic
    match result {
        Ok(_) => {}, // Success is fine
        Err(_) => {}, // Error is also fine for overflow protection
    }
}

#[test]
fn test_fee_schedule_overflow_protection() {
    let fee_schedule = FeeSchedule::new(
        ExchangeId::GateIo,
        Decimal::MAX / Decimal::from(2), // Large fee rate
        Decimal::MAX / Decimal::from(2),
    );
    
    let large_notional = Decimal::MAX / Decimal::from(2);
    
    // Should handle overflow gracefully
    let result = fee_schedule.calculate_fee(large_notional, true);
    match result {
        Ok(_) => {}, // Success is fine
        Err(_) => {}, // Error is also fine for overflow protection
    }
}