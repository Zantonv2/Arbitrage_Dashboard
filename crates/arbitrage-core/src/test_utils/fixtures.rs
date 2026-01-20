use crate::strategies::{
    ConfidenceFactors, FeeSchedule, FilterContext, FundingRate, MarketBundle, RawSignal, Ticker,
    TradeLeg,
};
use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};
use rust_decimal::Decimal;
use std::sync::Arc;

pub fn btc_usdt_symbol() -> Symbol {
    Symbol::new("BTC", "USDT")
}

pub fn eth_usdt_symbol() -> Symbol {
    Symbol::new("ETH", "USDT")
}

pub fn sol_usdt_symbol() -> Symbol {
    Symbol::new("SOL", "USDT")
}

pub fn eth_usdc_symbol() -> Symbol {
    Symbol::new("ETH", "USDC")
}

pub fn usdt_usdc_symbol() -> Symbol {
    Symbol::new("USDT", "USDC")
}

pub fn okx_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::OKX,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(2))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    )
}

pub fn bybit_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50015), Decimal::from(2))],
    )
}

pub fn mexc_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::MEXC,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(3))],
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(2))],
    )
}

pub fn gateio_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::GateIo,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50002), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50008), Decimal::from(1))],
    )
}

pub fn valid_btc_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![
            OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)),
            OrderBookLevel::new(Decimal::from(49999), Decimal::from(2)),
        ],
        vec![
            OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
            OrderBookLevel::new(Decimal::from(50002), Decimal::from(2)),
        ],
    )
}

pub fn invalid_btc_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50002), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
    )
}

pub fn low_liquidity_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
    )
}

pub fn wide_spread_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(49500), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(50500), Decimal::from(5))],
    )
}

pub fn funding_rate_okx() -> FundingRate {
    FundingRate::new(
        ExchangeId::OKX,
        btc_usdt_symbol(),
        Decimal::from_str_exact("0.0100").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn funding_rate_bybit() -> FundingRate {
    FundingRate::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        Decimal::from_str_exact("-0.0050").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn high_funding_rate() -> FundingRate {
    FundingRate::new(
        ExchangeId::OKX,
        btc_usdt_symbol(),
        Decimal::from_str_exact("0.0500").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn low_funding_rate() -> FundingRate {
    FundingRate::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        Decimal::from_str_exact("-0.0500").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn fee_schedule_okx() -> FeeSchedule {
    FeeSchedule::new(
        ExchangeId::OKX,
        Decimal::from_str_exact("-0.02").unwrap(),
        Decimal::from_str_exact("0.05").unwrap(),
    )
}

pub fn fee_schedule_bybit() -> FeeSchedule {
    FeeSchedule::new(
        ExchangeId::ByBit,
        Decimal::from_str_exact("-0.01").unwrap(),
        Decimal::from_str_exact("0.06").unwrap(),
    )
}

pub fn high_confidence_factors() -> ConfidenceFactors {
    ConfidenceFactors {
        depth_score: Decimal::from(90),
        volatility_score: Decimal::from(20),
        reliability_score: Decimal::from(95),
        spread_stability: Decimal::from(85),
        freshness_score: Decimal::from(100),
    }
}

pub fn low_confidence_factors() -> ConfidenceFactors {
    ConfidenceFactors {
        depth_score: Decimal::from(20),
        volatility_score: Decimal::from(80),
        reliability_score: Decimal::from(30),
        spread_stability: Decimal::from(25),
        freshness_score: Decimal::from(40),
    }
}

pub fn buy_trade_leg(exchange: ExchangeId, price: Decimal, quantity: Decimal) -> TradeLeg {
    TradeLeg::new(
        exchange,
        btc_usdt_symbol(),
        crate::types::Side::Buy,
        price,
        quantity,
    )
}

pub fn sell_trade_leg(exchange: ExchangeId, price: Decimal, quantity: Decimal) -> TradeLeg {
    TradeLeg::new(
        exchange,
        btc_usdt_symbol(),
        crate::types::Side::Sell,
        price,
        quantity,
    )
}

pub struct TestFixtures;

impl TestFixtures {
    pub fn order_book() -> OrderBookBuilder {
        OrderBookBuilder::new()
    }

    pub fn ticker() -> TickerBuilder {
        TickerBuilder::new()
    }

    pub fn signal() -> SignalBuilder {
        SignalBuilder::new()
    }

    pub fn market_bundle() -> MarketBundleBuilder {
        MarketBundleBuilder::new()
    }

    pub fn config() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    pub fn filter_context() -> FilterContextBuilder {
        FilterContextBuilder::new()
    }

    pub fn arbitrary_opportunity() -> RawSignal {
        TestFixtures::signal()
            .symbol("BTC", "USDT")
            .exchange(
                ExchangeId::OKX,
                ExchangeId::ByBit,
                Decimal::from(50000),
                Decimal::from(50050),
            )
            .profit_bps(10)
            .build()
    }

    pub fn no_opportunity() -> RawSignal {
        TestFixtures::signal()
            .symbol("BTC", "USDT")
            .exchange(
                ExchangeId::OKX,
                ExchangeId::ByBit,
                Decimal::from(50000),
                Decimal::from(50000),
            )
            .profit_bps(0)
            .build()
    }

    pub fn high_volatility() -> RawSignal {
        TestFixtures::signal()
            .symbol("BTC", "USDT")
            .exchange(
                ExchangeId::OKX,
                ExchangeId::ByBit,
                Decimal::from(49000),
                Decimal::from(51000),
            )
            .profit_bps(408)
            .build()
    }

    pub fn thin_liquidity() -> RawSignal {
        TestFixtures::signal()
            .symbol("BTC", "USDT")
            .exchange(
                ExchangeId::OKX,
                ExchangeId::ByBit,
                Decimal::from(50000),
                Decimal::from(50010),
            )
            .profit_bps(20)
            .build()
    }
}

pub struct OrderBookBuilder {
    exchange: Option<ExchangeId>,
    symbol: Option<Symbol>,
    bid_price: Option<Decimal>,
    ask_price: Option<Decimal>,
    bid_qty: Option<Decimal>,
    ask_qty: Option<Decimal>,
    depth: usize,
}

impl OrderBookBuilder {
    fn new() -> Self {
        Self {
            exchange: None,
            symbol: None,
            bid_price: None,
            ask_price: None,
            bid_qty: None,
            ask_qty: None,
            depth: 1,
        }
    }

    pub fn exchange(mut self, exchange: ExchangeId) -> Self {
        self.exchange = Some(exchange);
        self
    }

    pub fn symbol(mut self, base: impl Into<String>, quote: impl Into<String>) -> Self {
        self.symbol = Some(Symbol::new(base, quote));
        self
    }

    pub fn spread(mut self, bid: Decimal, ask: Decimal, qty: Decimal) -> Self {
        self.bid_price = Some(bid);
        self.ask_price = Some(ask);
        self.bid_qty = Some(qty);
        self.ask_qty = Some(qty);
        self
    }

    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    pub fn build(self) -> OrderBook {
        let exchange = self.exchange.unwrap_or(ExchangeId::OKX);
        let symbol = self.symbol.unwrap_or_else(|| Symbol::new("BTC", "USDT"));
        let bid_price = self.bid_price.unwrap_or(Decimal::from(50000));
        let ask_price = self.ask_price.unwrap_or(Decimal::from(50010));
        let qty = self.bid_qty.unwrap_or(Decimal::from(1));

        let bids: Vec<OrderBookLevel> = (0..self.depth)
            .map(|i| OrderBookLevel::new(bid_price - Decimal::from(i), qty))
            .collect();

        let asks: Vec<OrderBookLevel> = (0..self.depth)
            .map(|i| OrderBookLevel::new(ask_price + Decimal::from(i), qty))
            .collect();

        OrderBook::new(exchange, symbol, bids, asks)
    }
}

pub struct TickerBuilder {
    exchange: Option<ExchangeId>,
    symbol: Option<Symbol>,
    last_price: Option<Decimal>,
    bid: Option<Decimal>,
    ask: Option<Decimal>,
    volume_24h: Option<Decimal>,
    change_24h: Option<Decimal>,
}

impl TickerBuilder {
    fn new() -> Self {
        Self {
            exchange: None,
            symbol: None,
            last_price: None,
            bid: None,
            ask: None,
            volume_24h: None,
            change_24h: None,
        }
    }

    pub fn exchange(mut self, exchange: ExchangeId) -> Self {
        self.exchange = Some(exchange);
        self
    }

    pub fn symbol(mut self, base: impl Into<String>, quote: impl Into<String>) -> Self {
        self.symbol = Some(Symbol::new(base, quote));
        self
    }

    pub fn last_price(mut self, price: Decimal) -> Self {
        self.last_price = Some(price);
        self
    }

    pub fn bid(mut self, bid: Decimal) -> Self {
        self.bid = Some(bid);
        self
    }

    pub fn ask(mut self, ask: Decimal) -> Self {
        self.ask = Some(ask);
        self
    }

    pub fn build(self) -> Ticker {
        let exchange = self.exchange.unwrap_or(ExchangeId::OKX);
        let symbol = self.symbol.unwrap_or_else(|| Symbol::new("BTC", "USDT"));
        let bid = self.bid.unwrap_or(Decimal::from(50000));
        let ask = self.ask.unwrap_or(Decimal::from(50010));
        let last = self.last_price.unwrap_or((bid + ask) / Decimal::from(2));

        Ticker::new(exchange, symbol, bid, ask, last)
    }
}

pub struct SignalBuilder {
    strategy_id: String,
    symbol: Option<Symbol>,
    legs: Vec<TradeLeg>,
    profit_bps: Option<i32>,
}

impl SignalBuilder {
    fn new() -> Self {
        Self {
            strategy_id: "test_strategy".to_string(),
            symbol: None,
            legs: Vec::new(),
            profit_bps: None,
        }
    }

    pub fn exchange(
        mut self,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
        buy_price: Decimal,
        sell_price: Decimal,
    ) -> Self {
        let symbol = self
            .symbol
            .clone()
            .unwrap_or_else(|| Symbol::new("BTC", "USDT"));
        self.legs.push(TradeLeg::new(
            buy_exchange,
            symbol.clone(),
            crate::types::Side::Buy,
            buy_price,
            Decimal::from(1),
        ));
        self.legs.push(TradeLeg::new(
            sell_exchange,
            symbol.clone(),
            crate::types::Side::Sell,
            sell_price,
            Decimal::from(1),
        ));
        self
    }

    pub fn symbol(mut self, base: impl Into<String>, quote: impl Into<String>) -> Self {
        self.symbol = Some(Symbol::new(base, quote));
        self
    }

    pub fn profit_bps(mut self, profit_bps: i32) -> Self {
        self.profit_bps = Some(profit_bps);
        self
    }

    pub fn build(self) -> RawSignal {
        let mut signal = RawSignal::new(
            self.strategy_id,
            self.symbol.unwrap_or_else(|| Symbol::new("BTC", "USDT")),
        );
        for leg in self.legs {
            signal.add_leg(leg);
        }
        if let Some(profit) = self.profit_bps {
            signal.set_profit_bps(profit);
        }
        signal
    }
}

pub struct MarketBundleBuilder {
    order_books: Vec<OrderBook>,
    tickers: Vec<Ticker>,
}

impl MarketBundleBuilder {
    fn new() -> Self {
        Self {
            order_books: Vec::new(),
            tickers: Vec::new(),
        }
    }

    pub fn exchange(mut self, exchange: ExchangeId) -> Self {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            exchange,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        self.order_books.push(order_book);
        self
    }

    pub fn symbol(mut self, base: impl Into<String>, quote: impl Into<String>) -> Self {
        let symbol = Symbol::new(base, quote);
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        self.order_books.push(order_book);
        self
    }

    pub fn order_book(mut self, order_book: OrderBook) -> Self {
        self.order_books.push(order_book);
        self
    }

    pub fn ticker(mut self, ticker: Ticker) -> Self {
        self.tickers.push(ticker);
        self
    }

    pub fn build(self) -> MarketBundle {
        let mut bundle = MarketBundle::new();
        for order_book in self.order_books {
            bundle.add_order_book(Arc::new(order_book));
        }
        for ticker in self.tickers {
            bundle.add_ticker(Arc::new(ticker));
        }
        bundle
    }
}

pub struct ConfigBuilder {
    fee: Option<Decimal>,
    min_size: Option<Decimal>,
    min_profit_bps: Option<i32>,
    max_exposure: Option<Decimal>,
}

impl ConfigBuilder {
    fn new() -> Self {
        Self {
            fee: None,
            min_size: None,
            min_profit_bps: None,
            max_exposure: None,
        }
    }

    pub fn fee(mut self, fee: Decimal) -> Self {
        self.fee = Some(fee);
        self
    }

    pub fn min_size(mut self, min_size: Decimal) -> Self {
        self.min_size = Some(min_size);
        self
    }

    pub fn build(self) -> crate::config::Config {
        let mut config = crate::config::Config::default();
        if let Some(fee) = self.fee {
            config.trading.slippage_buffer_percent = fee;
        }
        if let Some(min_size) = self.min_size {
            config.risk.min_order_size_usd = min_size;
        }
        if let Some(min_profit_bps) = self.min_profit_bps {
            config.trading.min_profit_threshold_percent =
                Decimal::from(min_profit_bps) / Decimal::from(100);
        }
        if let Some(max_exposure) = self.max_exposure {
            config.risk.max_position_size_usd = max_exposure;
        }
        config
    }
}

pub struct FilterContextBuilder {
    min_profit_bps: Option<i32>,
    max_spread_bps: Option<i32>,
    max_exposure: Option<Decimal>,
    allowed_exchanges: Option<Vec<ExchangeId>>,
}

impl FilterContextBuilder {
    fn new() -> Self {
        Self {
            min_profit_bps: None,
            max_spread_bps: None,
            max_exposure: None,
            allowed_exchanges: None,
        }
    }

    pub fn min_profit_bps(mut self, min_profit_bps: i32) -> Self {
        self.min_profit_bps = Some(min_profit_bps);
        self
    }

    pub fn max_spread_bps(mut self, max_spread_bps: i32) -> Self {
        self.max_spread_bps = Some(max_spread_bps);
        self
    }

    pub fn build(self) -> FilterContext {
        let min_profit = self.min_profit_bps.unwrap_or(10);
        let mut context = FilterContext::new(min_profit);
        if let Some(max_exposure) = self.max_exposure {
            context.max_exposure = max_exposure;
        }
        if let Some(allowed_exchanges) = self.allowed_exchanges {
            context.allowed_exchanges = allowed_exchanges;
        }
        context
    }
}
