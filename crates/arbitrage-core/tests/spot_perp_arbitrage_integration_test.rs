use arbitrage_core::{
    strategies::{Strategy, strategies::SpotPerpArbitrageStrategy, MarketBundle, FilterContext, RawSignal, TradeLeg, FundingRate},
    types::{Symbol, ExchangeId, OrderBook, OrderBookLevel, Side},
    Result,
};
use rust_decimal::Decimal;
use chrono::{Utc, Duration};
use tokio;

/// Integration test for spot-perpetual arbitrage
#[tokio::test]
async fn test_spot_perp_arbitrage_integration() -> Result<()> {
    let mut spot_perp_strategy = SpotPerpArbitrageStrategy::new();
    
    // Override the min_profit_bps to be lower for testing
    let mut config = spot_perp_strategy.config().clone();
    config.min_profit_bps = 10; // 0.1% minimum profit
    spot_perp_strategy.update_config(config)?;
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // Create basis spread between spot and perpetual
    // Spot: BTC at $50,000
    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp, // Spot exchange
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(1))], // bid: $49,995
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))], // ask: $50,005
    );
    
    // Perpetual: BTC at $50,200 (premium)
    let perp_book = OrderBook::new(
        ExchangeId::ByBit, // Perpetual exchange
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50195), Decimal::from(1))], // bid: $50,195
        vec![OrderBookLevel::new(Decimal::from(50205), Decimal::from(1))], // ask: $50,205
    );
    
    // Add funding rate for perpetual
    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(1, 4), // 0.01% funding rate
        Utc::now() + Duration::hours(8), // Next funding in 8 hours
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(spot_book);
    market_bundle.add_order_book(perp_book);
    market_bundle.add_funding_rate(funding_rate);
    
    let signals = spot_perp_strategy.detect(&market_bundle)?;
    
    // Should detect basis arbitrage opportunity
    assert!(!signals.is_empty(), "Should detect spot-perp arbitrage");
    
    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
    assert!(signal.expected_profit_bps > 0);
    
    // Should have 2 legs (spot + perpetual)
    assert_eq!(signal.legs.len(), 2);
    
    Ok(())
}

/// Test spot-perpetual arbitrage filtering
#[tokio::test]
async fn test_spot_perp_arbitrage_filtering() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();
    let symbol = Symbol::new("ETH", "USDT");
    
    // Create a valid spot-perp signal
    let mut signal = RawSignal::new("spot_perp_arbitrage", symbol.clone());
    
    let spot_leg = TradeLeg::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        Side::Buy,
        Decimal::from(3000), // Buy spot at $3,000
        Decimal::from(1),
    );
    signal.add_leg(spot_leg);
    
    let perp_leg = TradeLeg::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Side::Sell,
        Decimal::from(3060), // Sell perp at $3,060
        Decimal::from(1),
    );
    signal.add_leg(perp_leg);
    signal.set_profit_bps(150); // 1.50% profit
    
    // Create filter context
    let mut context = FilterContext::new(50); // 0.5% minimum profit
    context.max_exposure = Decimal::from(10000); // $10k max exposure
    context.allowed_exchanges = vec![
        ExchangeId::Bitstamp,
        ExchangeId::ByBit,
        ExchangeId::OKX,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Kraken,
    ];
    context.set_inventory_limit(ExchangeId::Bitstamp, "USDT", Decimal::from(5000));
    context.set_inventory_limit(ExchangeId::ByBit, "ETH", Decimal::from(5));
    
    // Should pass filtering
    let result = spot_perp_strategy.filter(&signal, &context)?;
    assert!(result, "Valid spot-perp signal should pass filtering");
    
    // Test with insufficient profit
    signal.set_profit_bps(30); // Below minimum
    
    let result = spot_perp_strategy.filter(&signal, &context)?;
    assert!(!result, "Signal with insufficient profit should be filtered out");
    
    Ok(())
}

/// Test spot-perpetual arbitrage with no basis spread
#[tokio::test]
async fn test_spot_perp_arbitrage_no_opportunity() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // Create similar prices (no basis spread)
    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    );
    
    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
    );
    
    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(1, 4), // 0.01% funding rate
        Utc::now() + Duration::hours(8),
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(spot_book);
    market_bundle.add_order_book(perp_book);
    market_bundle.add_funding_rate(funding_rate);
    
    let signals = spot_perp_strategy.detect(&market_bundle)?;
    
    // Should not detect opportunity due to small basis
    assert!(signals.is_empty() || signals[0].expected_profit_bps < 10, 
           "Should not detect opportunity with small basis");
    
    Ok(())
}

/// Test spot-perpetual arbitrage with negative funding
#[tokio::test]
async fn test_spot_perp_arbitrage_negative_funding() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();
    
    let symbol = Symbol::new("ETH", "USDT");
    
    // Spot higher than perpetual (backwardation)
    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3095), Decimal::from(2))],
        vec![OrderBookLevel::new(Decimal::from(3105), Decimal::from(2))],
    );
    
    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2995), Decimal::from(2))],
        vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(2))],
    );
    
    // Negative funding rate (shorts pay longs)
    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(-5, 4), // -0.05% funding rate
        Utc::now() + Duration::hours(4),
    );
    
    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(spot_book);
    market_bundle.add_order_book(perp_book);
    market_bundle.add_funding_rate(funding_rate);
    
    let signals = spot_perp_strategy.detect(&market_bundle)?;
    
    if !signals.is_empty() {
        let signal = &signals[0];
        
        // In backwardation with negative funding, we might:
        // - Sell spot, buy perpetual
        // - Collect negative funding (get paid to be long perp)
        let spot_leg = signal.legs.iter().find(|leg| leg.exchange == ExchangeId::Bitstamp).unwrap();
        let perp_leg = signal.legs.iter().find(|leg| leg.exchange == ExchangeId::ByBit).unwrap();
        
        // Verify the trade direction makes sense
        if spot_leg.side == Side::Sell {
            assert_eq!(perp_leg.side, Side::Buy);
        }
    }
    
    Ok(())
}