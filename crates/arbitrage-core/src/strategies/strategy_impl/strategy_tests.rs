#[cfg(test)]
mod tests {
    use crate::strategies::base::{
        FilterContext, MarketBundle, RawSignal, Strategy, StrategyConfig, TradeLeg,
    };
    use crate::strategies::stablecoin_arbitrage::StablecoinArbitrageStrategy;
    use crate::strategies::Ticker;
    use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};
    use rust_decimal::Decimal;
    use serde_json::json;

    fn create_test_market_bundle() -> MarketBundle {
        MarketBundle::new()
    }

    // === Happy Path Tests ===

    #[test]
    fn test_stablecoin_arbitrage_new() {
        let strategy = StablecoinArbitrageStrategy::new();
        assert_eq!(strategy.id(), "stablecoin_arbitrage");
        assert!(strategy.name().contains("Stablecoin"));
    }

    #[test]
    fn test_stablecoin_arbitrage_default() {
        let strategy = StablecoinArbitrageStrategy::default();
        assert_eq!(strategy.id(), "stablecoin_arbitrage");
    }

    #[test]
    fn test_stablecoin_arbitrage_config() {
        let strategy = StablecoinArbitrageStrategy::new();
        let config = strategy.config();
        assert!(config.min_profit_bps > 0);
        assert!(config.max_exposure > Decimal::ZERO);
    }

    #[test]
    fn test_stablecoin_arbitrage_with_config() {
        let config = StrategyConfig {
            min_profit_bps: 100,
            max_exposure: Decimal::from(100000),
            ..StrategyConfig::default()
        };
        let strategy = StablecoinArbitrageStrategy::with_config(config);
        assert_eq!(strategy.config().min_profit_bps, 100);
    }

    #[test]
    fn test_stablecoin_arbitrage_detect_no_data() {
        let strategy = StablecoinArbitrageStrategy::new();
        let market_data = create_test_market_bundle();
        let result = strategy.detect(&market_data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_stablecoin_arbitrage_filter_valid_signal() {
        let strategy = StablecoinArbitrageStrategy::new();
        let mut signal = RawSignal::new("stablecoin_arbitrage", Symbol::new("USDT", "USD"));
        signal.add_leg(TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("USDT", "USD"),
            crate::Side::Buy,
            Decimal::from(99),
            Decimal::from(100),
        ));
        signal.set_profit_bps(50);

        let context = crate::strategies::FilterContext::new(0);
        let result = strategy.filter(&signal, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stablecoin_arbitrage_filter_invalid_signal_empty_legs() {
        let strategy = StablecoinArbitrageStrategy::new();
        let signal = RawSignal::new("stablecoin_arbitrage", Symbol::new("USDT", "USD"));
        let context = crate::strategies::FilterContext::new(0);
        let result = strategy.filter(&signal, &context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_raw_signal_new() {
        let signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        assert_eq!(signal.strategy_id, "test_strategy");
        assert_eq!(signal.symbol.base, "BTC");
        assert!(signal.legs.is_empty());
    }

    #[test]
    fn test_raw_signal_add_leg() {
        let mut signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        let leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );
        signal.add_leg(leg);
        assert_eq!(signal.legs.len(), 1);
    }

    #[test]
    fn test_raw_signal_set_profit_bps() {
        let mut signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        signal.set_profit_bps(100);
        assert_eq!(signal.expected_profit_bps, 100);
    }

    #[test]
    fn test_raw_signal_add_metadata() {
        let mut signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        signal.add_metadata("test_key", json!("test_value"));
        assert!(signal.metadata.contains_key("test_key"));
    }

    #[test]
    fn test_trade_leg_new() {
        let leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );
        assert_eq!(leg.exchange, ExchangeId::OKX);
        assert_eq!(leg.side, crate::Side::Buy);
        assert_eq!(leg.price, Decimal::from(50000));
    }

    #[test]
    fn test_strategy_config_default() {
        let config = StrategyConfig::default();
        assert!(config.min_profit_bps > 0);
        assert!(config.enabled);
    }

    // === Edge Case Tests ===

    #[test]
    fn test_stablecoin_arbitrage_filter_too_many_legs() {
        let strategy = StablecoinArbitrageStrategy::new();
        let mut signal = RawSignal::new("stablecoin_arbitrage", Symbol::new("USDT", "USD"));
        signal.add_leg(TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("USDT", "USD"),
            crate::Side::Buy,
            Decimal::from(99),
            Decimal::from(100),
        ));
        signal.add_leg(TradeLeg::new(
            ExchangeId::ByBit,
            Symbol::new("USDT", "USD"),
            crate::Side::Sell,
            Decimal::from(101),
            Decimal::from(100),
        ));
        signal.add_leg(TradeLeg::new(
            ExchangeId::MEXC,
            Symbol::new("USDT", "USD"),
            crate::Side::Buy,
            Decimal::from(100),
            Decimal::from(100),
        ));

        let context = crate::strategies::FilterContext::new(0);
        let result = strategy.filter(&signal, &context);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_market_bundle_new() {
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
            vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );
        bundle.add_order_book(std::sync::Arc::new(order_book));
        assert_eq!(bundle.order_books.len(), 1);
    }

    // === Error Path Tests ===

    #[test]
    fn test_raw_signal_is_valid_empty() {
        let signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        assert!(!signal.is_valid());
    }

    #[test]
    fn test_raw_signal_is_valid_with_legs() {
        let mut signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        signal.add_leg(TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        ));
        signal.set_profit_bps(100); // Need positive profit for validity
        assert!(signal.is_valid());
    }

    // === Boundary Condition Tests ===

    #[test]
    fn test_strategy_config_zero_min_profit() {
        let config = StrategyConfig {
            min_profit_bps: 0,
            ..StrategyConfig::default()
        };
        assert_eq!(config.min_profit_bps, 0);
    }

    #[test]
    fn test_strategy_config_large_values() {
        let config = StrategyConfig {
            min_profit_bps: 10000,
            max_exposure: Decimal::MAX,
            ..StrategyConfig::default()
        };
        assert!(config.min_profit_bps > 0);
    }

    // === Type Conversion Tests ===

    #[test]
    fn test_trade_leg_clone() {
        let leg = TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        );
        let cloned = leg.clone();
        assert_eq!(leg.exchange, cloned.exchange);
        assert_eq!(leg.price, cloned.price);
    }

    #[test]
    fn test_raw_signal_clone() {
        let mut signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        signal.add_leg(TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        ));
        signal.set_profit_bps(100);
        signal.add_metadata("key", json!("value"));

        let cloned = signal.clone();
        assert_eq!(signal.expected_profit_bps, cloned.expected_profit_bps);
        assert_eq!(signal.legs.len(), cloned.legs.len());
    }

    // === Additional Tests ===

    #[test]
    fn test_filter_context_default() {
        let context = FilterContext::new(50);
        assert!(context.min_profit_bps >= 0);
        assert!(context.max_exposure > Decimal::ZERO);
    }

    #[test]
    fn test_filter_context_with_values() {
        let context = crate::strategies::FilterContext {
            min_profit_bps: 50,
            max_exposure: Decimal::from(50000),
            ..crate::strategies::FilterContext::new(0)
        };
        assert_eq!(context.min_profit_bps, 50);
    }

    #[test]
    fn test_market_bundle_get_order_book() {
        let mut bundle = MarketBundle::new();
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );
        bundle.add_order_book(std::sync::Arc::new(order_book));

        let result = bundle.get_order_book(ExchangeId::OKX, &symbol);
        assert!(result.is_some());
    }

    #[test]
    fn test_market_bundle_get_order_book_not_found() {
        let bundle = MarketBundle::new();
        let symbol = Symbol::new("BTC", "USDT");
        let result = bundle.get_order_book(ExchangeId::OKX, &symbol);
        assert!(result.is_none());
    }

    #[test]
    fn test_stablecoin_arbitrage_detect_empty_market_data() {
        let strategy = StablecoinArbitrageStrategy::new();
        let market_data = create_test_market_bundle();
        let result = strategy.detect(&market_data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_raw_signal_total_notional() {
        let mut signal = RawSignal::new("test_strategy", Symbol::new("BTC", "USDT"));
        signal.add_leg(TradeLeg::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            crate::Side::Buy,
            Decimal::from(50000),
            Decimal::from(2),
        ));
        let notional = signal.total_notional();
        assert_eq!(notional, Decimal::from(100000));
    }

    #[test]
    fn test_market_bundle_with_ticker() {
        let mut bundle = MarketBundle::new();
        let symbol = Symbol::new("BTC", "USDT");
        let ticker = Ticker {
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
            std::sync::Arc::new(ticker),
        );
        assert_eq!(bundle.tickers.len(), 1);
    }
}
