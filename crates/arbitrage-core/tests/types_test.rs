use arbitrage_core::test_utils::fixtures::{invalid_btc_order_book, valid_btc_order_book};
use arbitrage_core::types::*;
use chrono::Utc;
use rust_decimal::Decimal;

#[test]
fn test_symbol_creation() {
    let symbol = Symbol::new("BTC", "USDT");
    assert_eq!(symbol.base, "BTC");
    assert_eq!(symbol.quote, "USDT");
    assert_eq!(symbol.to_pair(), "BTC/USDT");
}

#[test]
fn test_symbol_from_pair() {
    let symbol = Symbol::from_pair("ETH/USDC").unwrap();
    assert_eq!(symbol.base, "ETH");
    assert_eq!(symbol.quote, "USDC");
}

#[test]
fn test_symbol_from_pair_invalid() {
    assert!(Symbol::from_pair("INVALID").is_none());
    assert!(Symbol::from_pair("BTC/USDT/EXTRA").is_none());
}

#[test]
fn test_order_book_validity() {
    let valid_book = valid_btc_order_book();
    assert!(valid_book.is_valid());

    let invalid_book = invalid_btc_order_book();
    assert!(!invalid_book.is_valid());
}

#[test]
fn test_order_book_best_prices() {
    let book = valid_btc_order_book();

    assert_eq!(book.best_bid().unwrap().price, Decimal::from(50000));
    assert_eq!(book.best_ask().unwrap().price, Decimal::from(50001));
    assert_eq!(book.spread().unwrap(), Decimal::from(1));
    assert_eq!(
        book.mid_price().unwrap(),
        Decimal::from_str_exact("50000.5").unwrap()
    );
}

#[test]
fn test_signal_expiry() {
    let mut signal = Signal::new(
        Symbol::new("BTC", "USDT"),
        ExchangeId::ByBit,
        ExchangeId::BingX,
        Decimal::from(50000),
        Decimal::from(50100),
        Utc::now(),
    );

    assert!(!signal.is_expired());

    // Set expiry to past
    signal.expires_at = Utc::now() - chrono::Duration::minutes(1);
    assert!(signal.is_expired());
}

#[test]
fn test_signal_age() {
    let signal = Signal::new(
        Symbol::new("BTC", "USDT"),
        ExchangeId::ByBit,
        ExchangeId::BingX,
        Decimal::from(50000),
        Decimal::from(50100),
        Utc::now(),
    );

    assert!(signal.age_seconds() >= 0);
}

#[test]
fn test_order_creation() {
    let order = Order::new(
        ExchangeId::ByBit,
        Symbol::new("BTC", "USDT"),
        Side::Buy,
        OrderType::Limit,
        Decimal::from(1),
        Some(Decimal::from(50000)),
    );

    assert_eq!(order.exchange, ExchangeId::ByBit);
    assert_eq!(order.side, Side::Buy);
    assert_eq!(order.order_type, OrderType::Limit);
    assert_eq!(order.quantity, Decimal::from(1));
    assert_eq!(order.price, Some(Decimal::from(50000)));
    assert!(order.client_order_id.starts_with("arb_"));
}

#[test]
fn test_execution_instruction_validity() {
    let signal_id = uuid::Uuid::new_v4();
    let buy_order = Order::new(
        ExchangeId::ByBit,
        Symbol::new("BTC", "USDT"),
        Side::Buy,
        OrderType::Limit,
        Decimal::from(1),
        Some(Decimal::from(50000)),
    );
    let sell_order = Order::new(
        ExchangeId::BingX,
        Symbol::new("BTC", "USDT"),
        Side::Sell,
        OrderType::Limit,
        Decimal::from(1),
        Some(Decimal::from(50100)),
    );

    let instruction = ExecutionInstruction::new(signal_id, buy_order, sell_order);
    assert!(instruction.is_valid()); // No validation errors initially
    assert_eq!(instruction.signal_id, signal_id);
}

#[test]
fn test_exchange_id_display() {
    assert_eq!(ExchangeId::ByBit.to_string(), "bybit");
    assert_eq!(ExchangeId::BingX.to_string(), "bingx");
    assert_eq!(ExchangeId::Hyperliquid.to_string(), "hyperliquid");
}

#[test]
fn test_exchange_status() {
    let status = ExchangeStatus::new(ExchangeId::ByBit);
    assert_eq!(status.exchange, ExchangeId::ByBit);
    assert!(matches!(status.status, ConnectionStatus::Disconnected));
    assert_eq!(status.error_count, 0);
    assert!(status.subscribed_symbols.is_empty());
}
