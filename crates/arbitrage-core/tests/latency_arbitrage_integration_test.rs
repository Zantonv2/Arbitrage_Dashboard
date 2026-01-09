use arbitrage_core::{
    strategies::{Strategy, strategies::LatencyArbitrageStrategy, MarketBundle, FilterContext, RawSignal, TradeLeg, Ticker},
    types::{Symbol, ExchangeId, OrderBook, OrderBookLevel, Side},
    Result,
};
use rust_decimal::Decimal;
use tokio;

/// Integration test for latency arbitrage
#[tokio::test]
async fn test_latency_arbitrage_integration() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // Create stale price on slow exchange vs fresh price on fast exchange
    // OKX (fast): BTC at $50,100
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50095), Decimal::from(1))], // bid: $50,095
        vec![OrderBookLevel::new(Decimal::from(50105), Decimal::from(1))], // ask: $50,105
    );
    
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(50095),
        Decimal::from(50105),
        Decimal::from(50100),
    );
    
    // Gate.io (slower): BTC still at old price $49,800 (larger gap)
    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))], // bid: $49,795
        vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))], // ask: $49,805
    );
    
    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(49795),
        Decimal::from(49805),
        Decimal::from(49800), // $49,800 vs $50,100 = 0.60% difference
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(gateio_book);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(gateio_ticker);
    
    let signals = latency_strategy.detect(&market_bundle)?;
    
    // Should detect latency arbitrage opportunity
    assert!(!signals.is_empty(), "Should detect latency arbitrage");
    
    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
    assert!(signal.expected_profit_bps > 0);
    
    // Should have 2 legs (buy stale, sell fresh)
    assert_eq!(signal.legs.len(), 2);
    
    let buy_leg = signal.legs.iter().find(|leg| leg.side == Side::Buy).unwrap();
    let sell_leg = signal.legs.iter().find(|leg| leg.side == Side::Sell).unwrap();
    
    // Buy from slower exchange, sell to faster exchange
    assert_eq!(buy_leg.exchange, ExchangeId::GateIo);
    assert_eq!(sell_leg.exchange, ExchangeId::OKX);
    assert!(sell_leg.price > buy_leg.price);
    
    Ok(())
}

/// Test latency arbitrage filtering
#[tokio::test]
async fn test_latency_arbitrage_filtering() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();
    let symbol = Symbol::new("ETH", "USDT");
    
    // Create a valid latency arbitrage signal
    let mut signal = RawSignal::new("latency_arbitrage", symbol.clone());
    
    let buy_leg = TradeLeg::new(
        ExchangeId::GateIo, // Slower exchange
        symbol.clone(),
        Side::Buy,
        Decimal::from(2990), // Buy at stale price
        Decimal::from(2),
    );
    signal.add_leg(buy_leg);
    
    let sell_leg = TradeLeg::new(
        ExchangeId::OKX, // Faster exchange
        symbol.clone(),
        Side::Sell,
        Decimal::from(3030), // Sell at current price
        Decimal::from(2),
    );
    signal.add_leg(sell_leg);
    signal.set_profit_bps(120); // 1.20% profit
    
    // Create filter context with tight latency requirements
    let mut context = FilterContext::new(50); // 0.5% minimum profit
    context.max_exposure = Decimal::from(15000); // $15k max exposure
    context.max_latency_ms = 100; // 100ms max latency
    context.set_inventory_limit(ExchangeId::OKX, "ETH", Decimal::from(5));
    context.set_inventory_limit(ExchangeId::GateIo, "USDT", Decimal::from(8000));
    
    // Should pass filtering
    let result = latency_strategy.filter(&signal, &context)?;
    assert!(result, "Valid latency arbitrage signal should pass filtering");
    
    // Test with insufficient profit margin
    signal.set_profit_bps(30); // Below minimum
    
    let result = latency_strategy.filter(&signal, &context)?;
    assert!(!result, "Signal with insufficient profit should be filtered out");
    
    Ok(())
}

/// Test latency arbitrage with no price staleness
#[tokio::test]
async fn test_latency_arbitrage_no_opportunity() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // All exchanges have similar, current prices
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    );
    
    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
    );
    
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49995),
        Decimal::from(50005),
        Decimal::from(50000),
    );
    
    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(49990),
        Decimal::from(50000),
        Decimal::from(49995),
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(gateio_book);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(gateio_ticker);
    
    let signals = latency_strategy.detect(&market_bundle)?;
    
    // Should not detect opportunity due to synchronized prices
    assert!(signals.is_empty() || signals[0].expected_profit_bps < 10, 
           "Should not detect opportunity with synchronized prices");
    
    Ok(())
}

/// Test latency arbitrage with multiple symbols
#[tokio::test]
async fn test_latency_arbitrage_multiple_symbols() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    
    // BTC latency opportunity - larger spread
    let btc_symbol = Symbol::new("BTC", "USDT");
    let btc_okx = OrderBook::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50095), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50105), Decimal::from(1))],
    );
    let btc_gateio = OrderBook::new(
        ExchangeId::GateIo,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))], // Larger gap
        vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))],
    );
    
    // ETH latency opportunity - larger spread
    let eth_symbol = Symbol::new("ETH", "USDT");
    let eth_okx = OrderBook::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3025), Decimal::from(3))],
        vec![OrderBookLevel::new(Decimal::from(3035), Decimal::from(3))],
    );
    let eth_mexc = OrderBook::new(
        ExchangeId::MEXC,
        eth_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2955), Decimal::from(3))], // Larger gap
        vec![OrderBookLevel::new(Decimal::from(2965), Decimal::from(3))],
    );
    
    market_bundle.add_order_book(btc_okx);
    market_bundle.add_order_book(btc_gateio);
    market_bundle.add_order_book(eth_okx);
    market_bundle.add_order_book(eth_mexc);
    
    // Add tickers for price calculation
    let btc_okx_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::from(50095),
        Decimal::from(50105),
        Decimal::from(50100),
    );
    let btc_gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        btc_symbol.clone(),
        Decimal::from(49795),
        Decimal::from(49805),
        Decimal::from(49800),
    );
    let eth_okx_ticker = Ticker::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        Decimal::from(3025),
        Decimal::from(3035),
        Decimal::from(3030),
    );
    let eth_mexc_ticker = Ticker::new(
        ExchangeId::MEXC,
        eth_symbol.clone(),
        Decimal::from(2955),
        Decimal::from(2965),
        Decimal::from(2960),
    );
    
    market_bundle.add_ticker(btc_okx_ticker);
    market_bundle.add_ticker(btc_gateio_ticker);
    market_bundle.add_ticker(eth_okx_ticker);
    market_bundle.add_ticker(eth_mexc_ticker);
    
    let signals = latency_strategy.detect(&market_bundle)?;
    
    // Should detect opportunities for both symbols
    assert!(!signals.is_empty(), "Should detect multiple latency arbitrage opportunities");
    
    let btc_signals: Vec<_> = signals.iter()
        .filter(|s| s.symbol.base == "BTC")
        .collect();
    let eth_signals: Vec<_> = signals.iter()
        .filter(|s| s.symbol.base == "ETH")
        .collect();
    
    assert!(!btc_signals.is_empty(), "Should detect BTC latency arbitrage");
    assert!(!eth_signals.is_empty(), "Should detect ETH latency arbitrage");
    
    Ok(())
}