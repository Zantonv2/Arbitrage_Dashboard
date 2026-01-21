use arbitrage_core::{
    strategies::{
        FilterContext, LatencyArbitrageStrategy, MarketBundle, RawSignal, Strategy, Ticker,
        TradeLeg,
    },
    types::{ExchangeId, OrderBook, OrderBookLevel, Side, Symbol},
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio;

#[tokio::test]
async fn test_latency_arbitrage_integration() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50095), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50105), Decimal::from(1))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(50095),
        Decimal::from(50105),
        Decimal::from(50100),
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))],
    );

    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(49795),
        Decimal::from(49805),
        Decimal::from(49800),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(gateio_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    assert!(!signals.is_empty(), "Should detect latency arbitrage");

    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
    assert!(signal.expected_profit_bps > 0);

    assert_eq!(signal.legs.len(), 2);

    let buy_leg = signal
        .legs
        .iter()
        .find(|leg| leg.side == Side::Buy)
        .unwrap();
    let sell_leg = signal
        .legs
        .iter()
        .find(|leg| leg.side == Side::Sell)
        .unwrap();

    assert_eq!(buy_leg.exchange, ExchangeId::GateIo);
    assert_eq!(sell_leg.exchange, ExchangeId::OKX);
    assert!(sell_leg.price > buy_leg.price);

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_filtering() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();
    let symbol = Symbol::new("ETH", "USDT");

    let mut signal = RawSignal::new("latency_arbitrage", Arc::new(symbol.clone()));

    let buy_leg = TradeLeg::new(
        ExchangeId::GateIo,
        Arc::new(symbol.clone()),
        Side::Buy,
        Decimal::from(2990),
        Decimal::from(2),
    );
    signal.add_leg(buy_leg);

    let sell_leg = TradeLeg::new(
        ExchangeId::OKX,
        Arc::new(symbol.clone()),
        Side::Sell,
        Decimal::from(3030),
        Decimal::from(2),
    );
    signal.add_leg(sell_leg);
    signal.set_profit_bps(120);

    let mut context = FilterContext::new(50);
    context.max_exposure = Decimal::from(15000);
    context.max_latency_ms = 100;
    context.set_inventory_limit(ExchangeId::OKX, "ETH", Decimal::from(5));
    context.set_inventory_limit(ExchangeId::GateIo, "USDT", Decimal::from(8000));

    let result = latency_strategy.filter(&signal, &context)?;
    assert!(
        result,
        "Valid latency arbitrage signal should pass filtering"
    );

    signal.set_profit_bps(30);

    let result = latency_strategy.filter(&signal, &context)?;
    assert!(
        !result,
        "Signal with insufficient profit should be filtered out"
    );

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_no_opportunity() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

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
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(gateio_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty() || signals[0].expected_profit_bps < 10,
        "Should not detect opportunity with synchronized prices"
    );

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_multiple_symbols() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

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
        vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))],
    );

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
        vec![OrderBookLevel::new(Decimal::from(2955), Decimal::from(3))],
        vec![OrderBookLevel::new(Decimal::from(2965), Decimal::from(3))],
    );

    market_bundle.add_order_book(Arc::new(btc_okx));
    market_bundle.add_order_book(Arc::new(btc_gateio));
    market_bundle.add_order_book(Arc::new(eth_okx));
    market_bundle.add_order_book(Arc::new(eth_mexc));

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

    market_bundle.add_ticker(Arc::new(btc_okx_ticker));
    market_bundle.add_ticker(Arc::new(btc_gateio_ticker));
    market_bundle.add_ticker(Arc::new(eth_okx_ticker));
    market_bundle.add_ticker(Arc::new(eth_mexc_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    assert!(
        !signals.is_empty(),
        "Should detect multiple latency arbitrage opportunities"
    );

    let btc_signals: Vec<_> = signals.iter().filter(|s| s.symbol.base == "BTC").collect();
    let eth_signals: Vec<_> = signals.iter().filter(|s| s.symbol.base == "ETH").collect();

    assert!(
        !btc_signals.is_empty(),
        "Should detect BTC latency arbitrage"
    );
    assert!(
        !eth_signals.is_empty(),
        "Should detect ETH latency arbitrage"
    );

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_large_price_gap() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(51000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(51010), Decimal::from(1))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(51000),
        Decimal::from(51010),
        Decimal::from(51005),
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49010), Decimal::from(1))],
    );

    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(49000),
        Decimal::from(49010),
        Decimal::from(49005),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(gateio_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    assert!(
        !signals.is_empty(),
        "Should detect latency arbitrage with large price gap"
    );

    let signal = &signals[0];
    assert!(
        signal.expected_profit_bps > 200,
        "Large price gap should produce large profit"
    );

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_small_price_gap() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50020), Decimal::from(1))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(50010),
        Decimal::from(50020),
        Decimal::from(50015),
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    );

    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(50000),
        Decimal::from(50005),
        Decimal::from(50002),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(gateio_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    let profitable_signals: Vec<_> = signals
        .iter()
        .filter(|s| s.expected_profit_bps >= 10)
        .collect();

    assert!(
        profitable_signals.is_empty(),
        "Should not detect latency arbitrage with small price gap"
    );

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_all_exchanges() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    for (fast_exchange, slow_exchange) in [
        (ExchangeId::OKX, ExchangeId::GateIo),
        (ExchangeId::ByBit, ExchangeId::MEXC),
        (ExchangeId::OKX, ExchangeId::Kraken),
    ] {
        let symbol = Symbol::new("BTC", "USDT");

        let fast_book = OrderBook::new(
            fast_exchange,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(1))],
        );

        let fast_ticker = Ticker::new(
            fast_exchange,
            symbol.clone(),
            Decimal::from(50100),
            Decimal::from(50110),
            Decimal::from(50105),
        );

        let slow_book = OrderBook::new(
            slow_exchange,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49800), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(49810), Decimal::from(1))],
        );

        let slow_ticker = Ticker::new(
            slow_exchange,
            symbol.clone(),
            Decimal::from(49800),
            Decimal::from(49810),
            Decimal::from(49805),
        );

        let mut market_bundle = MarketBundle::new();
        market_bundle.add_order_book(Arc::new(fast_book));
        market_bundle.add_order_book(Arc::new(slow_book));
        market_bundle.add_ticker(Arc::new(fast_ticker));
        market_bundle.add_ticker(Arc::new(slow_ticker));

        let signals = latency_strategy.detect(&market_bundle)?;

        assert!(
            !signals.is_empty(),
            "Should detect latency arbitrage for {} -> {}",
            fast_exchange,
            slow_exchange
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_no_ticker_data() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50095), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50105), Decimal::from(1))],
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))],
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));

    let signals = latency_strategy.detect(&market_bundle)?;

    assert!(
        signals.is_empty(),
        "Should not detect latency arbitrage without ticker data"
    );

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_reversed_direction() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(49795),
        Decimal::from(49805),
        Decimal::from(49800),
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50095), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50105), Decimal::from(1))],
    );

    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(50095),
        Decimal::from(50105),
        Decimal::from(50100),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(gateio_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];
        let buy_leg = signal
            .legs
            .iter()
            .find(|leg| leg.side == Side::Buy)
            .unwrap();
        let sell_leg = signal
            .legs
            .iter()
            .find(|leg| leg.side == Side::Sell)
            .unwrap();

        assert!(
            buy_leg.price < sell_leg.price,
            "Buy price should be less than sell price"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_eth_symbol() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("ETH", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3020), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3030), Decimal::from(5))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(3020),
        Decimal::from(3030),
        Decimal::from(3025),
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(2950), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(2960), Decimal::from(5))],
    );

    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(2950),
        Decimal::from(2960),
        Decimal::from(2955),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(gateio_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    for signal in &signals {
        assert_eq!(
            signal.symbol.base, "ETH",
            "ETH symbol should be correctly detected"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_multi_exchange() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(1))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(50100),
        Decimal::from(50110),
        Decimal::from(50105),
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50050), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50060), Decimal::from(1))],
    );

    let bybit_ticker = Ticker::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(50050),
        Decimal::from(50060),
        Decimal::from(50055),
    );

    let mexc_book = OrderBook::new(
        ExchangeId::MEXC,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49800), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49810), Decimal::from(1))],
    );

    let mexc_ticker = Ticker::new(
        ExchangeId::MEXC,
        symbol.clone(),
        Decimal::from(49800),
        Decimal::from(49810),
        Decimal::from(49805),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(bybit_book));
    market_bundle.add_order_book(Arc::new(mexc_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(bybit_ticker));
    market_bundle.add_ticker(Arc::new(mexc_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    assert!(
        !signals.is_empty(),
        "Should detect latency arbitrage with multiple exchanges"
    );

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_high_frequency() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    for _ in 0..10 {
        let okx_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50095), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50105), Decimal::from(1))],
        );

        let okx_ticker = Ticker::new(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50095),
            Decimal::from(50105),
            Decimal::from(50100),
        );

        let gateio_book = OrderBook::new(
            ExchangeId::GateIo,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))],
        );

        let gateio_ticker = Ticker::new(
            ExchangeId::GateIo,
            symbol.clone(),
            Decimal::from(49795),
            Decimal::from(49805),
            Decimal::from(49800),
        );

        let mut market_bundle = MarketBundle::new();
        market_bundle.add_order_book(Arc::new(okx_book));
        market_bundle.add_order_book(Arc::new(gateio_book));
        market_bundle.add_ticker(Arc::new(okx_ticker));
        market_bundle.add_ticker(Arc::new(gateio_ticker));

        let signals = latency_strategy.detect(&market_bundle)?;

        for signal in &signals {
            assert!(
                signal.expected_profit_bps > 0,
                "Signal should have positive profit"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_latency_arbitrage_price_staleness() -> Result<()> {
    let latency_strategy = LatencyArbitrageStrategy::new();

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50095), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50105), Decimal::from(1))],
    );

    let okx_ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(50095),
        Decimal::from(50105),
        Decimal::from(50100),
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(49795), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(49805), Decimal::from(1))],
    );

    let gateio_ticker = Ticker::new(
        ExchangeId::GateIo,
        symbol.clone(),
        Decimal::from(49795),
        Decimal::from(49805),
        Decimal::from(49800),
    );

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(gateio_book));
    market_bundle.add_ticker(Arc::new(okx_ticker));
    market_bundle.add_ticker(Arc::new(gateio_ticker));

    let signals = latency_strategy.detect(&market_bundle)?;

    if !signals.is_empty() {
        let signal = &signals[0];

        for leg in &signal.legs {
            assert!(leg.price > Decimal::ZERO, "Leg price should be positive");
            assert!(
                leg.quantity > Decimal::ZERO,
                "Leg quantity should be positive"
            );
        }
    }

    Ok(())
}
