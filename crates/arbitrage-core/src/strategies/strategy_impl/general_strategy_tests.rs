#[cfg(test)]
mod tests {
    use crate::strategies::base::{
        MarketBundle, RawSignal, Strategy, StrategyConfig, Ticker, TradeLeg,
    };
    use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};
    use rust_decimal::Decimal;

    // === General Strategy Tests ===

    #[test]
    fn test_market_bundle_get_all_symbols() {
        let mut bundle = MarketBundle::new();
        let btc = Symbol::new("BTC", "USDT");
        let eth = Symbol::new("ETH", "USDT");

        let btc_book = OrderBook::new(
            ExchangeId::OKX,
            btc.clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );
        bundle.add_order_book(std::sync::Arc::new(btc_book));

        let eth_book = OrderBook::new(
            ExchangeId::OKX,
            eth.clone(),
            vec![OrderBookLevel::new(Decimal::from(3000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(3001), Decimal::from(1))],
        );
        bundle.add_order_book(std::sync::Arc::new(eth_book));

        let symbols = bundle.get_all_symbols();
        assert!(symbols.iter().any(|s| s.base == "BTC" && s.quote == "USDT"));
        assert!(symbols.iter().any(|s| s.base == "ETH" && s.quote == "USDT"));
    }

    #[test]
    fn test_market_bundle_empty_symbols() {
        let bundle = MarketBundle::new();
        let symbols = bundle.get_all_symbols();
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_strategy_config_update() {
        let mut config = StrategyConfig::default();
        config.min_profit_bps = 100;
        config.max_exposure = Decimal::from(100000);

        let mut strategy = crate::strategies::strategy_impl::stablecoin_arbitrage::StablecoinArbitrageStrategy::new();
        let result = strategy.update_config(config);
        assert!(result.is_ok());
        assert_eq!(strategy.config().min_profit_bps, 100);
    }

    #[test]
    fn test_strategy_enabled_flag() {
        let config = StrategyConfig {
            enabled: false,
            ..StrategyConfig::default()
        };
        let strategy = crate::strategies::strategy_impl::stablecoin_arbitrage::StablecoinArbitrageStrategy::with_config(config);
        assert!(!strategy.config().enabled);
    }

    // === RawSignal Tests ===

    #[test]
    fn test_raw_signal_profit_bps_boundary() {
        let mut signal = RawSignal::new("test", Symbol::new("BTC", "USDT").into());
        signal.set_profit_bps(i32::MAX);
        assert_eq!(signal.expected_profit_bps, i32::MAX);

        signal.set_profit_bps(i32::MIN);
        assert_eq!(signal.expected_profit_bps, i32::MIN);
    }

    #[test]
    fn test_raw_signal_multiple_legs() {
        let mut signal = RawSignal::new("test", Symbol::new("BTC", "USDT").into());
        for i in 1..=5 {
            signal.add_leg(TradeLeg::new(
                ExchangeId::OKX,
                Symbol::new("BTC", "USDT").into(),
                crate::Side::Buy,
                Decimal::from(50000),
                Decimal::from(i),
            ));
        }
        assert_eq!(signal.legs.len(), 5);
    }

    #[test]
    fn test_raw_signal_metadata_types() {
        let mut signal = RawSignal::new("test", Symbol::new("BTC", "USDT").into());
        signal.add_metadata("string", serde_json::json!("value"));
        signal.add_metadata("number", serde_json::json!(42));
        signal.add_metadata("float", serde_json::json!(3.14));
        signal.add_metadata("bool", serde_json::json!(true));
        signal.add_metadata("array", serde_json::json!([1, 2, 3]));

        assert_eq!(signal.metadata.len(), 5);
    }

    // === TradeLeg Tests ===

    #[test]
    fn test_trade_leg_sides() {
        let buy_leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT").into(),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );
        assert_eq!(buy_leg.side, crate::Side::Buy);

        let sell_leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT").into(),
            crate::Side::Sell,
            Decimal::from(50001),
            Decimal::from(1),
        );
        assert_eq!(sell_leg.side, crate::Side::Sell);
    }

    #[test]
    fn test_trade_leg_extreme_values() {
        let leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT").into(),
            crate::Side::Buy,
            Decimal::MAX,
            Decimal::MAX,
        );
        assert_eq!(leg.price, Decimal::MAX);
        assert_eq!(leg.quantity, Decimal::MAX);
    }

    #[test]
    fn test_trade_leg_notional() {
        let leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT").into(),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(2),
        );
        let notional = leg.price * leg.quantity;
        assert_eq!(notional, Decimal::from(100000));
    }

    // === MarketBundle Tests ===

    #[test]
    fn test_market_bundle_multiple_exchanges() {
        let mut bundle = MarketBundle::new();
        let symbol = Symbol::new("BTC", "USDT");

        for exchange in [ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC] {
            let order_book = OrderBook::new(
                exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))],
                vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
            );
            bundle.add_order_book(std::sync::Arc::new(order_book));
        }

        assert_eq!(bundle.order_books.len(), 3);
    }

    #[test]
    fn test_market_bundle_get_nonexistent() {
        let bundle = MarketBundle::new();
        let symbol = Symbol::new("BTC", "USDT");
        assert!(bundle.get_order_book(ExchangeId::OKX, &symbol).is_none());
    }

    #[test]
    fn test_market_bundle_tickers_insertion() {
        let mut bundle = MarketBundle::new();
        let symbol = Symbol::new("BTC", "USDT");

        let ticker1 = Ticker {
            exchange: ExchangeId::OKX,
            symbol: symbol.clone(),
            bid: Decimal::from(49999),
            ask: Decimal::from(50001),
            last: Decimal::from(50000),
            volume_24h: Decimal::from(1000),
            change_24h: Decimal::from(100),
            timestamp: chrono::Utc::now(),
        };
        bundle.tickers.insert(
            (ExchangeId::OKX, std::sync::Arc::new(symbol.clone())),
            std::sync::Arc::new(ticker1),
        );

        let ticker2 = Ticker {
            exchange: ExchangeId::ByBit,
            symbol: symbol.clone(),
            bid: Decimal::from(49998),
            ask: Decimal::from(50002),
            last: Decimal::from(50000),
            volume_24h: Decimal::from(2000),
            change_24h: Decimal::from(200),
            timestamp: chrono::Utc::now(),
        };
        bundle.tickers.insert(
            (ExchangeId::ByBit, std::sync::Arc::new(symbol.clone())),
            std::sync::Arc::new(ticker2),
        );

        assert_eq!(bundle.tickers.len(), 2);
    }

    // === FilterContext Tests ===

    #[test]
    fn test_filter_context_custom_min_profit() {
        let context = crate::strategies::FilterContext::new(0);
        assert!(context.min_profit_bps >= 0);
    }

    #[test]
    fn test_filter_context_custom_exposure() {
        let context = crate::strategies::FilterContext {
            max_exposure: Decimal::from(1000000),
            min_notional_usd: Decimal::from(10),
            ..crate::strategies::FilterContext::new(0)
        };
        assert_eq!(context.max_exposure, Decimal::from(1000000));
    }

    #[test]
    fn test_filter_context_allowed_exchanges() {
        let context = crate::strategies::FilterContext {
            allowed_exchanges: vec![ExchangeId::OKX, ExchangeId::ByBit],
            min_notional_usd: Decimal::from(10),
            ..crate::strategies::FilterContext::new(0)
        };
        assert!(context.is_exchange_allowed(ExchangeId::OKX));
        assert!(context.is_exchange_allowed(ExchangeId::ByBit));
        assert!(!context.is_exchange_allowed(ExchangeId::MEXC));
    }

    // === Symbol Tests ===

    #[test]
    fn test_symbol_base_quote() {
        let symbol = Symbol::new("BTC", "USDT");
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_symbol_pair() {
        let symbol = Symbol::from_pair("BTC/USDT");
        assert!(symbol.is_some());
        let s = symbol.unwrap();
        assert_eq!(s.base, "BTC");
        assert_eq!(s.quote, "USDT");
    }

    #[test]
    fn test_symbol_pair_invalid() {
        let symbol = Symbol::from_pair("INVALID");
        assert!(symbol.is_none());
    }

    #[test]
    fn test_symbol_display() {
        let symbol = Symbol::new("BTC", "USDT");
        let display = format!("{}", symbol);
        assert!(display.contains("BTC"));
        assert!(display.contains("USDT"));
    }

    // === OrderBook Tests ===

    #[test]
    fn test_order_book_best_bid_ask() {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![
                OrderBookLevel::new(Decimal::from(49999), Decimal::from(1)),
                OrderBookLevel::new(Decimal::from(49998), Decimal::from(2)),
            ],
            vec![
                OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
                OrderBookLevel::new(Decimal::from(50002), Decimal::from(2)),
            ],
        );

        let best_bid = order_book.best_bid();
        let best_ask = order_book.best_ask();

        assert!(best_bid.is_some());
        assert!(best_ask.is_some());
        assert_eq!(best_bid.unwrap().price, Decimal::from(49999));
        assert_eq!(best_ask.unwrap().price, Decimal::from(50001));
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

        let spread = order_book.spread();
        assert!(spread.is_some());
        assert_eq!(spread.unwrap(), Decimal::from(20));
    }

    #[test]
    fn test_order_book_is_valid() {
        let symbol = Symbol::new("BTC", "USDT");

        let valid = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49990), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        );
        assert!(valid.is_valid());

        let invalid = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        );
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_order_book_mid_price() {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(Decimal::from(49900), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
        );

        let mid = order_book.mid_price();
        assert!(mid.is_some());
        assert_eq!(mid.unwrap(), Decimal::from(50000));
    }

    // === Strategy Utilities Tests ===

    #[test]
    fn test_strategy_config_debug() {
        let config = StrategyConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("min_profit_bps"));
        assert!(debug.contains("enabled"));
    }

    #[test]
    fn test_raw_signal_debug() {
        let mut signal = RawSignal::new("test", Symbol::new("BTC", "USDT").into());
        signal.set_profit_bps(100);
        let debug = format!("{:?}", signal);
        assert!(debug.contains("test"));
        assert!(debug.contains("BTC"));
    }

    #[test]
    fn test_trade_leg_debug() {
        let leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT").into(),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );
        let debug = format!("{:?}", leg);
        assert!(debug.contains("OKX"));
        assert!(debug.contains("Buy"));
    }

    #[test]
    fn test_market_bundle_debug() {
        let bundle = MarketBundle::new();
        let debug = format!("{:?}", bundle);
        assert!(debug.contains("order_books"));
    }
}
