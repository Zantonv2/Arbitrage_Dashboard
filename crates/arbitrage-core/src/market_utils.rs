//! Market utility functions for arbitrage strategies

use crate::strategies::MarketBundle;
use crate::types::{exchange_constants, ExchangeId, Symbol};
use rust_decimal::Decimal;

/// Find the best bid (highest price) across multiple exchanges
pub fn find_best_bid(
    market_data: &MarketBundle,
    symbol: &Symbol,
    exchanges: &[ExchangeId],
) -> Option<(ExchangeId, Decimal)> {
    let mut best: Option<(ExchangeId, Decimal)> = None;
    for &exchange in exchanges {
        if let Some(ticker) = market_data.get_ticker(exchange, symbol) {
            if ticker.bid > Decimal::ZERO {
                if let Some((_, current)) = best {
                    if ticker.bid > current {
                        best = Some((exchange, ticker.bid));
                    }
                } else {
                    best = Some((exchange, ticker.bid));
                }
            }
        }
    }
    best
}

/// Find the best ask (lowest price) across multiple exchanges
pub fn find_best_ask(
    market_data: &MarketBundle,
    symbol: &Symbol,
    exchanges: &[ExchangeId],
) -> Option<(ExchangeId, Decimal)> {
    let mut best: Option<(ExchangeId, Decimal)> = None;
    for &exchange in exchanges {
        if let Some(ticker) = market_data.get_ticker(exchange, symbol) {
            if ticker.ask > Decimal::ZERO {
                if let Some((_, current)) = best {
                    if ticker.ask < current {
                        best = Some((exchange, ticker.ask));
                    }
                } else {
                    best = Some((exchange, ticker.ask));
                }
            }
        }
    }
    best
}

/// Get supported spot exchanges for CEX arbitrage
pub fn get_spot_exchanges() -> Vec<ExchangeId> {
    exchange_constants::SPOT_EXCHANGES.to_vec()
}

/// Get supported perpetual exchanges
pub fn get_perpetual_exchanges() -> Vec<ExchangeId> {
    exchange_constants::PERPETUAL_EXCHANGES.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::{MarketBundle, Ticker};
    use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};
    use rust_decimal::prelude::ToPrimitive;
    use rust_decimal::Decimal;
    use std::sync::Arc;

    fn create_test_ticker(
        exchange: ExchangeId,
        symbol: &Symbol,
        bid: Decimal,
        ask: Decimal,
    ) -> Ticker {
        Ticker::new(
            exchange,
            symbol.clone(),
            bid,
            ask,
            (bid + ask) / Decimal::from(2),
        )
    }

    fn create_test_order_book(
        exchange: ExchangeId,
        symbol: &Symbol,
        bids: Vec<(Decimal, Decimal)>,
        asks: Vec<(Decimal, Decimal)>,
    ) -> OrderBook {
        OrderBook::new(
            exchange,
            symbol.clone(),
            bids.into_iter()
                .map(|(p, q)| OrderBookLevel::new(p, q))
                .collect(),
            asks.into_iter()
                .map(|(p, q)| OrderBookLevel::new(p, q))
                .collect(),
        )
    }

    fn create_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    #[tokio::test]
    async fn test_find_best_bid_single_exchange() {
        let symbol = create_symbol();
        let market_data = MarketBundle::new();
        let exchanges = vec![ExchangeId::ByBit];

        let result = find_best_bid(&market_data, &symbol, &exchanges);
        assert!(result.is_none(), "Should return None when no tickers exist");
    }

    #[tokio::test]
    async fn test_find_best_bid_multiple_exchanges() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50005),
            Decimal::from(50015),
        );
        let ticker3 = create_test_ticker(
            ExchangeId::MEXC,
            &symbol,
            Decimal::from(49995),
            Decimal::from(50005),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));
        market_data.add_ticker(Arc::new(ticker3));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX, ExchangeId::MEXC];
        let result = find_best_bid(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (exchange, bid) = result.unwrap();
        assert_eq!(bid, Decimal::from(50005));
        assert_eq!(exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_find_best_bid_with_zero_bid() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::ZERO,
            Decimal::from(50010),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50005),
            Decimal::from(50015),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];
        let result = find_best_bid(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (exchange, bid) = result.unwrap();
        assert_eq!(bid, Decimal::from(50005));
        assert_eq!(exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_find_best_ask_single_exchange() {
        let symbol = create_symbol();
        let market_data = MarketBundle::new();
        let exchanges = vec![ExchangeId::ByBit];

        let result = find_best_ask(&market_data, &symbol, &exchanges);
        assert!(result.is_none(), "Should return None when no tickers exist");
    }

    #[tokio::test]
    async fn test_find_best_ask_multiple_exchanges() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50005),
            Decimal::from(50008),
        );
        let ticker3 = create_test_ticker(
            ExchangeId::MEXC,
            &symbol,
            Decimal::from(49995),
            Decimal::from(50012),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));
        market_data.add_ticker(Arc::new(ticker3));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX, ExchangeId::MEXC];
        let result = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (exchange, ask) = result.unwrap();
        assert_eq!(ask, Decimal::from(50008));
        assert_eq!(exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_find_best_ask_with_zero_ask() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::ZERO,
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50005),
            Decimal::from(50008),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];
        let result = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (exchange, ask) = result.unwrap();
        assert_eq!(ask, Decimal::from(50008));
        assert_eq!(exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_empty_exchanges_list() {
        let symbol = create_symbol();
        let market_data = MarketBundle::new();

        let result_bid = find_best_bid(&market_data, &symbol, &[]);
        let result_ask = find_best_ask(&market_data, &symbol, &[]);

        assert!(result_bid.is_none());
        assert!(result_ask.is_none());
    }

    #[tokio::test]
    async fn test_price_calculation_btc_usdt() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        market_data.add_ticker(Arc::new(ticker));

        let exchanges = vec![ExchangeId::ByBit];

        let bid_result = find_best_bid(&market_data, &symbol, &exchanges);
        let ask_result = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(bid_result.is_some());
        assert!(ask_result.is_some());
        assert_eq!(bid_result.unwrap().1, Decimal::from(50000));
        assert_eq!(ask_result.unwrap().1, Decimal::from(50010));
    }

    #[tokio::test]
    async fn test_spread_computation_across_exchanges() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50005),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(49998),
            Decimal::from(50008),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];

        let best_bid = find_best_bid(&market_data, &symbol, &exchanges);
        let best_ask = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(best_bid.is_some());
        assert!(best_ask.is_some());

        let (_, bid_price) = best_bid.unwrap();
        let (_, ask_price) = best_ask.unwrap();

        let spread = ask_price - bid_price;
        let spread_bps = (spread / bid_price * Decimal::from(10000))
            .to_i32()
            .unwrap();

        assert_eq!(bid_price, Decimal::from(50000));
        assert_eq!(ask_price, Decimal::from(50005));
        assert_eq!(spread, Decimal::from(5));
        assert_eq!(spread_bps, 1);
    }

    #[tokio::test]
    async fn test_volume_normalization() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50001),
        );
        market_data.add_ticker(Arc::new(ticker));

        let order_book = create_test_order_book(
            ExchangeId::ByBit,
            &symbol,
            vec![
                (Decimal::from(50000), Decimal::from(1)),
                (Decimal::from(49999), Decimal::from(2)),
                (Decimal::from(49998), Decimal::from(3)),
            ],
            vec![
                (Decimal::from(50001), Decimal::from(1)),
                (Decimal::from(50002), Decimal::from(2)),
                (Decimal::from(50003), Decimal::from(3)),
            ],
        );

        market_data.add_order_book(Arc::new(order_book));

        let exchanges = vec![ExchangeId::ByBit];

        let best_bid = find_best_bid(&market_data, &symbol, &exchanges);
        let best_ask = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(best_bid.is_some());
        assert!(best_ask.is_some());
        assert_eq!(best_bid.unwrap().1, Decimal::from(50000));
        assert_eq!(best_ask.unwrap().1, Decimal::from(50001));
    }

    #[test]
    fn test_get_spot_exchanges() {
        let exchanges = get_spot_exchanges();
        assert!(!exchanges.is_empty());
        assert!(exchanges.contains(&ExchangeId::OKX));
        assert!(exchanges.contains(&ExchangeId::ByBit));
    }

    #[test]
    fn test_get_perpetual_exchanges() {
        let exchanges = get_perpetual_exchanges();
        assert!(!exchanges.is_empty());
    }

    #[tokio::test]
    async fn test_find_best_bid_equal_prices() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];
        let result = find_best_bid(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (_, bid) = result.unwrap();
        assert_eq!(bid, Decimal::from(50000));
    }

    #[tokio::test]
    async fn test_find_best_ask_equal_prices() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];
        let result = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (_, ask) = result.unwrap();
        assert_eq!(ask, Decimal::from(50010));
    }

    #[tokio::test]
    async fn test_price_calculation_eth_usdt() {
        let symbol = Symbol::new("ETH", "USDT");
        let mut market_data = MarketBundle::new();

        let ticker = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(3000),
            Decimal::from(3005),
        );
        market_data.add_ticker(Arc::new(ticker));

        let exchanges = vec![ExchangeId::ByBit];

        let bid_result = find_best_bid(&market_data, &symbol, &exchanges);
        let ask_result = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(bid_result.is_some());
        assert!(ask_result.is_some());
        assert_eq!(bid_result.unwrap().1, Decimal::from(3000));
        assert_eq!(ask_result.unwrap().1, Decimal::from(3005));
    }

    #[tokio::test]
    async fn test_spread_computation_high_volatility() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(49500),
            Decimal::from(50500),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(49400),
            Decimal::from(50600),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];

        let best_bid = find_best_bid(&market_data, &symbol, &exchanges);
        let best_ask = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(best_bid.is_some());
        assert!(best_ask.is_some());

        let (_, bid_price) = best_bid.unwrap();
        let (_, ask_price) = best_ask.unwrap();

        let spread = ask_price - bid_price;
        let spread_bps = (spread / bid_price * Decimal::from(10000))
            .to_i32()
            .unwrap();

        assert_eq!(bid_price, Decimal::from(49500));
        assert_eq!(ask_price, Decimal::from(50500));
        assert_eq!(spread, Decimal::from(1000));
        assert_eq!(spread_bps, 202);
    }

    #[tokio::test]
    async fn test_volume_normalization_with_different_quantities() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50001),
        );
        market_data.add_ticker(Arc::new(ticker));

        let order_book = create_test_order_book(
            ExchangeId::ByBit,
            &symbol,
            vec![
                (Decimal::from(50000), Decimal::from(10)),
                (Decimal::from(49999), Decimal::from(20)),
                (Decimal::from(49998), Decimal::from(30)),
            ],
            vec![
                (Decimal::from(50001), Decimal::from(15)),
                (Decimal::from(50002), Decimal::from(25)),
                (Decimal::from(50003), Decimal::from(35)),
            ],
        );

        market_data.add_order_book(Arc::new(order_book));

        let exchanges = vec![ExchangeId::ByBit];

        let best_bid = find_best_bid(&market_data, &symbol, &exchanges);
        let best_ask = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(best_bid.is_some());
        assert!(best_ask.is_some());
        assert_eq!(best_bid.unwrap().1, Decimal::from(50000));
        assert_eq!(best_ask.unwrap().1, Decimal::from(50001));
    }

    #[tokio::test]
    async fn test_find_best_bid_with_very_small_price_differences() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000_00000000u64),
            Decimal::from(50000_00000010u64),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50000_00000005u64),
            Decimal::from(50000_00000015u64),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];
        let result = find_best_bid(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (exchange, bid) = result.unwrap();
        assert_eq!(exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_find_best_ask_with_very_small_price_differences() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000_00000000u64),
            Decimal::from(50000_00000015u64),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50000_00000005u64),
            Decimal::from(50000_00000010u64),
        );

        market_data.add_ticker(Arc::new(ticker1));
        market_data.add_ticker(Arc::new(ticker2));

        let exchanges = vec![ExchangeId::ByBit, ExchangeId::OKX];
        let result = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(result.is_some());
        let (exchange, ask) = result.unwrap();
        assert_eq!(exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_spread_computation_stablecoin_pair() {
        let symbol = Symbol::new("USDT", "USDC");
        let mut market_data = MarketBundle::new();

        let ticker = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(9998),
            Decimal::from(10002),
        );
        market_data.add_ticker(Arc::new(ticker));

        let exchanges = vec![ExchangeId::ByBit];

        let best_bid = find_best_bid(&market_data, &symbol, &exchanges);
        let best_ask = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(best_bid.is_some());
        assert!(best_ask.is_some());

        let (_, bid_price) = best_bid.unwrap();
        let (_, ask_price) = best_ask.unwrap();

        let spread = ask_price - bid_price;
        let spread_bps = (spread / bid_price * Decimal::from(10000))
            .to_i32()
            .unwrap();

        assert_eq!(spread, Decimal::from(4));
        assert_eq!(spread_bps, 4);
    }

    #[tokio::test]
    async fn test_volume_normalization_empty_sides() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let order_book = create_test_order_book(ExchangeId::ByBit, &symbol, vec![], vec![]);

        market_data.add_order_book(Arc::new(order_book));

        let exchanges = vec![ExchangeId::ByBit];

        let best_bid = find_best_bid(&market_data, &symbol, &exchanges);
        let best_ask = find_best_ask(&market_data, &symbol, &exchanges);

        assert!(best_bid.is_none());
        assert!(best_ask.is_none());
    }

    #[tokio::test]
    async fn test_multiple_exchanges_all_valid() {
        let symbol = create_symbol();
        let mut market_data = MarketBundle::new();

        let exchanges_list = vec![
            ExchangeId::ByBit,
            ExchangeId::OKX,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
        ];

        for (i, &exchange) in exchanges_list.iter().enumerate() {
            let ticker = create_test_ticker(
                exchange,
                &symbol,
                Decimal::from(50000 + i * 5),
                Decimal::from(50010 + i * 5),
            );
            market_data.add_ticker(Arc::new(ticker));
        }

        let result_bid = find_best_bid(&market_data, &symbol, &exchanges_list);
        let result_ask = find_best_ask(&market_data, &symbol, &exchanges_list);

        assert!(result_bid.is_some());
        assert!(result_ask.is_some());
        assert_eq!(result_bid.unwrap().0, ExchangeId::GateIo);
    }
}
