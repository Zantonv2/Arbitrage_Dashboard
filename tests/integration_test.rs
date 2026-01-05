// Integration tests for the arbitrage dashboard
// These tests will verify end-to-end functionality

use arbitrage_core::types::*;
use tokio_test;

#[tokio::test]
async fn test_basic_functionality() {
    // Basic smoke test to ensure core types work
    let symbol = Symbol::new("BTC", "USDT");
    assert_eq!(symbol.to_pair(), "BTC/USDT");
    
    let signal = Signal::new(
        symbol,
        ExchangeId::ByBit,
        ExchangeId::BingX,
        rust_decimal::Decimal::from(50000),
        rust_decimal::Decimal::from(50100),
    );
    
    assert!(!signal.is_expired());
    assert_eq!(signal.buy_exchange, ExchangeId::ByBit);
    assert_eq!(signal.sell_exchange, ExchangeId::BingX);
}

#[tokio::test]
async fn test_order_book_creation() {
    let symbol = Symbol::new("ETH", "USDT");
    let bids = vec![
        OrderBookLevel::new(rust_decimal::Decimal::from(3000), rust_decimal::Decimal::from(1)),
        OrderBookLevel::new(rust_decimal::Decimal::from(2999), rust_decimal::Decimal::from(2)),
    ];
    let asks = vec![
        OrderBookLevel::new(rust_decimal::Decimal::from(3001), rust_decimal::Decimal::from(1)),
        OrderBookLevel::new(rust_decimal::Decimal::from(3002), rust_decimal::Decimal::from(2)),
    ];
    
    let order_book = OrderBook::new(ExchangeId::ByBit, symbol, bids, asks);
    
    assert!(order_book.is_valid());
    assert_eq!(order_book.best_bid().unwrap().price, rust_decimal::Decimal::from(3000));
    assert_eq!(order_book.best_ask().unwrap().price, rust_decimal::Decimal::from(3001));
    assert_eq!(order_book.spread().unwrap(), rust_decimal::Decimal::from(1));
}