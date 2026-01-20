use arbitrage_core::test_utils::fixtures::TestFixtures;
use arbitrage_core::types::*;
use rust_decimal::Decimal;

#[test]
fn test_order_book_builder_basic() {
    let book = TestFixtures::order_book().build();
    assert_eq!(book.exchange, ExchangeId::OKX);
    assert_eq!(book.symbol.base, "BTC");
    assert_eq!(book.symbol.quote, "USDT");
}

#[test]
fn test_order_book_builder_chain() {
    let book = TestFixtures::order_book()
        .exchange(ExchangeId::ByBit)
        .symbol("ETH", "USDT")
        .spread(Decimal::from(3000), Decimal::from(3010), Decimal::from(2))
        .depth(3)
        .build();

    assert_eq!(book.exchange, ExchangeId::ByBit);
    assert_eq!(book.symbol.base, "ETH");
    assert_eq!(book.symbol.quote, "USDT");
    assert_eq!(book.bids.len(), 3);
    assert_eq!(book.asks.len(), 3);
}

#[test]
fn test_ticker_builder_chain() {
    let ticker = TestFixtures::ticker()
        .exchange(ExchangeId::MEXC)
        .symbol("SOL", "USDT")
        .last_price(Decimal::from(100))
        .bid(Decimal::from(99))
        .ask(Decimal::from(101))
        .build();

    assert_eq!(ticker.exchange, ExchangeId::MEXC);
    assert_eq!(ticker.symbol.base, "SOL");
    assert_eq!(ticker.bid, Decimal::from(99));
    assert_eq!(ticker.ask, Decimal::from(101));
}

#[test]
fn test_signal_builder_chain() {
    let signal = TestFixtures::signal()
        .symbol("BTC", "USDT")
        .exchange(
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50050),
        )
        .profit_bps(10)
        .build();

    assert_eq!(signal.symbol.base, "BTC");
    assert_eq!(signal.legs.len(), 2);
    assert_eq!(signal.expected_profit_bps, 10);
}

#[test]
fn test_market_bundle_builder_chain() {
    let bundle = TestFixtures::market_bundle()
        .exchange(ExchangeId::OKX)
        .exchange(ExchangeId::ByBit)
        .symbol("BTC", "USDT")
        .build();

    assert!(bundle
        .get_order_book(ExchangeId::OKX, &btc_usdt_symbol())
        .is_some());
    assert!(bundle
        .get_order_book(ExchangeId::ByBit, &btc_usdt_symbol())
        .is_some());
}

#[test]
fn test_config_builder_chain() {
    let config = TestFixtures::config()
        .fee(Decimal::from_str_exact("0.05").unwrap())
        .min_size(Decimal::from(10))
        .build();

    assert_eq!(
        config.trading.slippage_buffer_percent,
        Decimal::from_str_exact("0.05").unwrap()
    );
    assert_eq!(config.risk.min_order_size_usd, Decimal::from(10));
}

#[test]
fn test_filter_context_builder_chain() {
    let context = TestFixtures::filter_context().min_profit_bps(15).build();

    assert_eq!(context.min_profit_bps, 15);
}

#[test]
fn test_arbitrary_opportunity_preset() {
    let signal = TestFixtures::arbitrary_opportunity();
    assert!(signal.is_valid());
    assert_eq!(signal.expected_profit_bps, 10);
    assert_eq!(signal.legs.len(), 2);
}

#[test]
fn test_no_opportunity_preset() {
    let signal = TestFixtures::no_opportunity();
    assert_eq!(signal.expected_profit_bps, 0);
    assert_eq!(signal.legs.len(), 2);
}

#[test]
fn test_high_volatility_preset() {
    let signal = TestFixtures::high_volatility();
    assert!(signal.is_valid());
    assert!(signal.expected_profit_bps > 100);
}

#[test]
fn test_thin_liquidity_preset() {
    let signal = TestFixtures::thin_liquidity();
    assert!(signal.is_valid());
    assert_eq!(signal.expected_profit_bps, 20);
}

#[test]
fn test_order_book_spread_calculation() {
    let book = TestFixtures::order_book()
        .spread(Decimal::from(50000), Decimal::from(50010), Decimal::from(1))
        .build();

    assert_eq!(book.spread().unwrap(), Decimal::from(10));
}

#[test]
fn test_order_book_validity() {
    let book = TestFixtures::order_book()
        .spread(Decimal::from(50000), Decimal::from(50010), Decimal::from(1))
        .build();

    assert!(book.is_valid());
}

#[test]
fn test_market_bundle_with_custom_order_book() {
    let custom_book = TestFixtures::order_book()
        .exchange(ExchangeId::GateIo)
        .symbol("ETH", "USDT")
        .spread(Decimal::from(3000), Decimal::from(3010), Decimal::from(5))
        .build();

    let bundle = TestFixtures::market_bundle()
        .order_book(custom_book)
        .build();

    assert!(bundle
        .get_order_book(ExchangeId::GateIo, &Symbol::new("ETH", "USDT"))
        .is_some());
}

fn btc_usdt_symbol() -> Symbol {
    Symbol::new("BTC", "USDT")
}
