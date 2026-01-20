use arbitrage_core::{
    strategies::{ConvergenceArbitrageStrategy, FilterContext, MarketBundle, Strategy, Ticker},
    types::{ExchangeId, Symbol},
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;

/// Integration test for convergence arbitrage strategy
#[tokio::test]
async fn test_convergence_arbitrage_integration() -> Result<()> {
    let strategy = ConvergenceArbitrageStrategy::new();

    // Create market data with price ratio divergence
    let mut market_bundle = MarketBundle::new();

    // Two related symbols with same quote currency
    let btc_symbol = Symbol::new("BTC", "USDT");
    let eth_symbol = Symbol::new("ETH", "USDT");

    // BTC at $50,000, ETH at $3,000 (ratio = 16.67)
    let btc_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::from(49950), // bid
        Decimal::from(50050), // ask
        Decimal::from(50000), // last
    );

    // ETH significantly undervalued relative to normal ratio
    // Normal BTC/ETH ratio is around 16-20, let's make ETH very cheap
    let eth_ticker = Ticker::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        Decimal::from(1950), // bid (very low price creates large divergence)
        Decimal::from(2050), // ask
        Decimal::from(2000), // last - BTC/ETH ratio = 50000/2000 = 25 (high divergence)
    );

    market_bundle.add_ticker(Arc::new(btc_ticker));
    market_bundle.add_ticker(Arc::new(eth_ticker));

    // Run strategy detection
    let signals = strategy.detect(&market_bundle)?;

    println!("Detected {} convergence arbitrage signals", signals.len());

    // Debug: Check what symbols we have
    let symbols = market_bundle.get_all_symbols();
    println!("Available symbols: {:?}", symbols);

    // Debug: Check if symbols meet criteria
    for i in 0..symbols.len() {
        for j in (i + 1)..symbols.len() {
            let symbol1 = &symbols[i];
            let symbol2 = &symbols[j];

            println!(
                "Checking pair: {} vs {}",
                symbol1.to_pair(),
                symbol2.to_pair()
            );
            println!(
                "  Same base: {} vs {} = {}",
                symbol1.base,
                symbol2.base,
                symbol1.base == symbol2.base
            );
            println!(
                "  Same quote: {} vs {} = {}",
                symbol1.quote,
                symbol2.quote,
                symbol1.quote == symbol2.quote
            );

            // Skip if symbols are too similar (same base or quote)
            if symbol1.base == symbol2.base || symbol1.quote == symbol2.quote {
                println!("  Skipped: same base or quote");
                continue;
            }

            // Only look at symbols with same quote currency for simplicity
            if symbol1.quote != symbol2.quote {
                println!("  Skipped: different quote currencies");
                continue;
            }

            println!("  Would process this pair");
        }
    }

    println!("Detected {} convergence arbitrage signals", signals.len());
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

        if let Some(ratio) = signal.metadata.get("current_ratio") {
            println!("  Current ratio: {}", ratio);
        }
        if let Some(deviation) = signal.metadata.get("ratio_deviation") {
            println!("  Ratio deviation: {}", deviation);
        }
    }

    // Should detect convergence opportunity due to ratio divergence
    assert!(
        !signals.is_empty(),
        "Should detect convergence arbitrage opportunity"
    );

    let signal = &signals[0];
    assert_eq!(signal.legs.len(), 2, "Should have 2 legs (long + short)");
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

/// Test convergence arbitrage with normal ratios (no opportunity)
#[tokio::test]
async fn test_convergence_arbitrage_no_opportunity() -> Result<()> {
    let strategy = ConvergenceArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    // Two symbols with normal price ratio (close to 1.0)
    let usdt_symbol = Symbol::new("USDT", "USD");
    let usdc_symbol = Symbol::new("USDC", "USD");

    // Both stablecoins trading close to $1.00 (normal ratio ~1.0)
    let usdt_ticker = Ticker::new(
        ExchangeId::OKX,
        usdt_symbol.clone(),
        Decimal::new(9995, 4),  // $0.9995
        Decimal::new(10005, 4), // $1.0005
        Decimal::ONE,
    );

    let usdc_ticker = Ticker::new(
        ExchangeId::OKX,
        usdc_symbol.clone(),
        Decimal::new(9998, 4),  // $0.9998
        Decimal::new(10002, 4), // $1.0002
        Decimal::ONE,
    );

    market_bundle.add_ticker(Arc::new(usdt_ticker));
    market_bundle.add_ticker(Arc::new(usdc_ticker));

    let signals = strategy.detect(&market_bundle)?;

    // Should not detect opportunity with normal ratios
    assert!(
        signals.is_empty(),
        "Should not detect opportunity with normal price ratios"
    );

    Ok(())
}

/// Test convergence arbitrage filtering
#[tokio::test]
async fn test_convergence_arbitrage_filtering() -> Result<()> {
    let strategy = ConvergenceArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    // Create symbols with moderate divergence
    let btc_symbol = Symbol::new("BTC", "USDT");
    let eth_symbol = Symbol::new("ETH", "USDT");

    // BTC at $50,000
    let btc_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    // ETH at $2,800 (creates moderate ratio divergence)
    let eth_ticker = Ticker::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        Decimal::from(2790),
        Decimal::from(2810),
        Decimal::from(2800),
    );

    market_bundle.add_ticker(Arc::new(btc_ticker));
    market_bundle.add_ticker(Arc::new(eth_ticker));

    let signals = strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        // Test filtering with normal context
        let mut context = FilterContext::new(50); // 0.5% minimum profit

        // Set up inventory limits for the test
        // For long leg (buying), need quote currency (USDT)
        // For short leg (selling), need base currency (BTC or ETH)
        context.set_inventory_limit(ExchangeId::OKX, "USDT", Decimal::from(100000)); // $100k USDT
        context.set_inventory_limit(ExchangeId::OKX, "BTC", Decimal::from(10)); // 10 BTC
        context.set_inventory_limit(ExchangeId::OKX, "ETH", Decimal::from(100)); // 100 ETH

        // Increase max exposure to handle the signal size
        context.max_exposure = Decimal::from(100000); // $100k max exposure

        let is_valid = strategy.filter(signal, &context)?;

        println!(
            "Signal profit: {} bps, filter result: {}",
            signal.expected_profit_bps, is_valid
        );

        // Should pass basic filtering if profit is sufficient
        if signal.expected_profit_bps >= 50 {
            assert!(is_valid, "Valid signal should pass filtering");
        }

        // Test filtering with very high profit requirement
        let mut strict_context = FilterContext::new(2000); // 20% minimum profit (unrealistic)

        // Set up inventory limits for strict test too
        strict_context.set_inventory_limit(ExchangeId::OKX, "USDT", Decimal::from(100000));
        strict_context.set_inventory_limit(ExchangeId::OKX, "BTC", Decimal::from(10));
        strict_context.set_inventory_limit(ExchangeId::OKX, "ETH", Decimal::from(100));
        strict_context.max_exposure = Decimal::from(100000); // Same exposure limit

        let is_valid_strict = strategy.filter(signal, &strict_context)?;

        // Should fail strict filtering
        assert!(!is_valid_strict, "Signal should fail strict filtering");
    }

    Ok(())
}

/// Test convergence arbitrage with different quote currencies (should be skipped)
#[tokio::test]
async fn test_convergence_arbitrage_different_quotes() -> Result<()> {
    let strategy = ConvergenceArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    // Symbols with different quote currencies (should be skipped)
    let btc_usdt = Symbol::new("BTC", "USDT");
    let eth_usd = Symbol::new("ETH", "USD");

    let btc_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_usdt.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    let eth_ticker = Ticker::new(
        ExchangeId::OKX,
        eth_usd.clone(),
        Decimal::from(2950),
        Decimal::from(3050),
        Decimal::from(3000),
    );

    market_bundle.add_ticker(Arc::new(btc_ticker));
    market_bundle.add_ticker(Arc::new(eth_ticker));

    let signals = strategy.detect(&market_bundle)?;

    // Should not detect opportunity due to different quote currencies
    assert!(
        signals.is_empty(),
        "Should not detect opportunity with different quote currencies"
    );

    Ok(())
}

/// Test convergence arbitrage with same base/quote (should be skipped)
#[tokio::test]
async fn test_convergence_arbitrage_same_assets() -> Result<()> {
    let strategy = ConvergenceArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    // Symbols with overlapping assets (should be skipped)
    let btc_usdt = Symbol::new("BTC", "USDT");
    let btc_usd = Symbol::new("BTC", "USD"); // Same base asset

    let btc_usdt_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_usdt.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    let btc_usd_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_usd.clone(),
        Decimal::from(49900),
        Decimal::from(50100),
        Decimal::from(50000),
    );

    market_bundle.add_ticker(Arc::new(btc_usdt_ticker));
    market_bundle.add_ticker(Arc::new(btc_usd_ticker));

    let signals = strategy.detect(&market_bundle)?;

    // Should not detect opportunity due to same base asset
    assert!(
        signals.is_empty(),
        "Should not detect opportunity with same base assets"
    );

    Ok(())
}

/// Test convergence arbitrage with multiple exchanges
#[tokio::test]
async fn test_convergence_arbitrage_multiple_exchanges() -> Result<()> {
    let strategy = ConvergenceArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    let btc_symbol = Symbol::new("BTC", "USDT");
    let eth_symbol = Symbol::new("ETH", "USDT");

    // Add tickers for both symbols on OKX
    let btc_okx = Ticker::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    let eth_okx = Ticker::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        Decimal::from(2450), // Undervalued
        Decimal::from(2550),
        Decimal::from(2500),
    );

    // Add tickers for both symbols on ByBit (should create separate opportunity)
    let btc_bybit = Ticker::new(
        ExchangeId::ByBit,
        btc_symbol.clone(),
        Decimal::from(50050),
        Decimal::from(50150),
        Decimal::from(50100),
    );

    let eth_bybit = Ticker::new(
        ExchangeId::ByBit,
        eth_symbol.clone(),
        Decimal::from(2400), // Even more undervalued
        Decimal::from(2500),
        Decimal::from(2450),
    );

    market_bundle.add_ticker(Arc::new(btc_okx));
    market_bundle.add_ticker(Arc::new(eth_okx));
    market_bundle.add_ticker(Arc::new(btc_bybit));
    market_bundle.add_ticker(Arc::new(eth_bybit));

    let signals = strategy.detect(&market_bundle)?;

    println!(
        "Detected {} signals across multiple exchanges",
        signals.len()
    );

    // Should detect opportunities on both exchanges
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
        !bybit_signals.is_empty(),
        "Should detect opportunity on ByBit"
    );

    Ok(())
}
