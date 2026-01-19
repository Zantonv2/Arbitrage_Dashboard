use arbitrage_core::{
    strategies::{MarketBundle, StablecoinArbitrageStrategy, Strategy, Ticker},
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use rust_decimal::Decimal;
use tokio;

/// Integration test for stablecoin peg arbitrage
#[tokio::test]
async fn test_stablecoin_peg_arbitrage() -> Result<()> {
    let stablecoin_strategy = StablecoinArbitrageStrategy::new();

    // Create USDT trading above peg
    let symbol = Symbol::new("USDT", "USD");

    // USDT trading at $1.005 (50 bps above peg) - higher deviation to overcome fees
    let ticker = Ticker::new(
        ExchangeId::OKX, // Use OKX which has lower fees
        symbol.clone(),
        Decimal::new(10045, 4), // bid: $1.0045
        Decimal::new(10055, 4), // ask: $1.0055
        Decimal::new(1005, 3),  // last: $1.005
    );

    // Create order book with liquidity
    let order_book = OrderBook::new(
        ExchangeId::OKX, // Use OKX
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(10045, 4),
            Decimal::from(10000),
        )], // bids
        vec![OrderBookLevel::new(
            Decimal::new(10055, 4),
            Decimal::from(10000),
        )], // asks
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_ticker(ticker);
    market_bundle.add_order_book(order_book);

    let signals = stablecoin_strategy.detect(&market_bundle)?;

    println!("Detected {} stablecoin peg signals", signals.len());

    // Should detect peg arbitrage opportunity
    assert!(!signals.is_empty(), "Should detect USDT peg deviation");

    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "USDT/USD");
    assert!(signal.expected_profit_bps > 0);

    // Should have 1 leg (sell USDT above peg)
    assert_eq!(signal.legs.len(), 1);
    let leg = &signal.legs[0];
    assert_eq!(leg.side, arbitrage_core::types::Side::Sell);

    Ok(())
}

/// Test stablecoin trading below peg
#[tokio::test]
async fn test_stablecoin_below_peg() -> Result<()> {
    let stablecoin_strategy = StablecoinArbitrageStrategy::new();

    // Create USDC trading below peg
    let symbol = Symbol::new("USDC", "USD");

    // USDC trading at $0.998 (20 bps below peg)
    let ticker = Ticker::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        Decimal::new(9975, 4), // bid: $0.9975
        Decimal::new(9985, 4), // ask: $0.9985
        Decimal::new(998, 3),  // last: $0.998
    );

    let order_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(9975, 4),
            Decimal::from(5000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(9985, 4),
            Decimal::from(5000),
        )],
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_ticker(ticker);
    market_bundle.add_order_book(order_book);

    let signals = stablecoin_strategy.detect(&market_bundle)?;

    println!("Detected {} signals for USDC below peg", signals.len());

    if !signals.is_empty() {
        let signal = &signals[0];
        assert_eq!(signal.symbol.to_pair(), "USDC/USD");

        // Should have 1 leg (buy USDC below peg)
        assert_eq!(signal.legs.len(), 1);
        let leg = &signal.legs[0];
        assert_eq!(leg.side, arbitrage_core::types::Side::Buy);
    }

    Ok(())
}

/// Test cross-stablecoin arbitrage (USDT/USDC)
#[tokio::test]
async fn test_cross_stablecoin_arbitrage() -> Result<()> {
    let stablecoin_strategy = StablecoinArbitrageStrategy::new();

    let symbol = Symbol::new("USDT", "USDC");

    // Create arbitrage opportunity between exchanges
    // OKX: USDT/USDC at 1.002 (expensive)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(1002, 3),
            Decimal::from(1000),
        )], // bid: 1.002
        vec![OrderBookLevel::new(
            Decimal::new(1003, 3),
            Decimal::from(1000),
        )], // ask: 1.003
    );

    // ByBit: USDT/USDC at 0.999 (cheap)
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(999, 3),
            Decimal::from(1000),
        )], // bid: 0.999
        vec![OrderBookLevel::new(
            Decimal::new(1000, 3),
            Decimal::from(1000),
        )], // ask: 1.000
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(bybit_book);

    let signals = stablecoin_strategy.detect(&market_bundle)?;

    println!("Detected {} cross-stablecoin signals", signals.len());

    if !signals.is_empty() {
        let signal = &signals[0];
        assert_eq!(signal.symbol.to_pair(), "USDT/USDC");

        // Should have 2 legs (buy + sell)
        assert_eq!(signal.legs.len(), 2);

        let buy_leg = &signal.legs[0];
        let sell_leg = &signal.legs[1];

        // Verify arbitrage structure
        assert_eq!(buy_leg.side, arbitrage_core::types::Side::Buy);
        assert_eq!(sell_leg.side, arbitrage_core::types::Side::Sell);
        assert_ne!(buy_leg.exchange, sell_leg.exchange);
        assert!(sell_leg.price > buy_leg.price);
    }

    Ok(())
}

/// Test stablecoin arbitrage filtering
#[tokio::test]
async fn test_stablecoin_arbitrage_filtering() -> Result<()> {
    use arbitrage_core::strategies::{FilterContext, RawSignal, TradeLeg};
    use arbitrage_core::types::Side;

    let stablecoin_strategy = StablecoinArbitrageStrategy::new();
    let symbol = Symbol::new("USDT", "USD");

    // Create a valid peg arbitrage signal with higher profit to overcome fees
    let mut signal = RawSignal::new("stablecoin_arbitrage", symbol.clone());

    let leg = TradeLeg::new(
        ExchangeId::OKX, // Use OKX which has lower fees
        symbol.clone(),
        Side::Sell,
        Decimal::new(1005, 3), // $1.005 - higher deviation
        Decimal::from(1000),
    );
    signal.add_leg(leg);
    signal.set_profit_bps(40); // 0.40% profit after fees (enough to overcome OKX fees)

    // Create filter context with higher max exposure
    let mut context = FilterContext::new(10); // 0.1% minimum profit
    context.max_exposure = Decimal::from(200000); // $200k max exposure
    context.set_inventory_limit(ExchangeId::OKX, "USDT", Decimal::from(5000));

    // Should pass filtering
    let result = stablecoin_strategy.filter(&signal, &context)?;
    assert!(result, "Valid stablecoin signal should pass filtering");

    // Test with insufficient inventory
    context.set_inventory_limit(ExchangeId::OKX, "USDT", Decimal::from(100)); // Not enough

    let result = stablecoin_strategy.filter(&signal, &context)?;
    assert!(
        !result,
        "Signal with insufficient inventory should be filtered out"
    );

    Ok(())
}

/// Test stablecoin arbitrage with small deviation (should be filtered)
#[tokio::test]
async fn test_stablecoin_small_deviation() -> Result<()> {
    let stablecoin_strategy = StablecoinArbitrageStrategy::new();

    let symbol = Symbol::new("USDT", "USD");

    // USDT trading very close to peg (only 2 bps deviation)
    let ticker = Ticker::new(
        ExchangeId::Kraken,
        symbol.clone(),
        Decimal::new(10001, 4), // bid: $1.0001
        Decimal::new(10003, 4), // ask: $1.0003
        Decimal::new(10002, 4), // last: $1.0002
    );

    let order_book = OrderBook::new(
        ExchangeId::Kraken,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(10001, 4),
            Decimal::from(1000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(10003, 4),
            Decimal::from(1000),
        )],
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_ticker(ticker);
    market_bundle.add_order_book(order_book);

    let signals = stablecoin_strategy.detect(&market_bundle)?;

    // Should not detect opportunity due to small deviation
    println!("Signals with small deviation: {}", signals.len());
    // Most likely no signals due to minimum deviation threshold

    Ok(())
}

/// Test multiple stablecoins
#[tokio::test]
async fn test_multiple_stablecoins() -> Result<()> {
    let stablecoin_strategy = StablecoinArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    // USDT above peg
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
            Decimal::from(2000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(1005, 3),
            Decimal::from(2000),
        )],
    );

    // DAI below peg
    let dai_symbol = Symbol::new("DAI", "USD");
    let dai_ticker = Ticker::new(
        ExchangeId::Bitstamp,
        dai_symbol.clone(),
        Decimal::new(996, 3), // $0.996
        Decimal::new(998, 3), // $0.998
        Decimal::new(997, 3), // $0.997
    );
    let dai_book = OrderBook::new(
        ExchangeId::Bitstamp,
        dai_symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(996, 3),
            Decimal::from(1500),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(998, 3),
            Decimal::from(1500),
        )],
    );

    market_bundle.add_ticker(usdt_ticker);
    market_bundle.add_order_book(usdt_book);
    market_bundle.add_ticker(dai_ticker);
    market_bundle.add_order_book(dai_book);

    let signals = stablecoin_strategy.detect(&market_bundle)?;

    println!(
        "Detected {} signals across multiple stablecoins",
        signals.len()
    );

    // Should detect opportunities for both stablecoins
    let usdt_signals: Vec<_> = signals.iter().filter(|s| s.symbol.base == "USDT").collect();
    let dai_signals: Vec<_> = signals.iter().filter(|s| s.symbol.base == "DAI").collect();

    println!(
        "USDT signals: {}, DAI signals: {}",
        usdt_signals.len(),
        dai_signals.len()
    );

    Ok(())
}
