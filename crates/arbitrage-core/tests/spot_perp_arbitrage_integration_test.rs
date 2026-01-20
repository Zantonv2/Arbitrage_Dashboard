use arbitrage_core::{
    strategies::{
        FilterContext, FundingRate, MarketBundle, RawSignal, SpotPerpArbitrageStrategy, Strategy,
        TradeLeg,
    },
    types::{ExchangeId, OrderBook, OrderBookLevel, Side, Symbol},
    Result,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio;

#[tokio::test]
async fn test_spot_perp_arbitrage_integration() -> Result<()> {
    let mut spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let mut config = spot_perp_strategy.config().clone();
    config.min_profit_bps = 10;
    spot_perp_strategy.update_config(config)?;

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50195), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50205), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(1, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    assert!(!signals.is_empty(), "Should detect spot-perp arbitrage");

    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
    assert!(signal.expected_profit_bps > 0);

    assert_eq!(signal.legs.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_filtering() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();
    let symbol = Symbol::new("ETH", "USDT");

    let mut signal = RawSignal::new("spot_perp_arbitrage", symbol.clone());

    let spot_leg = TradeLeg::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        Side::Buy,
        Decimal::from(3000),
        Decimal::from(1),
    );
    signal.add_leg(spot_leg);

    let perp_leg = TradeLeg::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Side::Sell,
        Decimal::from(3060),
        Decimal::from(1),
    );
    signal.add_leg(perp_leg);
    signal.set_profit_bps(150);

    let mut context = FilterContext::new(50);
    context.max_exposure = Decimal::from(10000);
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

    let result = spot_perp_strategy.filter(&signal, &context)?;
    assert!(result, "Valid spot-perp signal should pass filtering");

    signal.set_profit_bps(30);

    let result = spot_perp_strategy.filter(&signal, &context)?;
    assert!(
        !result,
        "Signal with insufficient profit should be filtered out"
    );

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_no_opportunity() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

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
        Decimal::new(1, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty() || signals[0].expected_profit_bps < 10,
        "Should not detect opportunity with small basis"
    );

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_negative_funding() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("ETH", "USDT");

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

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(-5, 4),
        Utc::now() + Duration::hours(4),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        let spot_leg = signal
            .legs
            .iter()
            .find(|leg| leg.exchange == ExchangeId::Bitstamp)
            .unwrap();
        let perp_leg = signal
            .legs
            .iter()
            .find(|leg| leg.exchange == ExchangeId::ByBit)
            .unwrap();

        if spot_leg.side == Side::Sell {
            assert_eq!(perp_leg.side, Side::Buy);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_contango() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50220), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(3, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];
        assert!(
            signal.expected_profit_bps > 0,
            "Contango should produce positive profit"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_backwardation() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50220), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(-3, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];
        assert!(
            signal.expected_profit_bps > 0,
            "Backwardation with negative funding should produce profit"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_eth_symbol() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("ETH", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2995), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(10))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3020), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(3030), Decimal::from(10))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(2, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    for signal in &signals {
        assert_eq!(
            signal.symbol.base, "ETH",
            "ETH symbol should be correctly detected"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_leg_directions() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50220), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(3, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        let sides: Vec<_> = signal.legs.iter().map(|leg| leg.side).collect();
        assert!(sides.contains(&Side::Buy), "Should have a buy leg");
        assert!(sides.contains(&Side::Sell), "Should have a sell leg");
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_exchange_pairs() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    for spot_exchange in [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Bitstamp,
        ExchangeId::Kraken,
    ] {
        for perp_exchange in [ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC] {
            if spot_exchange == perp_exchange {
                continue;
            }

            let symbol = Symbol::new("BTC", "USDT");

            let spot_book = OrderBook::new(
                spot_exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
                vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
            );

            let perp_book = OrderBook::new(
                perp_exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
                vec![OrderBookLevel::new(Decimal::from(50220), Decimal::from(1))],
            );

            let funding_rate = FundingRate::new(
                perp_exchange,
                symbol.clone(),
                Decimal::new(3, 4),
                Utc::now() + Duration::hours(8),
            );

            let mut market_bundle = MarketBundle::new();
            market_bundle.add_order_book(Arc::new(spot_book));
            market_bundle.add_order_book(Arc::new(perp_book));
            market_bundle.add_funding_rate(Arc::new(funding_rate));

            let signals = spot_perp_strategy.detect(&market_bundle)?;

            for signal in &signals {
                assert!(
                    signal.legs.len() == 2,
                    "Should have 2 legs for {} spot and {} perp",
                    spot_exchange,
                    perp_exchange
                );
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_high_funding() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50120), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(10, 3),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];
        assert!(
            signal.expected_profit_bps > 100,
            "High funding should produce high profit"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_expired_funding() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50220), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(3, 4),
        Utc::now() - Duration::hours(1),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    println!("Expired funding test: {} signals detected", signals.len());

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_no_funding_data() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50220), Decimal::from(1))],
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    println!("No funding data test: {} signals detected", signals.len());

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_minimal_basis() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50002), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50003), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(1, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty() || signals[0].expected_profit_bps < 10,
        "Should not detect opportunity with minimal basis"
    );

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_large_basis() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49100), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(51000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(51100), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(5, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];
        assert!(
            signal.expected_profit_bps > 100,
            "Large basis should produce large profit"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_spot_perp_arbitrage_with_funding_mixed() -> Result<()> {
    let spot_perp_strategy = SpotPerpArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let spot_book = OrderBook::new(
        ExchangeId::Bitstamp,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let perp_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50050), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50070), Decimal::from(1))],
    );

    let funding_rate = FundingRate::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::new(-2, 4),
        Utc::now() + Duration::hours(8),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(spot_book));
    market_bundle.add_order_book(Arc::new(perp_book));
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let signals = spot_perp_strategy.detect(&market_bundle)?;

    for signal in &signals {
        assert!(
            signal.expected_profit_bps > 0,
            "Signal should have positive profit"
        );
    }

    Ok(())
}
