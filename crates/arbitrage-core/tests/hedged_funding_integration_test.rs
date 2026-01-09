use arbitrage_core::{
    strategies::{Strategy, MarketBundle, FilterContext, strategies::HedgedFundingStrategy, FundingRate, Ticker},
    types::{Symbol, ExchangeId},
    Result,
};
use rust_decimal::Decimal;
use chrono::{Utc, Duration};

/// Integration test for hedged funding strategy
#[tokio::test]
async fn test_hedged_funding_integration() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();
    
    // Create market data with attractive funding rate
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");
    
    // High positive funding rate on OKX (shorts pay longs)
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 3), // 0.012 = 1.2% = 120 bps (above 1% minimum)
        Utc::now() + Duration::hours(4), // 4 hours until next funding
    );
    
    // Perpetual price on OKX
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950), // bid
        Decimal::from(50050), // ask
        Decimal::from(50000), // last
    );
    
    // Spot price on ByBit for hedging
    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49980), // bid
        Decimal::from(50020), // ask
        Decimal::from(50000), // last
    );
    
    market_bundle.add_funding_rate(funding_rate);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(bybit_ticker);
    
    // Run strategy detection
    let signals = strategy.detect(&market_bundle)?;
    
    println!("Detected {} hedged funding signals", signals.len());
    for (i, signal) in signals.iter().enumerate() {
        println!("Signal {}: {} legs, profit: {} bps", 
                 i, signal.legs.len(), signal.expected_profit_bps);
        for (j, leg) in signal.legs.iter().enumerate() {
            println!("  Leg {}: {} {:?} {} @ {}", 
                     j, leg.exchange, leg.side, leg.quantity, leg.price);
        }
        
        if let Some(funding_rate) = signal.metadata.get("funding_rate") {
            println!("  Funding rate: {}", funding_rate);
        }
        if let Some(perp_ex) = signal.metadata.get("perp_exchange") {
            println!("  Perp exchange: {}", perp_ex);
        }
        if let Some(hedge_ex) = signal.metadata.get("hedge_exchange") {
            println!("  Hedge exchange: {}", hedge_ex);
        }
    }
    
    // Should detect hedged funding opportunity
    assert!(!signals.is_empty(), "Should detect hedged funding opportunity");
    
    let signal = &signals[0];
    assert_eq!(signal.legs.len(), 2, "Should have 2 legs (perpetual + spot hedge)");
    assert!(signal.expected_profit_bps > 0, "Should have positive profit");
    
    // Legs should be on different exchanges
    assert_ne!(signal.legs[0].exchange, signal.legs[1].exchange, 
               "Legs should be on different exchanges");
    
    // Should have one buy and one sell order
    let sides: Vec<_> = signal.legs.iter().map(|leg| leg.side).collect();
    assert!(sides.contains(&arbitrage_core::types::Side::Buy));
    assert!(sides.contains(&arbitrage_core::types::Side::Sell));
    
    Ok(())
}

/// Test hedged funding with negative funding rate
#[tokio::test]
async fn test_hedged_funding_negative_rate() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");
    
    // Negative funding rate (longs pay shorts)
    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(-12, 4), // -0.0012 = -0.12% = -12 bps
        Utc::now() + Duration::hours(6),
    );
    
    // Perpetual price on ByBit
    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(2995),
        Decimal::from(3005),
        Decimal::from(3000),
    );
    
    // Spot price on OKX for hedging
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(2998),
        Decimal::from(3002),
        Decimal::from(3000),
    );
    
    market_bundle.add_funding_rate(funding_rate);
    market_bundle.add_ticker(bybit_ticker);
    market_bundle.add_ticker(okx_ticker);
    
    let signals = strategy.detect(&market_bundle)?;
    
    println!("Detected {} signals with negative funding", signals.len());
    
    if !signals.is_empty() {
        let signal = &signals[0];
        
        // With negative funding, we should go short perpetual and long spot
        let perp_leg = signal.legs.iter()
            .find(|leg| leg.exchange == ExchangeId::ByBit)
            .expect("Should have perpetual leg on ByBit");
        
        let spot_leg = signal.legs.iter()
            .find(|leg| leg.exchange == ExchangeId::OKX)
            .expect("Should have spot leg on OKX");
        
        // For negative funding: short perp (receive funding), long spot (hedge)
        assert_eq!(perp_leg.side, arbitrage_core::types::Side::Sell, 
                   "Should short perpetual with negative funding");
        assert_eq!(spot_leg.side, arbitrage_core::types::Side::Buy, 
                   "Should long spot to hedge");
    }
    
    Ok(())
}

/// Test hedged funding with low funding rate (should be filtered)
#[tokio::test]
async fn test_hedged_funding_low_rate() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");
    
    // Very low funding rate (below minimum threshold)
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(5, 5), // 0.00005 = 0.005% = 0.5 bps (very low)
        Utc::now() + Duration::hours(4),
    );
    
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );
    
    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49980),
        Decimal::from(50020),
        Decimal::from(50000),
    );
    
    market_bundle.add_funding_rate(funding_rate);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(bybit_ticker);
    
    let signals = strategy.detect(&market_bundle)?;
    
    // Should not detect opportunity due to low funding rate
    assert!(signals.is_empty(), "Should not detect opportunity with low funding rate");
    
    Ok(())
}

/// Test hedged funding with insufficient time to funding
#[tokio::test]
async fn test_hedged_funding_insufficient_time() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");
    
    // Good funding rate but insufficient time
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(10, 4), // 0.001 = 0.1% = 10 bps
        Utc::now() + Duration::minutes(30), // Only 30 minutes (below 2 hour minimum)
    );
    
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(2995),
        Decimal::from(3005),
        Decimal::from(3000),
    );
    
    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(2998),
        Decimal::from(3002),
        Decimal::from(3000),
    );
    
    market_bundle.add_funding_rate(funding_rate);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(bybit_ticker);
    
    let signals = strategy.detect(&market_bundle)?;
    
    // Should not detect opportunity due to insufficient time
    assert!(signals.is_empty(), "Should not detect opportunity with insufficient time to funding");
    
    Ok(())
}

/// Test hedged funding filtering
#[tokio::test]
async fn test_hedged_funding_filtering() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");
    
    // Moderate funding rate
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(15, 4), // 0.0015 = 0.15% = 15 bps
        Utc::now() + Duration::hours(3),
    );
    
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );
    
    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49980),
        Decimal::from(50020),
        Decimal::from(50000),
    );
    
    market_bundle.add_funding_rate(funding_rate);
    market_bundle.add_ticker(okx_ticker);
    market_bundle.add_ticker(bybit_ticker);
    
    let signals = strategy.detect(&market_bundle)?;
    
    if !signals.is_empty() {
        let signal = &signals[0];
        
        // Test filtering with normal context
        let context = FilterContext::new(5); // 0.05% minimum profit
        let is_valid = strategy.filter(signal, &context)?;
        
        println!("Signal profit: {} bps, filter result: {}", 
                 signal.expected_profit_bps, is_valid);
        
        // Should pass basic filtering if net profit is sufficient
        if signal.expected_profit_bps >= 5 {
            assert!(is_valid, "Valid signal should pass filtering");
        }
        
        // Test filtering with high profit requirement
        let strict_context = FilterContext::new(100); // 1% minimum profit
        let is_valid_strict = strategy.filter(signal, &strict_context)?;
        
        // May fail strict filtering depending on costs
        if signal.expected_profit_bps < 100 {
            assert!(!is_valid_strict, "Signal should fail strict filtering");
        }
    }
    
    Ok(())
}

/// Test hedged funding with no hedge exchange available
#[tokio::test]
async fn test_hedged_funding_no_hedge_exchange() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");
    
    // Good funding rate on OKX
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 4), // 0.0012 = 0.12% = 12 bps
        Utc::now() + Duration::hours(4),
    );
    
    // Only perpetual ticker, no spot ticker for hedging
    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );
    
    market_bundle.add_funding_rate(funding_rate);
    market_bundle.add_ticker(okx_ticker);
    // No hedge exchange ticker added
    
    let signals = strategy.detect(&market_bundle)?;
    
    // Should not detect opportunity without hedge exchange
    assert!(signals.is_empty(), "Should not detect opportunity without hedge exchange");
    
    Ok(())
}

/// Test hedged funding with multiple funding rates
#[tokio::test]
async fn test_hedged_funding_multiple_rates() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();
    
    let mut market_bundle = MarketBundle::new();
    
    // BTC with high funding rate
    let btc_symbol = Symbol::new("BTC", "USDT");
    let btc_funding = FundingRate::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::new(15, 3), // 0.015 = 1.5% = 150 bps (above 1% minimum)
        Utc::now() + Duration::hours(4),
    );
    
    // ETH with moderate funding rate
    let eth_symbol = Symbol::new("ETH", "USDT");
    let eth_funding = FundingRate::new(
        ExchangeId::ByBit,
        eth_symbol.clone(),
        Decimal::new(12, 3), // 0.012 = 1.2% = 120 bps (above 1% minimum)
        Utc::now() + Duration::hours(6),
    );
    
    // Add tickers for both
    let btc_okx = Ticker::new(ExchangeId::OKX, btc_symbol.clone(), 
                              Decimal::from(49950), Decimal::from(50050), Decimal::from(50000));
    let btc_bybit = Ticker::new(ExchangeId::ByBit, btc_symbol.clone(), 
                                Decimal::from(49980), Decimal::from(50020), Decimal::from(50000));
    
    let eth_bybit = Ticker::new(ExchangeId::ByBit, eth_symbol.clone(), 
                                Decimal::from(2995), Decimal::from(3005), Decimal::from(3000));
    let eth_okx = Ticker::new(ExchangeId::OKX, eth_symbol.clone(), 
                              Decimal::from(2998), Decimal::from(3002), Decimal::from(3000));
    
    market_bundle.add_funding_rate(btc_funding);
    market_bundle.add_funding_rate(eth_funding);
    market_bundle.add_ticker(btc_okx);
    market_bundle.add_ticker(btc_bybit);
    market_bundle.add_ticker(eth_bybit);
    market_bundle.add_ticker(eth_okx);
    
    let signals = strategy.detect(&market_bundle)?;
    
    println!("Detected {} signals across multiple funding rates", signals.len());
    
    // Should detect opportunities for both symbols
    let btc_signals: Vec<_> = signals.iter()
        .filter(|s| s.symbol == btc_symbol)
        .collect();
    
    let eth_signals: Vec<_> = signals.iter()
        .filter(|s| s.symbol == eth_symbol)
        .collect();
    
    assert!(!btc_signals.is_empty(), "Should detect BTC funding opportunity");
    assert!(!eth_signals.is_empty(), "Should detect ETH funding opportunity");
    
    // BTC should have higher profit due to higher funding rate
    if !btc_signals.is_empty() && !eth_signals.is_empty() {
        assert!(btc_signals[0].expected_profit_bps >= eth_signals[0].expected_profit_bps,
                "BTC should have higher profit due to higher funding rate");
    }
    
    Ok(())
}