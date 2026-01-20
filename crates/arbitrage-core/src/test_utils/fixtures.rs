use crate::{
    config::Config,
    strategies::{ConfidenceFactors, FilterContext, MarketBundle, RawSignal, Ticker, TradeLeg},
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
};
use chrono::Utc;
use rust_decimal::Decimal;
use std::sync::Arc;

pub struct OrderBookBuilder {
    exchange: ExchangeId,
    symbol: Symbol,
    bids: Vec<(Decimal, Decimal)>,
    asks: Vec<(Decimal, Decimal)>,
    sequence: Option<u64>,
}

impl OrderBookBuilder {
    pub fn new(exchange: ExchangeId, symbol: Symbol) -> Self {
        Self {
            exchange,
            symbol,
            bids: Vec::new(),
            asks: Vec::new(),
            sequence: None,
        }
    }

    pub fn bid(mut self, price: Decimal, quantity: Decimal) -> Self {
        self.bids.push((price, quantity));
        self
    }

    pub fn asks(mut self, price: Decimal, quantity: Decimal) -> Self {
        self.asks.push((price, quantity));
        self
    }

    pub fn bids_vec(mut self, bids: Vec<(Decimal, Decimal)>) -> Self {
        self.bids = bids;
        self
    }

    pub fn asks_vec(mut self, asks: Vec<(Decimal, Decimal)>) -> Self {
        self.asks = asks;
        self
    }

    pub fn sequence(mut self, seq: u64) -> Self {
        self.sequence = Some(seq);
        self
    }

    pub fn build(self) -> OrderBook {
        let order_bids: Vec<OrderBookLevel> = self
            .bids
            .into_iter()
            .map(|(price, qty)| OrderBookLevel::new(price, qty))
            .collect();
        let order_asks: Vec<OrderBookLevel> = self
            .asks
            .into_iter()
            .map(|(price, qty)| OrderBookLevel::new(price, qty))
            .collect();

        let mut book = OrderBook::new(self.exchange, self.symbol, order_bids, order_asks);
        if let Some(seq) = self.sequence {
            book.sequence = Some(seq);
        }
        book
    }
}

pub struct TickerBuilder {
    exchange: ExchangeId,
    symbol: Symbol,
    bid: Decimal,
    ask: Decimal,
    last: Decimal,
    volume_24h: Decimal,
    change_24h: Decimal,
}

impl TickerBuilder {
    pub fn new(exchange: ExchangeId, symbol: Symbol, bid: Decimal, ask: Decimal) -> Self {
        let last = (bid + ask) / Decimal::from(2);
        Self {
            exchange,
            symbol,
            bid,
            ask,
            last,
            volume_24h: Decimal::ZERO,
            change_24h: Decimal::ZERO,
        }
    }

    pub fn last(mut self, last: Decimal) -> Self {
        self.last = last;
        self
    }

    pub fn volume_24h(mut self, volume: Decimal) -> Self {
        self.volume_24h = volume;
        self
    }

    pub fn change_24h(mut self, change: Decimal) -> Self {
        self.change_24h = change;
        self
    }

    pub fn build(self) -> Ticker {
        Ticker {
            exchange: self.exchange,
            symbol: self.symbol,
            bid: self.bid,
            ask: self.ask,
            last: self.last,
            volume_24h: self.volume_24h,
            change_24h: self.change_24h,
            timestamp: Utc::now(),
        }
    }
}

pub struct SignalBuilder {
    strategy_id: String,
    symbol: Symbol,
    legs: Vec<TradeLeg>,
    expected_profit_bps: i32,
    basis_bps: Option<i32>,
    confidence_factors: ConfidenceFactors,
    metadata: Vec<(String, serde_json::Value)>,
}

impl SignalBuilder {
    pub fn new(strategy_id: impl Into<String>, symbol: Symbol) -> Self {
        Self {
            strategy_id: strategy_id.into(),
            symbol,
            legs: Vec::new(),
            expected_profit_bps: 0,
            basis_bps: None,
            confidence_factors: ConfidenceFactors::default(),
            metadata: Vec::new(),
        }
    }

    pub fn buy_leg(mut self, exchange: ExchangeId, price: Decimal, quantity: Decimal) -> Self {
        let leg = TradeLeg::new(
            exchange,
            self.symbol.clone(),
            crate::Side::Buy,
            price,
            quantity,
        );
        self.legs.push(leg);
        self
    }

    pub fn sell_leg(mut self, exchange: ExchangeId, price: Decimal, quantity: Decimal) -> Self {
        let leg = TradeLeg::new(
            exchange,
            self.symbol.clone(),
            crate::Side::Sell,
            price,
            quantity,
        );
        self.legs.push(leg);
        self
    }

    pub fn profit_bps(mut self, bps: i32) -> Self {
        self.expected_profit_bps = bps;
        self
    }

    pub fn basis_bps(mut self, bps: i32) -> Self {
        self.basis_bps = Some(bps);
        self
    }

    pub fn confidence_factors(mut self, factors: ConfidenceFactors) -> Self {
        self.confidence_factors = factors;
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.push((key.into(), value));
        self
    }

    pub fn build(self) -> RawSignal {
        let mut signal = RawSignal::new(self.strategy_id, self.symbol);
        signal.legs = self.legs;
        signal.expected_profit_bps = self.expected_profit_bps;
        signal.basis_bps = self.basis_bps;
        signal.confidence_factors = self.confidence_factors;

        for (key, value) in self.metadata {
            signal.add_metadata(key, value);
        }

        signal
    }
}

pub struct MarketBundleBuilder {
    order_books: Vec<OrderBook>,
    tickers: Vec<Ticker>,
}

impl MarketBundleBuilder {
    pub fn new() -> Self {
        Self {
            order_books: Vec::new(),
            tickers: Vec::new(),
        }
    }

    pub fn order_book(mut self, book: OrderBook) -> Self {
        self.order_books.push(book);
        self
    }

    pub fn ticker(mut self, ticker: Ticker) -> Self {
        self.tickers.push(ticker);
        self
    }

    pub fn build(self) -> MarketBundle {
        let mut bundle = MarketBundle::new();

        for book in self.order_books {
            bundle.add_order_book(Arc::new(book));
        }

        for ticker in self.tickers {
            bundle.add_ticker(Arc::new(ticker));
        }

        bundle
    }
}

impl Default for MarketBundleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ConfigBuilder {
    min_profit_threshold_percent: Decimal,
    max_position_size_usd: Decimal,
    port: u16,
    host: String,
    enabled_exchanges: Vec<ExchangeId>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            min_profit_threshold_percent: Decimal::new(1, 4),
            max_position_size_usd: Decimal::from(200000),
            port: 3000,
            host: "127.0.0.1".to_string(),
            enabled_exchanges: vec![
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::MEXC,
                ExchangeId::GateIo,
            ],
        }
    }

    pub fn min_profit_threshold(mut self, percent: Decimal) -> Self {
        self.min_profit_threshold_percent = percent;
        self
    }

    pub fn max_position_size(mut self, size: Decimal) -> Self {
        self.max_position_size_usd = size;
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn host(mut self, host: &str) -> Self {
        self.host = host.to_string();
        self
    }

    pub fn enabled_exchanges(mut self, exchanges: Vec<ExchangeId>) -> Self {
        self.enabled_exchanges = exchanges;
        self
    }

    pub fn build(self) -> Config {
        let mut config = Config::default();

        config.server.port = self.port;
        config.server.host = self.host;

        config.trading.min_profit_threshold_percent = self.min_profit_threshold_percent;
        config.risk.max_position_size_usd = self.max_position_size_usd;

        for exchange in self.enabled_exchanges {
            if let Some(exchange_config) = config.exchanges.get_mut(&exchange) {
                exchange_config.enabled = true;
            }
        }

        config
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FilterContextBuilder {
    min_profit_bps: i32,
    max_exposure: Decimal,
    allowed_exchanges: Vec<ExchangeId>,
    max_latency_ms: u64,
    min_notional_usd: Decimal,
}

impl FilterContextBuilder {
    pub fn new(min_profit_bps: i32) -> Self {
        Self {
            min_profit_bps,
            max_exposure: Decimal::from(10000),
            allowed_exchanges: vec![
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::MEXC,
                ExchangeId::GateIo,
            ],
            max_latency_ms: 500,
            min_notional_usd: Decimal::from(10),
        }
    }

    pub fn max_exposure(mut self, exposure: Decimal) -> Self {
        self.max_exposure = exposure;
        self
    }

    pub fn allowed_exchanges(mut self, exchanges: Vec<ExchangeId>) -> Self {
        self.allowed_exchanges = exchanges;
        self
    }

    pub fn max_latency(mut self, ms: u64) -> Self {
        self.max_latency_ms = ms;
        self
    }

    pub fn min_notional(mut self, notional: Decimal) -> Self {
        self.min_notional_usd = notional;
        self
    }

    pub fn build(self) -> FilterContext {
        let mut context = FilterContext::new(self.min_profit_bps);
        context.max_exposure = self.max_exposure;
        context.allowed_exchanges = self.allowed_exchanges;
        context.max_latency_ms = self.max_latency_ms;
        context.min_notional_usd = self.min_notional_usd;
        context
    }
}

pub struct TestFixtures;

impl TestFixtures {
    pub fn order_book() -> OrderBookBuilder {
        OrderBookBuilder::new(ExchangeId::OKX, Symbol::new("BTC", "USDT"))
    }

    pub fn ticker() -> TickerBuilder {
        TickerBuilder::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
        )
    }

    pub fn signal() -> SignalBuilder {
        SignalBuilder::new("test_strategy", Symbol::new("BTC", "USDT"))
    }

    pub fn market_bundle() -> MarketBundleBuilder {
        MarketBundleBuilder::new()
    }

    pub fn config() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    pub fn filter_context() -> FilterContextBuilder {
        FilterContextBuilder::new(10)
    }

    pub fn arbitrage_opportunity() -> MarketBundle {
        let symbol = Symbol::new("BTC", "USDT");

        let okx_book = OrderBookBuilder::new(ExchangeId::OKX, symbol.clone())
            .bid(Decimal::from(50000), Decimal::from(1))
            .asks(Decimal::from(50010), Decimal::from(1))
            .build();

        let bybit_book = OrderBookBuilder::new(ExchangeId::ByBit, symbol.clone())
            .bid(Decimal::from(50200), Decimal::from(1))
            .asks(Decimal::from(50210), Decimal::from(1))
            .build();

        MarketBundleBuilder::new()
            .order_book(okx_book)
            .order_book(bybit_book)
            .build()
    }

    pub fn no_opportunity() -> MarketBundle {
        let symbol = Symbol::new("ETH", "USDT");

        let okx_book = OrderBookBuilder::new(ExchangeId::OKX, symbol.clone())
            .bid(Decimal::from(3000), Decimal::from(10))
            .asks(Decimal::from(3010), Decimal::from(10))
            .build();

        let bybit_book = OrderBookBuilder::new(ExchangeId::ByBit, symbol.clone())
            .bid(Decimal::from(3005), Decimal::from(10))
            .asks(Decimal::from(3015), Decimal::from(10))
            .build();

        MarketBundleBuilder::new()
            .order_book(okx_book)
            .order_book(bybit_book)
            .build()
    }

    pub fn high_volatility() -> MarketBundle {
        let symbol = Symbol::new("BTC", "USDT");

        let okx_book = OrderBookBuilder::new(ExchangeId::OKX, symbol.clone())
            .bid(Decimal::from(49500), Decimal::from(5))
            .bid(Decimal::from(49400), Decimal::from(3))
            .asks(Decimal::from(50600), Decimal::from(4))
            .asks(Decimal::from(50700), Decimal::from(2))
            .build();

        let bybit_book = OrderBookBuilder::new(ExchangeId::ByBit, symbol.clone())
            .bid(Decimal::from(49600), Decimal::from(4))
            .bid(Decimal::from(49500), Decimal::from(2))
            .asks(Decimal::from(50500), Decimal::from(5))
            .asks(Decimal::from(50600), Decimal::from(3))
            .build();

        MarketBundleBuilder::new()
            .order_book(okx_book)
            .order_book(bybit_book)
            .build()
    }

    pub fn thin_liquidity() -> MarketBundle {
        let symbol = Symbol::new("XRP", "USDT");

        let okx_book = OrderBookBuilder::new(ExchangeId::OKX, symbol.clone())
            .bid(Decimal::from(50), Decimal::from(100))
            .asks(Decimal::from(51), Decimal::from(50))
            .build();

        let bybit_book = OrderBookBuilder::new(ExchangeId::ByBit, symbol.clone())
            .bid(Decimal::from(49), Decimal::from(80))
            .asks(Decimal::from(52), Decimal::from(100))
            .build();

        MarketBundleBuilder::new()
            .order_book(okx_book)
            .order_book(bybit_book)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_book_builder() {
        let book = OrderBookBuilder::new(ExchangeId::OKX, Symbol::new("BTC", "USDT"))
            .bid(Decimal::from(50000), Decimal::from(1))
            .asks(Decimal::from(50010), Decimal::from(1))
            .build();

        assert_eq!(book.exchange, ExchangeId::OKX);
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
    }

    #[test]
    fn test_ticker_builder() {
        let ticker = TickerBuilder::new(
            ExchangeId::OKX,
            Symbol::new("BTC", "USDT"),
            Decimal::from(50000),
            Decimal::from(50010),
        )
        .volume_24h(Decimal::from(1000000))
        .build();

        assert_eq!(ticker.exchange, ExchangeId::OKX);
        assert_eq!(ticker.volume_24h, Decimal::from(1000000));
    }

    #[test]
    fn test_signal_builder() {
        let signal = SignalBuilder::new("test_strategy", Symbol::new("BTC", "USDT"))
            .buy_leg(ExchangeId::OKX, Decimal::from(50000), Decimal::from(1))
            .sell_leg(ExchangeId::ByBit, Decimal::from(50200), Decimal::from(1))
            .profit_bps(40)
            .build();

        assert_eq!(signal.strategy_id, "test_strategy");
        assert_eq!(signal.legs.len(), 2);
        assert_eq!(signal.expected_profit_bps, 40);
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .min_profit_threshold(Decimal::new(5, 4))
            .max_position_size(Decimal::from(500000))
            .port(8080)
            .build();

        assert_eq!(config.server.port, 8080);
        assert_eq!(
            config.trading.min_profit_threshold_percent,
            Decimal::new(5, 4)
        );
        assert_eq!(config.risk.max_position_size_usd, Decimal::from(500000));
    }

    #[test]
    fn test_filter_context_builder() {
        let context = FilterContextBuilder::new(20)
            .max_exposure(Decimal::from(50000))
            .max_latency(1000)
            .build();

        assert_eq!(context.min_profit_bps, 20);
        assert_eq!(context.max_exposure, Decimal::from(50000));
        assert_eq!(context.max_latency_ms, 1000);
    }

    #[test]
    fn test_arbitrage_opportunity_preset() {
        let bundle = TestFixtures::arbitrage_opportunity();

        let btc_symbol = Symbol::new("BTC", "USDT");
        let okx_book = bundle.get_order_book(ExchangeId::OKX, &btc_symbol);
        let bybit_book = bundle.get_order_book(ExchangeId::ByBit, &btc_symbol);

        assert!(okx_book.is_some());
        assert!(bybit_book.is_some());
    }

    #[test]
    fn test_no_opportunity_preset() {
        let bundle = TestFixtures::no_opportunity();
        assert!(bundle.get_all_symbols().len() >= 1);
    }
}
