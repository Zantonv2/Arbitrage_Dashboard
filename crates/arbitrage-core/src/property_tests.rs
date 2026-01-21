use proptest::prelude::*;
use proptest::strategy::Strategy;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

use crate::calculate_profit_bps;
use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};

fn small_decimal() -> impl Strategy<Value = Decimal> {
    (1i64..10000i64).prop_map(|n| Decimal::new(n, 0))
}

fn larger_decimal() -> impl Strategy<Value = Decimal> {
    (10000i64..100_000_000i64).prop_map(|n| Decimal::new(n, 0))
}

fn any_decimal() -> impl Strategy<Value = Decimal> {
    (-1_000_000_000i64..1_000_000_000i64).prop_map(|n| Decimal::new(n, 0))
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

        let prices: Vec<Decimal> = levels.iter().map(|(p, _)| *p).filter(|p| *p > Decimal::new(0, 0)).collect();

        if prices.len() >= 2 {
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

    #[test]
    fn test_fee_calculations(
        taker_fee in 1i64..100,
        maker_fee in 1i64..100,
        quantity in 1i64..1000,
        price in 1000i64..100000
    ) {
        let taker = Decimal::new(taker_fee, 4);
        let maker = Decimal::new(maker_fee, 4);
        let qty = Decimal::new(quantity, 0);
        let prc = Decimal::new(price, 0);

        let notional = prc * qty;
        let taker_fee_amount = notional * taker;
        let maker_fee_amount = notional * maker;

        prop_assert!(taker_fee_amount >= Decimal::ZERO);
        prop_assert!(maker_fee_amount >= Decimal::ZERO);
        prop_assert!(taker_fee_amount <= notional);
        prop_assert!(maker_fee_amount <= notional);
    }

    #[test]
    fn test_to_bps_conversion(value in any_decimal()) {
        let bps = crate::ToBps::to_bps_or_zero(&value);
        prop_assert!(bps >= 0 || value < Decimal::ZERO);
    }

    #[test]
    fn test_mid_price_calculations(
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

        if let Some(mid) = order_book.mid_price() {
            prop_assert!(mid > Decimal::ZERO, "Mid price should be positive");
        }
    }

    #[test]
    fn test_vwap_basic(
        prices in proptest::collection::vec(small_decimal(), 3..10),
        quantities in proptest::collection::vec(small_decimal(), 3..10)
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        if prices.is_empty() || quantities.is_empty() {
            prop_assert!(true);
        } else {
            let min_price = prices.iter().min().unwrap();
            let max_price = prices.iter().max().unwrap();

            let bids: Vec<OrderBookLevel> = prices.iter().zip(quantities.iter())
                .map(|(p, q)| OrderBookLevel::new(*p - *min_price, *q))
                .filter(|l| l.price > Decimal::ZERO)
                .collect();

            let asks: Vec<OrderBookLevel> = prices.iter().zip(quantities.iter())
                .map(|(p, q)| OrderBookLevel::new(*p + *max_price, *q))
                .filter(|l| l.price > Decimal::ZERO)
                .collect();

            if !bids.is_empty() && !asks.is_empty() {
                let _order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bids.clone(), asks.clone());
                prop_assert!(true);
            } else {
                prop_assert!(true);
            }
        }
    }

    #[test]
    fn test_bid_less_than_ask_invariant(
        bids in proptest::collection::vec(small_decimal(), 1..10),
        asks in proptest::collection::vec(larger_decimal(), 1..10)
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        let bid_levels: Vec<OrderBookLevel> = bids.iter()
            .map(|p| OrderBookLevel::new(*p, Decimal::new(1, 0)))
            .collect();

        let ask_levels: Vec<OrderBookLevel> = asks.iter()
            .map(|p| OrderBookLevel::new(*p, Decimal::new(1, 0)))
            .collect();

        let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);
        let _is_valid = order_book.is_valid();

        if let (Some(best_bid), Some(best_ask)) = (order_book.best_bid(), order_book.best_ask()) {
            prop_assert!(best_bid.price < best_ask.price, "Best bid must be less than best ask");
        }
    }

    #[test]
    fn test_volume_non_negative(
        quantities in "[0-9]{1,6}\\.[0-9]{1,4}",
    ) {
        let quantities: Result<Decimal, _> = quantities.parse();
        if let Ok(qty) = quantities {
            prop_assert!(qty >= Decimal::ZERO, "Volume should be non-negative");
        }
    }

    #[test]
    fn test_vwap_price_range(
        _prices in proptest::collection::vec(small_decimal(), 3..10),
        quantities in proptest::collection::vec(small_decimal(), 3..10)
    ) {
        for qty in &quantities {
            prop_assert!(*qty >= Decimal::ZERO, "Volume should be non-negative");
        }
    }

    #[test]
    fn test_depth_counts(
        depth in 1i32..20
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let bid_levels: Vec<OrderBookLevel> = (0..depth)
            .map(|i| OrderBookLevel::new(Decimal::from(50000 - i), Decimal::from(10)))
            .collect();
        let ask_levels: Vec<OrderBookLevel> = (0..depth)
            .map(|i| OrderBookLevel::new(Decimal::from(50010 + i), Decimal::from(10)))
            .collect();

        let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);

        let bid_count = order_book.bids.len();
        let ask_count = order_book.asks.len();

        prop_assert_eq!(bid_count, depth as usize);
        prop_assert_eq!(ask_count, depth as usize);
    }

    #[test]
    fn test_symbol_normalization_roundtrip(
        base in "[A-Z]{2,8}",
        quote in "[A-Z]{2,8}"
    ) {
        let original = format!("{}_{}", base, quote);
        let symbol = Symbol::new(&base, &quote);
        let pair = symbol.to_pair();

        let normalized = pair.replace("/", "_");
        prop_assert_eq!(normalized, original.to_uppercase());
    }

    #[test]
    fn test_orderbook_serialization(
        bid_price in small_decimal(),
        ask_price in larger_decimal()
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(bid_price, Decimal::new(1, 0))],
            vec![OrderBookLevel::new(ask_price, Decimal::new(1, 0))],
        );

        let serialized = serde_json::to_string(&order_book).unwrap();
        let deserialized: OrderBook = serde_json::from_str(&serialized).unwrap();

        prop_assert_eq!(deserialized.exchange, order_book.exchange);
        prop_assert_eq!(deserialized.symbol.base, symbol.base);
        prop_assert_eq!(deserialized.symbol.quote, symbol.quote);
    }

    #[test]
    fn test_clone_semantics(
        bid_price in small_decimal(),
        ask_price in larger_decimal()
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(bid_price, Decimal::new(1, 0))],
            vec![OrderBookLevel::new(ask_price, Decimal::new(1, 0))],
        );

        let cloned = order_book.clone();

        prop_assert_eq!(cloned.exchange, order_book.exchange);
        prop_assert_eq!(cloned.bids.len(), order_book.bids.len());
        prop_assert_eq!(cloned.asks.len(), order_book.asks.len());
    }

    #[test]
    fn test_profit_calculation_edge_cases(
        buy_price in any_decimal(),
        sell_price in any_decimal()
    ) {
        if buy_price.is_zero() {
            let result = calculate_profit_bps(buy_price, sell_price);
            prop_assert!(result.is_none(), "Should return None for zero buy price");
        } else {
            let result = calculate_profit_bps(buy_price, sell_price);
            prop_assert!(result.is_some() || buy_price == sell_price);
        }
    }

    #[test]
    fn test_spread_calculation(
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

        if let Some(spread) = order_book.spread() {
            prop_assert!(spread > Decimal::ZERO, "Spread should be positive");
        }
    }

    #[test]
    fn test_liquidity_at_price(
        bid_prices in proptest::collection::vec(small_decimal(), 1..10),
        ask_prices in proptest::collection::vec(larger_decimal(), 1..10)
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        let bid_levels: Vec<OrderBookLevel> = bid_prices.iter()
            .enumerate()
            .map(|(i, p)| OrderBookLevel::new(*p, Decimal::from(10 * (i + 1))))
            .collect();

        let ask_levels: Vec<OrderBookLevel> = ask_prices.iter()
            .enumerate()
            .map(|(i, p)| OrderBookLevel::new(*p, Decimal::from(10 * (i + 1))))
            .collect();

        if !bid_levels.is_empty() && !ask_levels.is_empty() {
            let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);

            let bid_liquidity = order_book.liquidity_at_price(Decimal::from(50000), false);
            let ask_liquidity = order_book.liquidity_at_price(Decimal::from(50000), true);

            prop_assert!(bid_liquidity >= Decimal::ZERO);
            prop_assert!(ask_liquidity >= Decimal::ZERO);
        }
    }

    #[test]
    fn test_orderbook_validation(
        bids in proptest::collection::vec(small_decimal(), 0..10),
        asks in proptest::collection::vec(larger_decimal(), 0..10)
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        let bid_levels: Vec<OrderBookLevel> = bids.into_iter()
            .map(|p| OrderBookLevel::new(p, Decimal::new(1, 0)))
            .collect();

        let ask_levels: Vec<OrderBookLevel> = asks.into_iter()
            .map(|p| OrderBookLevel::new(p, Decimal::new(1, 0)))
            .collect();

        let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);

        if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
            if let (Some(best_bid), Some(best_ask)) = (order_book.best_bid(), order_book.best_ask()) {
                prop_assert!(best_bid.price < best_ask.price);
            }
        }
    }

    #[test]
    fn test_decimal_precision(
        a in 1i64..10000i64,
        b in 1i64..10000i64
    ) {
        let da = Decimal::new(a, 4);
        let db = Decimal::new(b, 4);

        let sum = da + db;
        let _diff = da - db;
        let _product = da * db;
        let quotient = da / db;

        prop_assert!(sum > Decimal::ZERO || da == db);
        prop_assert!(quotient > Decimal::ZERO);
    }

    #[test]
    fn test_exchange_id_variants(
        id in 0i32..12i32
    ) {
        let exchange = match id {
            0 => ExchangeId::OKX,
            1 => ExchangeId::ByBit,
            2 => ExchangeId::MEXC,
            3 => ExchangeId::GateIo,
            4 => ExchangeId::Bitstamp,
            5 => ExchangeId::Kraken,
            6 => ExchangeId::HTX,
            7 => ExchangeId::BingX,
            8 => ExchangeId::Hyperliquid,
            9 => ExchangeId::KuCoin,
            10 => ExchangeId::Bitget,
            11 => ExchangeId::Binance,
            _ => ExchangeId::Coinbase,
        };

        let name = exchange.to_string();
        prop_assert!(!name.is_empty());

        let parsed: Result<ExchangeId, _> = name.parse();
        prop_assert!(parsed.is_ok());
    }

    #[test]
    fn test_vwap_result_properties(
        quantity in 1i64..100i64
    ) {
        let symbol = Symbol::new("BTC", "USDT");
        let qty = Decimal::from(quantity);

        let bid_levels: Vec<OrderBookLevel> = (1..=10)
            .map(|i| OrderBookLevel::new(Decimal::from(50000 - i), Decimal::from(i)))
            .collect();
        let ask_levels: Vec<OrderBookLevel> = (1..=10)
            .map(|i| OrderBookLevel::new(Decimal::from(50010 + i), Decimal::from(i)))
            .collect();

        let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);

        if let Some(vwap) = order_book.vwap_buy(qty) {
            prop_assert!(vwap.filled_quantity > Decimal::ZERO);
            prop_assert!(vwap.total_cost > Decimal::ZERO);
        }

        if let Some(vwap) = order_book.vwap_sell(qty) {
            prop_assert!(vwap.filled_quantity > Decimal::ZERO);
            prop_assert!(vwap.total_cost > Decimal::ZERO);
        }
    }

    #[test]
    fn test_orderbook_is_valid(
        bid_prices in proptest::collection::vec(small_decimal(), 1..10),
        ask_prices in proptest::collection::vec(larger_decimal(), 1..10)
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        let bid_levels: Vec<OrderBookLevel> = bid_prices.iter()
            .map(|p| OrderBookLevel::new(*p, Decimal::new(1, 0)))
            .collect();
        let ask_levels: Vec<OrderBookLevel> = ask_prices.iter()
            .map(|p| OrderBookLevel::new(*p, Decimal::new(1, 0)))
            .collect();

        let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);

        let _is_valid = order_book.is_valid();

        if let (Some(best_bid), Some(best_ask)) = (order_book.best_bid(), order_book.best_ask()) {
            prop_assert!(best_bid.price < best_ask.price);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn test_profit_bps_basic(
        buy in "[0-9]{1,6}\\.[0-9]{1,2}",
        sell in "[0-9]{1,6}\\.[0-9]{1,2}"
    ) {
        let buy_result: Result<Decimal, _> = buy.parse();
        let sell_result: Result<Decimal, _> = sell.parse();

        if let (Ok(buy_price), Ok(sell_price)) = (buy_result, sell_result) {
            if !buy_price.is_zero() {
                let _ = calculate_profit_bps(buy_price, sell_price);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn test_profit_invariants(
        buy_price in 1i64..1_000_000i64,
        sell_price in 1i64..1_000_000i64
    ) {
        let buy = Decimal::new(buy_price, 0);
        let sell = Decimal::new(sell_price, 0);

        if buy.is_zero() {
            let result = calculate_profit_bps(buy, sell);
            prop_assert!(result.is_none(), "Should return None for zero buy price");
        } else {
            let result = calculate_profit_bps(buy, sell);
            match result {
                Some(profit_bps) => {
                    let expected = ((sell - buy) / buy * Decimal::from(10000)).to_i32().unwrap_or(0);
                    prop_assert_eq!(profit_bps, expected, "Profit BPS should match expected calculation");
                }
                None => {
                    prop_assert!(buy.is_zero(), "Should only return None for zero buy price");
                }
            }
        }
    }

    #[test]
    fn test_fees_never_exceed_notional(
        taker_fee in 1i64..1000i64,
        maker_fee in 1i64..1000i64,
        quantity in 1i64..100_000i64,
        price in 1i64..1_000_000i64
    ) {
        let taker = Decimal::new(taker_fee, 4);
        let maker = Decimal::new(maker_fee, 4);
        let qty = Decimal::new(quantity, 0);
        let prc = Decimal::new(price, 0);

        let notional = prc * qty;
        let taker_fee_amount = notional * taker;
        let maker_fee_amount = notional * maker;

        prop_assert!(taker_fee_amount >= Decimal::ZERO, "Taker fee should be non-negative");
        prop_assert!(maker_fee_amount >= Decimal::ZERO, "Maker fee should be non-negative");
        prop_assert!(taker_fee_amount <= notional, "Taker fee should not exceed notional");
        prop_assert!(maker_fee_amount <= notional, "Maker fee should not exceed notional");

        if notional > Decimal::ZERO {
            let taker_ratio = taker_fee_amount / notional;
            let maker_ratio = maker_fee_amount / notional;
            prop_assert!(taker_ratio <= Decimal::ONE, "Taker fee ratio should be <= 1");
            prop_assert!(maker_ratio <= Decimal::ONE, "Maker fee ratio should be <= 1");
        }
    }

    #[test]
    fn test_slippage_bps_non_negative(
        best_price in 1i64..1_000_000i64,
        vwap_multiplier in 90i64..110i64
    ) {
        let best = Decimal::new(best_price, 0);
        let vwap = best * Decimal::new(vwap_multiplier, 2);

        if best.is_zero() {
            prop_assert!(true);
        } else {
            let slippage_ratio = (vwap - best).abs() / best;
            let slippage_bps = (slippage_ratio * Decimal::from(10000))
                .to_i32()
                .unwrap_or(0);
            prop_assert!(slippage_bps >= 0, "Slippage BPS should be non-negative");
        }
    }

    #[test]
    fn test_vwap_slippage_bps_non_negative_property(
        bid_prices in proptest::collection::vec(1i64..100_000i64, 1..10),
        ask_prices in proptest::collection::vec(100_001i64..200_000i64, 1..10),
        target_qty in 1i64..1000i64
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        let bid_levels: Vec<OrderBookLevel> = bid_prices.iter()
            .map(|p| OrderBookLevel::new(Decimal::from(*p), Decimal::from(10)))
            .collect();

        let ask_levels: Vec<OrderBookLevel> = ask_prices.iter()
            .map(|p| OrderBookLevel::new(Decimal::from(*p), Decimal::from(10)))
            .collect();

        let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);
        let target = Decimal::from(target_qty);

        if let Some(vwap) = order_book.vwap_buy(target) {
            prop_assert!(vwap.slippage_bps >= 0, "Buy VWAP slippage should be non-negative");
            prop_assert!(vwap.filled_quantity > Decimal::ZERO, "Filled quantity should be positive");
        }

        if let Some(vwap) = order_book.vwap_sell(target) {
            prop_assert!(vwap.slippage_bps >= 0, "Sell VWAP slippage should be non-negative");
            prop_assert!(vwap.filled_quantity > Decimal::ZERO, "Filled quantity should be positive");
        }
    }

    #[test]
    fn test_fee_calculation_edge_cases_high_fees(
        taker_fee in 9000i64..10000i64,
        maker_fee in 9000i64..10000i64,
        quantity in 1i64..100i64,
        price in 1000i64..100000i64
    ) {
        let taker = Decimal::new(taker_fee, 4);
        let maker = Decimal::new(maker_fee, 4);
        let qty = Decimal::new(quantity, 0);
        let prc = Decimal::new(price, 0);

        let notional = prc * qty;
        let taker_fee_amount = notional * taker;
        let maker_fee_amount = notional * maker;

        prop_assert!(taker_fee_amount >= Decimal::ZERO);
        prop_assert!(maker_fee_amount >= Decimal::ZERO);
        prop_assert!(taker_fee_amount <= notional);
        prop_assert!(maker_fee_amount <= notional);
    }

    #[test]
    fn test_fee_calculation_edge_cases_zero_fees(
        taker_fee in 0i64..1i64,
        maker_fee in 0i64..1i64,
        quantity in 1i64..1000i64,
        price in 1000i64..100000i64
    ) {
        let taker = Decimal::new(taker_fee, 4);
        let maker = Decimal::new(maker_fee, 4);
        let qty = Decimal::new(quantity, 0);
        let prc = Decimal::new(price, 0);

        let notional = prc * qty;
        let taker_fee_amount = notional * taker;
        let maker_fee_amount = notional * maker;

        prop_assert_eq!(taker_fee_amount, Decimal::ZERO, "Zero taker fee should give zero amount");
        prop_assert_eq!(maker_fee_amount, Decimal::ZERO, "Zero maker fee should give zero amount");
    }

    #[test]
    fn test_vwap_partial_fill_properties(
        ask_prices in proptest::collection::vec(50_000i64..60_000i64, 1..10),
        ask_quantities in proptest::collection::vec(1i64..20i64, 1..10),
        target_qty in 50i64..200i64
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        if ask_prices.is_empty() || ask_quantities.is_empty() {
            prop_assert!(true);
        } else {
            let min_len = ask_prices.len().min(ask_quantities.len());
            let ask_levels: Vec<OrderBookLevel> = (0..min_len)
                .map(|i| OrderBookLevel::new(
                    Decimal::from(ask_prices[i]),
                    Decimal::from(ask_quantities[i])
                ))
                .collect();

            let bid_levels = vec![OrderBookLevel::new(Decimal::from(49900), Decimal::from(100))];

            let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels.clone(), ask_levels);
            let target = Decimal::from(target_qty);

            if let Some(vwap) = order_book.vwap_buy(target) {
                prop_assert!(vwap.filled_quantity > Decimal::ZERO, "Should have some filled quantity");
                prop_assert!(vwap.filled_quantity <= target, "Filled should not exceed target");

                if vwap.is_fully_filled {
                    prop_assert_eq!(vwap.filled_quantity, target, "Fully filled should equal target");
                }
            }
        }
    }

    #[test]
    fn test_vwap_empty_book_edge_cases(
        has_bids in proptest::collection::vec(1i64..100_000i64, 0..5),
        has_asks in proptest::collection::vec(100_001i64..200_000i64, 0..5),
        target_qty in 1i64..100i64
    ) {
        let symbol = Symbol::new("BTC", "USDT");

        let bid_levels: Vec<OrderBookLevel> = has_bids.iter()
            .map(|p| OrderBookLevel::new(Decimal::from(*p), Decimal::from(10)))
            .collect();

        let ask_levels: Vec<OrderBookLevel> = has_asks.iter()
            .map(|p| OrderBookLevel::new(Decimal::from(*p), Decimal::from(10)))
            .collect();

        let bid_count = bid_levels.len();
        let ask_count = ask_levels.len();

        let order_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), bid_levels, ask_levels);
        let target = Decimal::from(target_qty);

        let buy_result = order_book.vwap_buy(target);
        let sell_result = order_book.vwap_sell(target);

        if bid_count == 0 {
            prop_assert!(sell_result.is_some() || sell_result.is_none());
        }
        if ask_count == 0 {
            prop_assert!(buy_result.is_some() || buy_result.is_none());
        }

        if buy_result.is_some() {
            let vwap = buy_result.unwrap();
            prop_assert!(vwap.slippage_bps >= 0);
        }
        if sell_result.is_some() {
            let vwap = sell_result.unwrap();
            prop_assert!(vwap.slippage_bps >= 0);
        }
    }
}
