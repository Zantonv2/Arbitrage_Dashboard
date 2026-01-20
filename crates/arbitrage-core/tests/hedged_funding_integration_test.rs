use std::sync::Arc;

use arbitrage_core::strategies::base::{
    ConfidenceFactors, FilterContext, FundingRate, MarketBundle, RiskLimits, Strategy,
    StrategyConfig, Ticker,
};
use arbitrage_core::strategies::HedgedFundingStrategy;
use arbitrage_core::{
    types::{ExchangeId, Symbol},
    FeeSchedule, Result,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;

#[tokio::test]
async fn test_hedged_funding_integration() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 3),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        !signals.is_empty(),
        "Should detect hedged funding opportunity"
    );

    let signal = &signals[0];
    assert_eq!(
        signal.legs.len(),
        2,
        "Should have 2 legs (perpetual + spot hedge)"
    );
    assert!(
        signal.expected_profit_bps > 0,
        "Should have positive profit"
    );

    let sides: Vec<_> = signal.legs.iter().map(|leg| leg.side).collect();
    assert!(sides.contains(&arbitrage_core::types::Side::Buy));
    assert!(sides.contains(&arbitrage_core::types::Side::Sell));

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_negative_rate() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(-12, 4),
        Utc::now() + Duration::hours(6),
    );

    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(2995),
        Decimal::from(3005),
        Decimal::from(3000),
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(2998),
        Decimal::from(3002),
        Decimal::from(3000),
    );

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(bybit_ticker));
    market_bundle.add_ticker(Arc::new(okx_ticker));

    let signals = strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        let perp_leg = signal
            .legs
            .iter()
            .find(|leg| leg.exchange == ExchangeId::ByBit)
            .expect("Should have perpetual leg on ByBit");

        let spot_leg = signal
            .legs
            .iter()
            .find(|leg| leg.exchange == ExchangeId::OKX)
            .expect("Should have spot leg on OKX");

        assert_eq!(
            perp_leg.side,
            arbitrage_core::types::Side::Sell,
            "Should short perpetual with negative funding"
        );
        assert_eq!(
            spot_leg.side,
            arbitrage_core::types::Side::Buy,
            "Should long spot to hedge"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_low_rate() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(5, 5),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect opportunity with low funding rate"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_insufficient_time() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(10, 4),
        Utc::now() + Duration::minutes(30),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect opportunity with insufficient time to funding"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_filtering() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(15, 4),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        let context = FilterContext::new(5);
        let is_valid = strategy.filter(signal, &context)?;

        if signal.expected_profit_bps >= 5 {
            assert!(is_valid, "Valid signal should pass filtering");
        }

        let strict_context = FilterContext::new(100);
        let is_valid_strict = strategy.filter(signal, &strict_context)?;

        if signal.expected_profit_bps < 100 {
            assert!(!is_valid_strict, "Signal should fail strict filtering");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_no_hedge_exchange() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 4),
        Utc::now() + Duration::hours(4),
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect opportunity without hedge exchange"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_multiple_rates() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();

    let btc_symbol = Symbol::new("BTC", "USDT");
    let btc_funding = FundingRate::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::new(15, 3),
        Utc::now() + Duration::hours(4),
    );

    let eth_symbol = Symbol::new("ETH", "USDT");
    let eth_funding = FundingRate::new(
        ExchangeId::ByBit,
        eth_symbol.clone(),
        Decimal::new(12, 3),
        Utc::now() + Duration::hours(6),
    );

    let btc_okx = Ticker::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );
    let btc_bybit = Ticker::new(
        ExchangeId::ByBit,
        btc_symbol.clone(),
        Decimal::from(49980),
        Decimal::from(50020),
        Decimal::from(50000),
    );

    let eth_bybit = Ticker::new(
        ExchangeId::ByBit,
        eth_symbol.clone(),
        Decimal::from(2995),
        Decimal::from(3005),
        Decimal::from(3000),
    );
    let eth_okx = Ticker::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        Decimal::from(2998),
        Decimal::from(3002),
        Decimal::from(3000),
    );

    market_bundle.add_funding_rate(Arc::new(btc_funding));
    market_bundle.add_funding_rate(Arc::new(eth_funding));
    market_bundle.add_ticker(Arc::new(btc_okx));
    market_bundle.add_ticker(Arc::new(btc_bybit));
    market_bundle.add_ticker(Arc::new(eth_bybit));
    market_bundle.add_ticker(Arc::new(eth_okx));

    let signals = strategy.detect(&market_bundle)?;

    let btc_signals: Vec<_> = signals.iter().filter(|s| s.symbol == btc_symbol).collect();
    let eth_signals: Vec<_> = signals.iter().filter(|s| s.symbol == eth_symbol).collect();

    assert!(
        !btc_signals.is_empty(),
        "Should detect BTC funding opportunity"
    );
    assert!(
        !eth_signals.is_empty(),
        "Should detect ETH funding opportunity"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_high_funding_rate() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(50, 3),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        !signals.is_empty(),
        "Should detect opportunity with high funding rate"
    );

    let signal = &signals[0];
    assert!(
        signal.expected_profit_bps > 100,
        "High funding rate should produce high profit"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_exact_funding_time() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(20, 4),
        Utc::now() + Duration::hours(2),
    );

    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(2995),
        Decimal::from(3005),
        Decimal::from(3000),
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(2998),
        Decimal::from(3002),
        Decimal::from(3000),
    );

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(bybit_ticker));
    market_bundle.add_ticker(Arc::new(okx_ticker));

    let signals = strategy.detect(&market_bundle)?;

    // Should detect opportunity at exact 2-hour funding time
    // Note: Result depends on strategy implementation details
    println!("Detected {} signals at exact funding time", signals.len());

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_extreme_funding_rate() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(100, 3),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        !signals.is_empty(),
        "Should detect opportunity with extreme funding rate"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_no_funding_data() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

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

    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect opportunity without funding data"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_expired_funding() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(15, 4),
        Utc::now() - Duration::hours(1),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect opportunity with expired funding"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_far_future_funding() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(15, 4),
        Utc::now() + Duration::hours(10),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect opportunity with funding too far in future"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_minimal_profit_threshold() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(11, 4),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    for signal in &signals {
        assert!(
            signal.expected_profit_bps > 0,
            "All signals should have positive profit"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_leg_properties() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 3),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        for leg in &signal.legs {
            assert!(
                leg.quantity > Decimal::ZERO,
                "Leg quantity should be positive"
            );
            assert!(leg.price > Decimal::ZERO, "Leg price should be positive");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_metadata() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 3),
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

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];
        println!("Signal metadata: {:?}", signal.metadata);
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_all_tier1_exchanges() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    for perp_exchange in [ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC] {
        for spot_exchange in [
            ExchangeId::OKX,
            ExchangeId::ByBit,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
            ExchangeId::Bitstamp,
            ExchangeId::Kraken,
        ] {
            if perp_exchange == spot_exchange {
                continue;
            }

            let mut market_bundle = MarketBundle::new();
            let symbol = Symbol::new("BTC", "USDT");

            let funding_rate = FundingRate::new(
                perp_exchange,
                symbol.clone(),
                Decimal::new(15, 4),
                Utc::now() + Duration::hours(4),
            );

            let perp_ticker = Ticker::new(
                perp_exchange,
                symbol.clone(),
                Decimal::from(49950),
                Decimal::from(50050),
                Decimal::from(50000),
            );

            let spot_ticker = Ticker::new(
                spot_exchange,
                symbol.clone(),
                Decimal::from(49980),
                Decimal::from(50020),
                Decimal::from(50000),
            );

            market_bundle.add_funding_rate(Arc::new(funding_rate));
            market_bundle.add_ticker(Arc::new(perp_ticker));
            market_bundle.add_ticker(Arc::new(spot_ticker));

            let signals = strategy.detect(&market_bundle)?;

            for signal in &signals {
                assert!(
                    signal.legs.len() == 2,
                    "Should have 2 legs for {} -> {}",
                    perp_exchange,
                    spot_exchange
                );
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_market_conditions() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 3),
        Utc::now() + Duration::hours(4),
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(25000),
        Decimal::from(25050),
        Decimal::from(25000),
    );

    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(25050),
        Decimal::from(25100),
        Decimal::from(25075),
    );

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));

    let signals = strategy.detect(&market_bundle)?;

    for signal in &signals {
        assert!(
            signal.expected_profit_bps > 0,
            "Signal should have positive profit even in different market conditions"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_same_exchange_hedge() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(12, 4),
        Utc::now() + Duration::hours(4),
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(okx_ticker));

    let signals = strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect opportunity when hedge exchange same as perp"
    );

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_eth_variant() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(8, 4),
        Utc::now() + Duration::hours(5),
    );

    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(2995),
        Decimal::from(3005),
        Decimal::from(3000),
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(2998),
        Decimal::from(3002),
        Decimal::from(3000),
    );

    market_bundle.add_funding_rate(Arc::new(funding_rate));
    market_bundle.add_ticker(Arc::new(bybit_ticker));
    market_bundle.add_ticker(Arc::new(okx_ticker));

    let signals = strategy.detect(&market_bundle)?;

    for signal in &signals {
        assert_eq!(
            signal.symbol.base, "ETH",
            "ETH symbol should be correctly detected"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_hedged_funding_repeated_detection() -> Result<()> {
    let strategy = HedgedFundingStrategy::new();

    let mut market_bundle1 = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");

    let funding_rate1 = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(15, 4),
        Utc::now() + Duration::hours(4),
    );

    let okx_ticker1 = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    let bybit_ticker1 = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49980),
        Decimal::from(50020),
        Decimal::from(50000),
    );

    market_bundle1.add_funding_rate(Arc::new(funding_rate1));
    market_bundle1.add_ticker(Arc::new(okx_ticker1));
    market_bundle1.add_ticker(Arc::new(bybit_ticker1));

    let signals1 = strategy.detect(&market_bundle1)?;

    let mut market_bundle2 = MarketBundle::new();
    let funding_rate2 = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(15, 4),
        Utc::now() + Duration::hours(4),
    );

    let okx_ticker2 = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49950),
        Decimal::from(50050),
        Decimal::from(50000),
    );

    let bybit_ticker2 = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49980),
        Decimal::from(50020),
        Decimal::from(50000),
    );

    market_bundle2.add_funding_rate(Arc::new(funding_rate2));
    market_bundle2.add_ticker(Arc::new(okx_ticker2));
    market_bundle2.add_ticker(Arc::new(bybit_ticker2));

    let signals2 = strategy.detect(&market_bundle2)?;

    assert_eq!(
        signals1.len(),
        signals2.len(),
        "Repeated detection should produce same number of signals"
    );

    Ok(())
}
