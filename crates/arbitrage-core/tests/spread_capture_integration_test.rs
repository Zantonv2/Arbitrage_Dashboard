use arbitrage_core::{
    strategies::{
        FilterContext, MarketBundle, SpreadCaptureStrategy, Strategy, StrategyConfig, Ticker,
    },
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use rust_decimal::Decimal;
use serde_json::json;

/// Integration test for spread capture strategy
#[tokio::test]
async fn test_spread_capture_integration() -> Result<()> {
    // Create strategy with more lenient maker fee requirements
    let mut config = arbitrage_core::strategies::StrategyConfig::default();
    config.min_profit_bps = 10; // 0.1% minimum profit

    // Set more lenient maker fee threshold to allow OKX (-2 bps)
    config
        .custom_params
        .insert("maker_fee_threshold_bps".to_string(), serde_json::json!(0)); // Allow up to 0 bps
    config
        .custom_params
        .insert("min_spread_bps".to_string(), serde_json::json!(20)); // 0.20% minimum spread
    config
        .custom_params
        .insert("min_order_book_depth".to_string(), serde_json::json!(1000)); // $1000 minimum depth

    let strategy = SpreadCaptureStrategy::with_config(config);

    // Create market data with wide spread opportunity
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    // OKX with very wide spread (good for spread capture)
    // Using 1000 USDT spread on 50,000 USDT = 2% = 200 bps (well above 20 bps minimum)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![
            OrderBookLevel::new(Decimal::from(49500), Decimal::from(5)), // bid - more liquidity
            OrderBookLevel::new(Decimal::from(49400), Decimal::from(3)),
            OrderBookLevel::new(Decimal::from(49300), Decimal::from(2)),
        ],
        vec![
            OrderBookLevel::new(Decimal::from(50500), Decimal::from(5)), // ask - more liquidity
            OrderBookLevel::new(Decimal::from(50600), Decimal::from(3)),
            OrderBookLevel::new(Decimal::from(50700), Decimal::from(2)),
        ],
    );

    // Add ticker for price calculation
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49500), // bid
        Decimal::from(50500), // ask
        Decimal::from(50000), // last
    );

    market_bundle.add_order_book(okx_book);
    market_bundle.add_ticker(okx_ticker);

    // Run strategy detection
    let signals = strategy.detect(&market_bundle)?;

    println!("Detected {} spread capture signals", signals.len());
    for (i, signal) in signals.iter().enumerate() {
        println!(
            "Signal {}: {} legs, profit: {} bps",
            i,
            signal.legs.len(),
            signal.expected_profit_bps
        );
        for (j, leg) in signal.legs.iter().enumerate() {
            println!(
                "  Leg {}: {} {:?} {} @ {}",
                j, leg.exchange, leg.side, leg.quantity, leg.price
            );
        }

        // Print metadata for debugging
        if let Some(spread_bps) = signal.metadata.get("spread_bps") {
            println!("  Market spread: {} bps", spread_bps);
        }
        if let Some(our_bid) = signal.metadata.get("our_bid") {
            println!("  Our bid: {}", our_bid);
        }
        if let Some(our_ask) = signal.metadata.get("our_ask") {
            println!("  Our ask: {}", our_ask);
        }
        if let Some(depth) = signal.metadata.get("depth_usd") {
            println!("  Depth: {} USD", depth);
        }
    }

    // Should detect spread capture opportunity
    assert!(
        !signals.is_empty(),
        "Should detect spread capture opportunity"
    );

    let signal = &signals[0];
    assert_eq!(
        signal.legs.len(),
        2,
        "Should have 2 legs (bid + ask orders)"
    );
    assert!(
        signal.expected_profit_bps > 0,
        "Should have positive profit"
    );

    // Both legs should be on the same exchange
    assert_eq!(signal.legs[0].exchange, signal.legs[1].exchange);

    // Should have one buy and one sell order
    let sides: Vec<_> = signal.legs.iter().map(|leg| leg.side).collect();
    assert!(sides.contains(&arbitrage_core::types::Side::Buy));
    assert!(sides.contains(&arbitrage_core::types::Side::Sell));

    Ok(())
}

/// Test spread capture filtering
#[tokio::test]
async fn test_spread_capture_filtering() -> Result<()> {
    let strategy = SpreadCaptureStrategy::new();

    // Create market data with narrow spread (should be filtered out)
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    // Narrow spread (only 5 USDT on 3000 USDT = ~0.17%)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2997), Decimal::from(10))], // bid
        vec![OrderBookLevel::new(Decimal::from(3003), Decimal::from(10))], // ask
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(2997),
        Decimal::from(3003),
        Decimal::from(3000),
    );

    market_bundle.add_order_book(okx_book);
    market_bundle.add_ticker(okx_ticker);

    let signals = strategy.detect(&market_bundle)?;

    // Should not detect opportunity due to narrow spread
    assert!(
        signals.is_empty() || signals[0].expected_profit_bps < 20,
        "Should not detect opportunity with narrow spread"
    );

    Ok(())
}

/// Test spread capture with insufficient depth
#[tokio::test]
async fn test_spread_capture_insufficient_depth() -> Result<()> {
    let strategy = SpreadCaptureStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    // Wide spread but very low liquidity
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::from(49000),
            Decimal::new(1, 3),
        )], // 0.001 BTC
        vec![OrderBookLevel::new(
            Decimal::from(51000),
            Decimal::new(1, 3),
        )], // 0.001 BTC
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49000),
        Decimal::from(51000),
        Decimal::from(50000),
    );

    market_bundle.add_order_book(okx_book);
    market_bundle.add_ticker(okx_ticker);

    let signals = strategy.detect(&market_bundle)?;

    // Should not detect opportunity due to insufficient depth
    assert!(
        signals.is_empty(),
        "Should not detect opportunity with insufficient depth"
    );

    Ok(())
}

/// Test spread capture signal filtering
#[tokio::test]
async fn test_spread_capture_signal_filtering() -> Result<()> {
    let strategy = SpreadCaptureStrategy::new();

    // Create a valid signal
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49800), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49800),
        Decimal::from(50200),
        Decimal::from(50000),
    );

    market_bundle.add_order_book(okx_book);
    market_bundle.add_ticker(okx_ticker);

    let signals = strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        // Test filtering with normal context
        let context = FilterContext::new(10); // 0.1% minimum profit
        let is_valid = strategy.filter(signal, &context)?;

        println!(
            "Signal profit: {} bps, filter result: {}",
            signal.expected_profit_bps, is_valid
        );

        // Should pass basic filtering if profit is sufficient
        if signal.expected_profit_bps >= 10 {
            assert!(is_valid, "Valid signal should pass filtering");
        }

        // Test filtering with high profit requirement
        let strict_context = FilterContext::new(1000); // 10% minimum profit (very high)
        let is_valid_strict = strategy.filter(signal, &strict_context)?;

        // Should fail strict filtering
        assert!(!is_valid_strict, "Signal should fail strict filtering");
    }

    Ok(())
}

/// Test spread capture with multiple exchanges
#[tokio::test]
async fn test_spread_capture_multiple_exchanges() -> Result<()> {
    // Create custom config with relaxed maker fee threshold
    let mut config = StrategyConfig::default();

    // Start with base custom params and add spread capture specific ones
    let mut custom_params = std::collections::HashMap::new();
    custom_params.insert("enabled".to_string(), json!(true));
    custom_params.insert("debug_mode".to_string(), json!(false));
    custom_params.insert("max_signals_per_minute".to_string(), json!(60));
    custom_params.insert("signal_cooldown_ms".to_string(), json!(1000));

    // Add spread capture specific parameters
    custom_params.insert("min_spread_bps".to_string(), json!(20)); // 0.20% minimum spread
    custom_params.insert("max_spread_bps".to_string(), json!(500)); // 5.0% maximum spread
    custom_params.insert("spread_stability_threshold".to_string(), json!(0.8)); // 80% stability
    custom_params.insert("min_order_book_depth".to_string(), json!(1000)); // $1000 minimum depth
    custom_params.insert("max_inventory_ratio".to_string(), json!(0.3)); // 30% max inventory
    custom_params.insert("maker_fee_threshold_bps".to_string(), json!(0)); // Allow up to 0 bps (more lenient)
    custom_params.insert("position_size_usd".to_string(), json!(500)); // $500 per position
    custom_params.insert("spread_capture_ratio".to_string(), json!(0.6)); // Capture 60% of spread

    config.custom_params = custom_params;

    let strategy = SpreadCaptureStrategy::with_config(config);

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    // OKX with good spread
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2980), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3020), Decimal::from(5))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(2980),
        Decimal::from(3020),
        Decimal::from(3000),
    );

    // ByBit with narrow spread (should not generate signal)
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2998), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3002), Decimal::from(5))],
    );

    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(2998),
        Decimal::from(3002),
        Decimal::from(3000),
    );

    market_bundle.add_order_book(okx_book);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_order_book(bybit_book);
    market_bundle.add_ticker(bybit_ticker);

    let signals = strategy.detect(&market_bundle)?;

    println!(
        "Detected {} signals across multiple exchanges",
        signals.len()
    );

    // Should detect opportunity on OKX but not ByBit
    let okx_signals: Vec<_> = signals
        .iter()
        .filter(|s| s.legs.iter().all(|leg| leg.exchange == ExchangeId::OKX))
        .collect();

    let bybit_signals: Vec<_> = signals
        .iter()
        .filter(|s| s.legs.iter().all(|leg| leg.exchange == ExchangeId::ByBit))
        .collect();

    assert!(!okx_signals.is_empty(), "Should detect opportunity on OKX");
    assert!(
        bybit_signals.is_empty(),
        "Should not detect opportunity on ByBit due to narrow spread"
    );

    Ok(())
}
