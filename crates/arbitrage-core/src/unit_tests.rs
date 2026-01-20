use rust_decimal::Decimal;

use crate::strategies::base::{MarketBundle, Strategy, StrategyConfig};
use crate::strategies::{
    CexArbitrageStrategy, FundingRateArbitrageStrategy, StablecoinArbitrageStrategy,
};
use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};

#[test]
fn test_strategy_config_defaults() {
    let config = StrategyConfig::default();
    assert!(config.min_profit_bps > 0);
}

#[test]
fn test_market_bundle_empty() {
    let bundle = MarketBundle::new();
    assert!(bundle.order_books.is_empty());
}

#[test]
fn test_market_bundle_add_order_book() {
    let mut bundle = MarketBundle::new();
    let symbol = Symbol::new("BTC", "USDT");
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    bundle.add_order_book(std::sync::Arc::new(order_book));
    assert_eq!(bundle.order_books.len(), 1);
}

#[test]
fn test_cex_arbitrage_strategy_new() {
    let strategy = CexArbitrageStrategy::new();
    assert!(strategy.id().contains("cex_arbitrage"));
    assert!(strategy.name().contains("Arbitrage"));
}

#[test]
fn test_funding_rate_strategy_new() {
    let strategy = FundingRateArbitrageStrategy::new();
    assert_eq!(strategy.id(), "funding_rate_arbitrage");
}

#[test]
fn test_stablecoin_strategy_new() {
    let strategy = StablecoinArbitrageStrategy::new();
    assert_eq!(strategy.id(), "stablecoin_arbitrage");
}

#[test]
fn test_order_book_best_prices() {
    let symbol = Symbol::new("BTC", "USDT");
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol,
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(2))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(3))],
    );

    assert!(order_book.best_bid().is_some());
    assert!(order_book.best_ask().is_some());
}

#[test]
fn test_order_book_spread() {
    let symbol = Symbol::new("BTC", "USDT");
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol,
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    if let Some(spread) = order_book.spread() {
        assert!(spread > Decimal::ZERO);
    }
}

#[test]
fn test_exchange_id_display() {
    assert_eq!(ExchangeId::OKX.to_string(), "okx");
    assert_eq!(ExchangeId::ByBit.to_string(), "bybit");
}

#[test]
fn test_symbol_creation() {
    let symbol = Symbol::new("BTC", "USDT");
    assert_eq!(symbol.base, "BTC");
    assert_eq!(symbol.quote, "USDT");
}

#[test]
fn test_symbol_from_pair() {
    let symbol = Symbol::from_pair("BTC_USDT");
    if let Some(s) = symbol {
        assert_eq!(s.base, "BTC");
        assert_eq!(s.quote, "USDT");
    }
}

#[test]
fn test_order_book_level_creation() {
    let level = OrderBookLevel::new(Decimal::from(50000), Decimal::from(1));
    assert_eq!(level.price, Decimal::from(50000));
}

#[test]
fn test_order_book_mid_price() {
    let symbol = Symbol::new("BTC", "USDT");
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol,
        vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    if let Some(mid) = order_book.mid_price() {
        assert_eq!(mid, Decimal::from(50000));
    }
}

#[test]
fn test_all_exchange_ids() {
    let exchanges = vec![ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC];
    for exchange in exchanges {
        assert!(!exchange.to_string().is_empty());
    }
}

#[test]
fn test_multi_order_book() {
    let symbol = Symbol::new("BTC", "USDT");
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol,
        vec![OrderBookLevel::new(Decimal::from(49980), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(2))],
    );

    assert!(order_book.best_bid().is_some());
    assert!(order_book.best_ask().is_some());
}

#[test]
fn test_strategy_id_and_name() {
    let cex = CexArbitrageStrategy::new();
    assert!(cex.id().contains("cex"));

    let stable = StablecoinArbitrageStrategy::new();
    assert!(stable.id().contains("stablecoin"));
}
