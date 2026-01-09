use arbitrage_core::{
    strategies::{Strategy, strategies::CrossExchangeArbitrageStrategy, MarketBundle, FilterContext, RawSignal, TradeLeg},
    types::{Symbol, ExchangeId, OrderBook, OrderBookLevel, Side},
    Result,
};
use rust_decimal::Decimal;
use tokio;

/// Integration test for cross-exchange arbitrage
#[tokio::test]
async fn test_cross_exchange_arbitrage_integration() -> Result<()> {
    let cross_exchange_strategy = CrossExchangeArbitrageStrategy::new();
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // Create price difference between exchanges
    // OKX: BTC at $50,100 (expensive)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50050), Decimal::from(1))], // bid: $50,050
        vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))], // ask: $50,100
    );
    
    // ByBit: BTC at $49,900 (cheap)
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49900), Decimal::from(1))], // bid: $49,900
        vec![OrderBookLevel::new(Decimal::from(49950), Decimal::from(1))], // ask: $49,950
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(bybit_book);
    
    let signals = cross_exchange_strategy.detect(&market_bundle)?;
    
    // Should detect arbitrage opportunity
    assert!(!signals.is_empty(), "Should detect cross-exchange arbitrage");
    
    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
    assert!(signal.expected_profit_bps > 0);
    
    // Should have 2 legs (buy + sell)
    assert_eq!(signal.legs.len(), 2);
    
    let buy_leg = signal.legs.iter().find(|leg| leg.side == Side::Buy).unwrap();
    let sell_leg = signal.legs.iter().find(|leg| leg.side == Side::Sell).unwrap();
    
    // Verify arbitrage structure
    assert_ne!(buy_leg.exchange, sell_leg.exchange);
    assert!(sell_leg.price > buy_leg.price);
    
    Ok(())
}

/// Test cross-exchange arbitrage filtering
#[tokio::test]
async fn test_cross_exchange_arbitrage_filtering() -> Result<()> {
    let cross_exchange_strategy = CrossExchangeArbitrageStrategy::new();
    let symbol = Symbol::new("ETH", "USDT");
    
    // Create a valid cross-exchange signal
    let mut signal = RawSignal::new("cross_exchange_arbitrage", symbol.clone());
    
    let buy_leg = TradeLeg::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Side::Buy,
        Decimal::from(3000), // $3,000
        Decimal::from(1),
    );
    signal.add_leg(buy_leg);
    
    let sell_leg = TradeLeg::new(
        ExchangeId::OKX,
        symbol.clone(),
        Side::Sell,
        Decimal::from(3030), // $3,030
        Decimal::from(1),
    );
    signal.add_leg(sell_leg);
    signal.set_profit_bps(80); // 0.80% profit
    
    // Create filter context
    let mut context = FilterContext::new(50); // 0.5% minimum profit
    context.max_exposure = Decimal::from(10000); // $10k max exposure
    context.set_inventory_limit(ExchangeId::OKX, "ETH", Decimal::from(5));
    context.set_inventory_limit(ExchangeId::ByBit, "USDT", Decimal::from(5000));
    
    // Should pass filtering
    let result = cross_exchange_strategy.filter(&signal, &context)?;
    assert!(result, "Valid cross-exchange signal should pass filtering");
    
    // Test with insufficient inventory
    context.set_inventory_limit(ExchangeId::OKX, "ETH", Decimal::from(0)); // No ETH to sell
    
    let result = cross_exchange_strategy.filter(&signal, &context)?;
    assert!(!result, "Signal with insufficient inventory should be filtered out");
    
    Ok(())
}

/// Test cross-exchange arbitrage with no opportunity
#[tokio::test]
async fn test_cross_exchange_arbitrage_no_opportunity() -> Result<()> {
    let cross_exchange_strategy = CrossExchangeArbitrageStrategy::new();
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // Create similar prices (no arbitrage opportunity)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );
    
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(bybit_book);
    
    let signals = cross_exchange_strategy.detect(&market_bundle)?;
    
    // Should not detect opportunity due to small spread
    assert!(signals.is_empty() || signals[0].expected_profit_bps < 10, 
           "Should not detect opportunity with small spread");
    
    Ok(())
}

/// Test cross-exchange arbitrage with multiple symbols
#[tokio::test]
async fn test_cross_exchange_arbitrage_multiple_symbols() -> Result<()> {
    let cross_exchange_strategy = CrossExchangeArbitrageStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    
    // BTC arbitrage opportunity
    let btc_symbol = Symbol::new("BTC", "USDT");
    let btc_okx = OrderBook::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50050), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
    );
    let btc_bybit = OrderBook::new(
        ExchangeId::ByBit,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49900), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49950), Decimal::from(1))],
    );
    
    // ETH arbitrage opportunity
    let eth_symbol = Symbol::new("ETH", "USDT");
    let eth_okx = OrderBook::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3020), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3030), Decimal::from(5))],
    );
    let eth_mexc = OrderBook::new(
        ExchangeId::MEXC,
        eth_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2980), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(2990), Decimal::from(5))],
    );
    
    market_bundle.add_order_book(btc_okx);
    market_bundle.add_order_book(btc_bybit);
    market_bundle.add_order_book(eth_okx);
    market_bundle.add_order_book(eth_mexc);
    
    let signals = cross_exchange_strategy.detect(&market_bundle)?;
    
    // Should detect opportunities for both symbols
    assert!(!signals.is_empty(), "Should detect multiple arbitrage opportunities");
    
    let btc_signals: Vec<_> = signals.iter()
        .filter(|s| s.symbol.base == "BTC")
        .collect();
    let eth_signals: Vec<_> = signals.iter()
        .filter(|s| s.symbol.base == "ETH")
        .collect();
    
    assert!(!btc_signals.is_empty(), "Should detect BTC arbitrage");
    assert!(!eth_signals.is_empty(), "Should detect ETH arbitrage");
    
    Ok(())
}