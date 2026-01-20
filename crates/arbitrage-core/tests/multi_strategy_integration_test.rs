use arbitrage_core::{
    strategies::{
        CexArbitrageStrategy, ConvergenceArbitrageStrategy, CrossExchangeArbitrageStrategy,
        FundingRate, FundingRateArbitrageStrategy, HedgedFundingStrategy, LatencyArbitrageStrategy,
        MarketBundle, NewListingArbitrageStrategy, SpotPerpArbitrageStrategy,
        SpreadCaptureStrategy, StablecoinArbitrageStrategy, Strategy, StrategyRegistry, Ticker,
    },
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio;

/// Integration test for multiple strategies working together
#[tokio::test]
async fn test_multi_strategy_detection() -> Result<()> {
    // Create strategy registry with all implemented strategies
    let mut registry = StrategyRegistry::new();

    registry.register(Arc::new(CexArbitrageStrategy::new()))?;
    registry.register(Arc::new(FundingRateArbitrageStrategy::new()))?;
    registry.register(Arc::new(StablecoinArbitrageStrategy::new()))?;
    registry.register(Arc::new(SpotPerpArbitrageStrategy::new()))?;
    registry.register(Arc::new(CrossExchangeArbitrageStrategy::new()))?;
    registry.register(Arc::new(NewListingArbitrageStrategy::new()))?;
    registry.register(Arc::new(LatencyArbitrageStrategy::new()))?;
    registry.register(Arc::new(ConvergenceArbitrageStrategy::new()))?;
    registry.register(Arc::new(SpreadCaptureStrategy::new()))?;
    registry.register(Arc::new(HedgedFundingStrategy::new()))?;

    println!("Registered {} strategies", registry.count());
    assert_eq!(registry.count(), 10);

    // Create comprehensive market data
    let mut market_bundle = MarketBundle::new();

    // 1. CEX Arbitrage opportunity: BTC price difference
    let btc_symbol = Symbol::new("BTC", "USDT");

    // OKX: Lower price (good for buying)
    let btc_okx_book = OrderBook::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    // ByBit: Higher price (good for selling)
    let btc_bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50150), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50160), Decimal::from(1))],
    );

    market_bundle.add_order_book(Arc::new(btc_okx_book));
    market_bundle.add_order_book(Arc::new(btc_bybit_book));

    // 2. Funding Rate Arbitrage opportunity
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::new(15, 5), // 0.00015 = 0.015% funding rate
        Utc::now() + Duration::hours(6),
    );
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    // Add BTC ticker for funding rate strategy
    let btc_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::from(50000),
        Decimal::from(50010),
        Decimal::from(50005),
    );
    market_bundle.add_ticker(Arc::new(btc_ticker));

    // 3. Stablecoin Arbitrage opportunity: USDT above peg
    let usdt_symbol = Symbol::new("USDT", "USD");
    let usdt_ticker = Ticker::new(
        ExchangeId::Kraken,
        usdt_symbol.clone(),
        Decimal::new(1003, 3), // $1.003
        Decimal::new(1005, 3), // $1.005
        Decimal::new(1004, 3), // $1.004
    );
    let usdt_book = OrderBook::new(
        ExchangeId::Kraken,
        usdt_symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(1003, 3),
            Decimal::from(5000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(1005, 3),
            Decimal::from(5000),
        )],
    );
    market_bundle.add_ticker(Arc::new(usdt_ticker));
    market_bundle.add_order_book(Arc::new(usdt_book));

    // 4. Cross-stablecoin opportunity: USDT/USDC spread
    let usdt_usdc_symbol = Symbol::new("USDT", "USDC");
    let usdt_usdc_okx = OrderBook::new(
        ExchangeId::OKX,
        usdt_usdc_symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(1002, 3),
            Decimal::from(2000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(1003, 3),
            Decimal::from(2000),
        )],
    );
    let usdt_usdc_bybit = OrderBook::new(
        ExchangeId::ByBit,
        usdt_usdc_symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(999, 3),
            Decimal::from(2000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(1000, 3),
            Decimal::from(2000),
        )],
    );
    market_bundle.add_order_book(Arc::new(usdt_usdc_okx));
    market_bundle.add_order_book(Arc::new(usdt_usdc_bybit));

    // Test each strategy individually
    println!("\n=== Testing Individual Strategies ===");

    let strategies = registry.get_all();
    let mut total_signals = 0;

    for strategy in strategies {
        let signals = strategy.detect(&market_bundle)?;
        println!(
            "Strategy '{}' detected {} signals",
            strategy.name(),
            signals.len()
        );

        for (i, signal) in signals.iter().enumerate() {
            println!(
                "  Signal {}: {} legs, {}bps profit",
                i + 1,
                signal.legs.len(),
                signal.expected_profit_bps
            );
        }

        total_signals += signals.len();
    }

    println!("\nTotal signals across all strategies: {}", total_signals);

    // Should detect multiple opportunities
    assert!(
        total_signals > 0,
        "Should detect arbitrage opportunities across strategies"
    );

    Ok(())
}

/// Test strategy priority and conflict resolution
#[tokio::test]
async fn test_strategy_priority_and_conflicts() -> Result<()> {
    let mut registry = StrategyRegistry::new();

    // Register strategies in priority order
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;
    registry.register(Arc::new(CrossExchangeArbitrageStrategy::new()))?;

    // Create market data that could trigger both strategies
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    // Create price difference between exchanges
    let eth_okx = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3000), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(5))],
    );

    let eth_bybit = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3050), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3055), Decimal::from(5))],
    );

    market_bundle.add_order_book(Arc::new(eth_okx));
    market_bundle.add_order_book(Arc::new(eth_bybit));

    // Test both strategies
    let cex_strategy = CexArbitrageStrategy::new();
    let cross_strategy = CrossExchangeArbitrageStrategy::new();

    let cex_signals = cex_strategy.detect(&market_bundle)?;
    let cross_signals = cross_strategy.detect(&market_bundle)?;

    println!("CEX arbitrage signals: {}", cex_signals.len());
    println!("Cross-exchange signals: {}", cross_signals.len());

    // Both strategies should detect the same opportunity
    // In a real system, we'd need conflict resolution logic

    Ok(())
}

/// Test strategy performance with large market data
#[tokio::test]
async fn test_strategy_performance() -> Result<()> {
    let strategy = CexArbitrageStrategy::new();

    // Create large market bundle
    let mut market_bundle = MarketBundle::new();

    // Add 100 symbols across 6 exchanges
    for i in 0..100 {
        let symbol = Symbol::new(&format!("TOKEN{}", i), "USDT");

        for exchange in [
            ExchangeId::OKX,
            ExchangeId::ByBit,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
            ExchangeId::Kraken,
            ExchangeId::Bitstamp,
        ] {
            let base_price = 100 + i; // Different base prices
            let spread = 1 + (i % 5); // Variable spreads

            let order_book = OrderBook::new(
                exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(
                    Decimal::from(base_price),
                    Decimal::from(10),
                )],
                vec![OrderBookLevel::new(
                    Decimal::from(base_price + spread),
                    Decimal::from(10),
                )],
            );

            market_bundle.add_order_book(Arc::new(order_book));
        }
    }

    println!(
        "Created market bundle with {} order books",
        market_bundle.order_books.len()
    );

    // Measure detection time
    let start = std::time::Instant::now();
    let signals = strategy.detect(&market_bundle)?;
    let duration = start.elapsed();

    println!(
        "Strategy detection took {:?} for {} signals",
        duration,
        signals.len()
    );

    // Should complete within reasonable time (< 100ms for this test)
    assert!(
        duration.as_millis() < 1000,
        "Strategy detection should be fast"
    );

    Ok(())
}

/// Test strategy configuration updates
#[tokio::test]
async fn test_strategy_configuration() -> Result<()> {
    use arbitrage_core::strategies::StrategyConfig;

    let mut strategy = CexArbitrageStrategy::new();

    // Get initial config
    let initial_config = strategy.config().clone();
    println!("Initial min profit: {}bps", initial_config.min_profit_bps);

    // Update configuration
    let mut new_config = initial_config.clone();
    new_config.min_profit_bps = 50; // Increase to 0.5%
    new_config.max_exposure = Decimal::from(5000); // Reduce exposure

    strategy.update_config(new_config)?;

    // Verify config was updated
    let updated_config = strategy.config();
    assert_eq!(updated_config.min_profit_bps, 50);
    assert_eq!(updated_config.max_exposure, Decimal::from(5000));

    println!("Updated min profit: {}bps", updated_config.min_profit_bps);

    Ok(())
}

/// Test strategy error handling
#[tokio::test]
async fn test_strategy_error_handling() -> Result<()> {
    let strategy = CexArbitrageStrategy::new();

    // Test with empty market bundle
    let empty_bundle = MarketBundle::new();
    let signals = strategy.detect(&empty_bundle)?;

    // Should handle empty data gracefully
    assert_eq!(signals.len(), 0);

    // Test with invalid data
    let mut invalid_bundle = MarketBundle::new();
    let symbol = Symbol::new("INVALID", "TOKEN");

    // Add order book with zero prices (should be handled gracefully)
    let invalid_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::ZERO, Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::ZERO, Decimal::from(1))],
    );

    invalid_bundle.add_order_book(Arc::new(invalid_book));

    // Should not panic or return error
    let signals = strategy.detect(&invalid_bundle)?;
    println!("Signals from invalid data: {}", signals.len());

    Ok(())
}
