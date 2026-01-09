use arbitrage_core::{
    strategies::{Strategy, strategies::NewListingArbitrageStrategy, MarketBundle, FilterContext, RawSignal, TradeLeg, Ticker},
    types::{Symbol, ExchangeId, OrderBook, OrderBookLevel, Side},
    Result,
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tokio;

/// Integration test for new listing arbitrage
#[tokio::test]
async fn test_new_listing_arbitrage_integration() -> Result<()> {
    let new_listing_strategy = NewListingArbitrageStrategy::new();
    
    // New token "NEWCOIN" listed on different exchanges at different times
    let symbol = Symbol::new("NEWCOIN", "USDT");
    
    // OKX: Early listing with high price due to FOMO
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::new(95, 2), Decimal::from(1000))], // bid: $0.95
        vec![OrderBookLevel::new(Decimal::new(105, 2), Decimal::from(1000))], // ask: $1.05
    );
    
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(95, 2),
        Decimal::new(105, 2),
        Decimal::ONE, // last: $1.00
    );
    
    // ByBit: Later listing with more rational pricing
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::new(75, 2), Decimal::from(2000))], // bid: $0.75
        vec![OrderBookLevel::new(Decimal::new(85, 2), Decimal::from(2000))], // ask: $0.85
    );
    
    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(75, 2),
        Decimal::new(85, 2),
        Decimal::new(80, 2), // last: $0.80
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(bybit_book);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(bybit_ticker);
    
    let signals = new_listing_strategy.detect(&market_bundle)?;
    
    // Should detect new listing arbitrage opportunity
    assert!(!signals.is_empty(), "Should detect new listing arbitrage");
    
    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "NEWCOIN/USDT");
    assert!(signal.expected_profit_bps > 0);
    
    // Should have 2 legs (buy cheap, sell expensive)
    assert_eq!(signal.legs.len(), 2);
    
    let buy_leg = signal.legs.iter().find(|leg| leg.side == Side::Buy).unwrap();
    let sell_leg = signal.legs.iter().find(|leg| leg.side == Side::Sell).unwrap();
    
    // Buy from ByBit (cheaper), sell on OKX (expensive)
    assert_eq!(buy_leg.exchange, ExchangeId::ByBit);
    assert_eq!(sell_leg.exchange, ExchangeId::OKX);
    assert!(sell_leg.price > buy_leg.price);
    
    Ok(())
}

/// Test new listing arbitrage filtering
#[tokio::test]
async fn test_new_listing_arbitrage_filtering() -> Result<()> {
    let new_listing_strategy = NewListingArbitrageStrategy::new();
    let symbol = Symbol::new("NEWTOKEN", "USDT");
    
    // Create a valid new listing arbitrage signal
    let mut signal = RawSignal::new("new_listing_arbitrage", symbol.clone());
    
    let buy_leg = TradeLeg::new(
        ExchangeId::MEXC, // Cheaper exchange
        symbol.clone(),
        Side::Buy,
        Decimal::new(50, 2), // Buy at $0.50
        Decimal::from(1000),
    );
    signal.add_leg(buy_leg);
    
    let sell_leg = TradeLeg::new(
        ExchangeId::OKX, // More expensive exchange
        symbol.clone(),
        Side::Sell,
        Decimal::new(75, 2), // Sell at $0.75
        Decimal::from(1000),
    );
    signal.add_leg(sell_leg);
    signal.set_profit_bps(800); // 8% profit (reasonable for new listings)
    
    // Create filter context with appropriate settings for new listings
    let mut context = FilterContext::new(50); // 0.5% minimum profit (lower for new listings)
    context.max_exposure = Decimal::from(20000); // $20k max exposure (will be reduced to 25% = $5k for new listings)
    context.set_inventory_limit(ExchangeId::OKX, "NEWTOKEN", Decimal::from(2000));
    context.set_inventory_limit(ExchangeId::MEXC, "USDT", Decimal::from(1000));
    
    // Should pass filtering (new symbols are considered newly listed by default)
    let result = new_listing_strategy.filter(&signal, &context)?;
    assert!(result, "Valid new listing signal should pass filtering");
    
    // Test with insufficient profit (even for new listings)
    signal.set_profit_bps(25); // 0.25% - below minimum for new listings
    
    let result = new_listing_strategy.filter(&signal, &context)?;
    assert!(!result, "Signal with insufficient profit should be filtered out");
    
    Ok(())
}

/// Test new listing arbitrage with established token (no opportunity)
#[tokio::test]
async fn test_new_listing_arbitrage_established_token() -> Result<()> {
    let new_listing_strategy = NewListingArbitrageStrategy::new();
    
    // Established token with efficient pricing
    let symbol = Symbol::new("BTC", "USDT");
    
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    );
    
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
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
    
    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49990),
        Decimal::from(50000),
        Decimal::from(49995),
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(bybit_book);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(bybit_ticker);
    
    let signals = new_listing_strategy.detect(&market_bundle)?;
    
    // Should not detect significant opportunity for established token
    assert!(signals.is_empty() || signals[0].expected_profit_bps < 100, 
           "Should not detect large opportunity for established token");
    
    Ok(())
}

/// Test new listing arbitrage with high volatility
#[tokio::test]
async fn test_new_listing_arbitrage_high_volatility() -> Result<()> {
    let new_listing_strategy = NewListingArbitrageStrategy::new();
    
    // Highly volatile new token with extreme price differences
    let symbol = Symbol::new("VOLATILE", "USDT");
    
    // Exchange 1: Very high price (early FOMO)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(5), Decimal::from(100))], // bid: $5.00
        vec![OrderBookLevel::new(Decimal::from(6), Decimal::from(100))], // ask: $6.00
    );
    
    // Exchange 2: Much lower price (rational pricing)
    let mexc_book = OrderBook::new(
        ExchangeId::MEXC,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2), Decimal::from(500))], // bid: $2.00
        vec![OrderBookLevel::new(Decimal::from(3), Decimal::from(500))], // ask: $3.00
    );
    
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(5),
        Decimal::from(6),
        Decimal::new(55, 1), // last: $5.50
    );
    
    let mexc_ticker = Ticker::new(
        ExchangeId::MEXC,
        symbol.clone(),
        Decimal::from(2),
        Decimal::from(3),
        Decimal::new(25, 1), // last: $2.50
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(okx_book);
    market_bundle.add_order_book(mexc_book);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(mexc_ticker);
    
    let signals = new_listing_strategy.detect(&market_bundle)?;
    
    if !signals.is_empty() {
        let signal = &signals[0];
        
        // Should detect massive arbitrage opportunity
        assert!(signal.expected_profit_bps > 5000, "Should detect large profit opportunity");
        
        let buy_leg = signal.legs.iter().find(|leg| leg.side == Side::Buy).unwrap();
        let sell_leg = signal.legs.iter().find(|leg| leg.side == Side::Sell).unwrap();
        
        // Buy from MEXC (cheaper), sell on OKX (expensive)
        assert_eq!(buy_leg.exchange, ExchangeId::MEXC);
        assert_eq!(sell_leg.exchange, ExchangeId::OKX);
        
        // Verify significant price difference
        let price_diff_pct = ((sell_leg.price - buy_leg.price) / buy_leg.price * Decimal::from(100))
            .to_f64().unwrap_or(0.0);
        assert!(price_diff_pct > 50.0, "Should have >50% price difference");
    }
    
    Ok(())
}