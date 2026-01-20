use proptest::prelude::*;
use proptest::strategy::Strategy;
use rust_decimal::Decimal;

use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};

fn small_decimal() -> impl Strategy<Value = Decimal> {
    (1i64..10000i64).prop_map(|n| Decimal::new(n, 0))
}

fn larger_decimal() -> impl Strategy<Value = Decimal> {
    (10000i64..100_000_000i64).prop_map(|n| Decimal::new(n, 0))
}

proptest! {
    #[test]
    fn test_mid_price_between_bid_ask(
        bid in small_decimal(),
        ask in larger_decimal()
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(bid, Decimal::new(1, 0))],
            vec![OrderBookLevel::new(ask, Decimal::new(1, 0))],
        );
        if let (Some(mid), Some(bid_level), Some(ask_level)) = (order_book.mid_price(), order_book.best_bid(), order_book.best_ask()) {
            prop_assert!(mid >= bid_level.price, "Mid price should be >= bid");
            prop_assert!(mid <= ask_level.price, "Mid price should be <= ask");
        }
    }

    #[test]
    fn test_symbol_normalization(base in "[A-Z]{2,10}", quote in "[A-Z]{2,10}") {
        let symbol = Symbol::new(&base, &quote);
        prop_assert_eq!(symbol.base, base);
        prop_assert_eq!(symbol.quote, quote);
    }

    #[test]
    fn test_order_book_bid_less_than_ask(
        bid_price in small_decimal(),
        ask_price in larger_decimal()
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(bid_price, Decimal::new(1, 0))],
            vec![OrderBookLevel::new(ask_price, Decimal::new(1, 0))],
        );
        if let (Some(bid), Some(ask)) = (order_book.best_bid(), order_book.best_ask()) {
            prop_assert!(bid.price < ask.price);
        }
    }

    #[test]
    fn test_order_book_spread_positive(
        bid_price in small_decimal(),
        ask_price in larger_decimal()
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(bid_price, Decimal::new(1, 0))],
            vec![OrderBookLevel::new(ask_price, Decimal::new(1, 0))],
        );
        if let Some(spread) = order_book.spread() {
            prop_assert!(spread > Decimal::ZERO);
        }
    }

    #[test]
    fn test_exchange_id_to_string(id in 0i32..6i32) {
        let exchange_id = match id {
            0 => ExchangeId::OKX,
            1 => ExchangeId::ByBit,
            2 => ExchangeId::MEXC,
            3 => ExchangeId::GateIo,
            4 => ExchangeId::Bitstamp,
            _ => ExchangeId::Kraken,
        };
        prop_assert!(!exchange_id.to_string().is_empty());
    }

    #[test]
    fn test_order_book_level_properties(
        price in small_decimal(),
        qty in small_decimal()
    ) {
        let level = OrderBookLevel::new(price, qty);
        prop_assert_eq!(level.price, price);
        prop_assert_eq!(level.quantity, qty);
    }

    #[test]
    fn test_multi_level_order_book(
        levels in proptest::collection::vec((small_decimal(), small_decimal()), 2..10)
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let mut bids = Vec::new();
        let mut asks = Vec::new();

        // Collect all prices first
        let prices: Vec<Decimal> = levels.iter().map(|(p, _)| *p).filter(|p| *p > Decimal::new(0, 0)).collect();

        if prices.len() >= 2 {
            // Use first half for bids, second half for asks
            let mid = prices.len() / 2;
            for (i, &price) in prices.iter().enumerate() {
                if i < mid {
                    bids.push(OrderBookLevel::new(price, Decimal::new(1, 0)));
                } else {
                    asks.push(OrderBookLevel::new(price + Decimal::new(10000, 0), Decimal::new(1, 0)));
                }
            }
        }

        if !bids.is_empty() && !asks.is_empty() {
            let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bids, asks);
            if let (Some(best_bid), Some(best_ask)) = (order_book.best_bid(), order_book.best_ask()) {
                prop_assert!(best_bid.price < best_ask.price);
            }
        }
    }

    #[test]
    fn test_symbol_from_pair_valid(base in "[A-Za-z0-9]+", quote in "[A-Za-z0-9]+") {
        let pair = format!("{}_{}", base, quote);
        if let Some(symbol) = Symbol::from_pair(&pair) {
            prop_assert_eq!(symbol.base, base.to_uppercase());
            prop_assert_eq!(symbol.quote, quote.to_uppercase());
        }
    }

    #[test]
    fn test_order_book_total_volume(
        bid_price in small_decimal(),
        ask_price in larger_decimal(),
        bid_qty in small_decimal(),
        ask_qty in small_decimal()
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(bid_price, bid_qty)],
            vec![OrderBookLevel::new(ask_price, ask_qty)],
        );
        let total = order_book.bids.iter().map(|l| l.quantity).sum::<Decimal>()
                 + order_book.asks.iter().map(|l| l.quantity).sum::<Decimal>();
        prop_assert!(total > Decimal::ZERO);
    }
}
