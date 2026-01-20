use crate::{
    arbitrage_engine::ArbitrageEngine,
    confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
    config::Config,
    execution_preparer::{ExecutionConfig, ExecutionPreparer},
    normalizer::Normalizer,
    size_calculator::{SizeCalculator, SizeConfig},
    storage::{StorageConfig, StorageService},
    strategies::{FilterContext, MarketBundle},
    types::{ExchangeId, FeeSchedule, OrderBook, OrderBookLevel, Signal, Symbol},
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::broadcast;

pub mod fixtures;

pub fn create_test_engine(
    config: Config,
    database_path: &str,
) -> Result<(ArbitrageEngine, broadcast::Receiver<Signal>)> {
    let normalizer = Arc::new(Normalizer::new());
    let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
    let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
    let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));
    let storage_config = StorageConfig {
        database_path: database_path.to_string(),
        ..Default::default()
    };
    let runtime = tokio::runtime::Runtime::new()?;
    let storage = Arc::new(runtime.block_on(StorageService::new(storage_config))?);

    ArbitrageEngine::new(
        config,
        normalizer,
        confidence_scorer,
        size_calculator,
        execution_preparer,
        storage,
    )
}

pub fn create_test_market_bundle() -> MarketBundle {
    MarketBundle::new()
}

pub fn create_test_order_book(
    exchange: ExchangeId,
    symbol: Symbol,
    bids: Vec<(Decimal, Decimal)>,
    asks: Vec<(Decimal, Decimal)>,
) -> OrderBook {
    let order_bids: Vec<OrderBookLevel> = bids
        .into_iter()
        .map(|(price, qty)| OrderBookLevel::new(price, qty))
        .collect();
    let order_asks: Vec<OrderBookLevel> = asks
        .into_iter()
        .map(|(price, qty)| OrderBookLevel::new(price, qty))
        .collect();

    OrderBook::new(exchange, symbol, order_bids, order_asks)
}

pub fn create_test_signal(
    symbol: Symbol,
    buy_exchange: ExchangeId,
    sell_exchange: ExchangeId,
    buy_price: Decimal,
    sell_price: Decimal,
) -> Signal {
    let mut signal = Signal::new(symbol, buy_exchange, sell_exchange, buy_price, sell_price);

    let gross_profit = (sell_price - buy_price) / buy_price * Decimal::from(10000);
    signal.gross_profit_percent = gross_profit / Decimal::from(10000);
    signal.net_profit_percent = signal.gross_profit_percent;
    signal.net_profit_absolute = sell_price - buy_price;
    signal.confidence = Decimal::from(80);
    signal.recommended_size = Decimal::from(1);
    signal.max_size = Decimal::from(10);
    signal.expected_slippage = Decimal::from(1);
    signal.estimated_execution_time_ms = 100;

    signal
}

pub fn create_test_config() -> Config {
    let mut config = Config::default();
    config.trading.min_profit_threshold_percent = Decimal::new(1, 4);
    config.risk.max_position_size_usd = Decimal::from(200000);
    config
}

pub fn create_test_filter_context(min_profit_bps: i32) -> FilterContext {
    FilterContext::new(min_profit_bps)
}

pub fn create_test_fee_schedule(exchange: ExchangeId) -> FeeSchedule {
    FeeSchedule::new(exchange, Decimal::new(1, 4), Decimal::new(5, 4))
}

pub fn create_btc_usdt_order_books() -> (OrderBook, OrderBook) {
    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = create_test_order_book(
        ExchangeId::OKX,
        symbol.clone(),
        vec![(Decimal::from(50000), Decimal::from(1))],
        vec![(Decimal::from(50010), Decimal::from(1))],
    );

    let bybit_book = create_test_order_book(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![(Decimal::from(50200), Decimal::from(1))],
        vec![(Decimal::from(50210), Decimal::from(1))],
    );

    (okx_book, bybit_book)
}

pub fn create_eth_usdt_order_books() -> (OrderBook, OrderBook) {
    let symbol = Symbol::new("ETH", "USDT");

    let okx_book = create_test_order_book(
        ExchangeId::OKX,
        symbol.clone(),
        vec![(Decimal::from(3000), Decimal::from(10))],
        vec![(Decimal::from(3005), Decimal::from(10))],
    );

    let bybit_book = create_test_order_book(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![(Decimal::from(3050), Decimal::from(10))],
        vec![(Decimal::from(3055), Decimal::from(10))],
    );

    (okx_book, bybit_book)
}

pub fn create_no_arbitrage_order_books() -> (OrderBook, OrderBook) {
    let symbol = Symbol::new("ETH", "USDT");

    let okx_book = create_test_order_book(
        ExchangeId::OKX,
        symbol.clone(),
        vec![(Decimal::from(3000), Decimal::from(10))],
        vec![(Decimal::from(3010), Decimal::from(10))],
    );

    let bybit_book = create_test_order_book(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![(Decimal::from(3005), Decimal::from(10))],
        vec![(Decimal::from(3015), Decimal::from(10))],
    );

    (okx_book, bybit_book)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_order_book() {
        let symbol = Symbol::new("BTC", "USDT");
        let book = create_test_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            vec![(Decimal::from(50000), Decimal::from(1))],
            vec![(Decimal::from(50010), Decimal::from(1))],
        );

        assert_eq!(book.exchange, ExchangeId::OKX);
        assert_eq!(book.symbol.base, "BTC");
        assert_eq!(book.symbol.quote, "USDT");
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
    }

    #[test]
    fn test_create_test_signal() {
        let symbol = Symbol::new("BTC", "USDT");
        let signal = create_test_signal(
            symbol,
            ExchangeId::OKX,
            ExchangeId::ByBit,
            Decimal::from(50000),
            Decimal::from(50200),
        );

        assert_eq!(signal.symbol.base, "BTC");
        assert_eq!(signal.buy_exchange, ExchangeId::OKX);
        assert_eq!(signal.sell_exchange, ExchangeId::ByBit);
        assert!(signal.net_profit_percent > Decimal::ZERO);
    }

    #[test]
    fn test_create_filter_context() {
        let context = create_test_filter_context(10);
        assert_eq!(context.min_profit_bps, 10);
        assert!(context.allowed_exchanges.contains(&ExchangeId::OKX));
    }

    #[test]
    fn test_create_btc_usdt_order_books() {
        let (okx, bybit) = create_btc_usdt_order_books();

        assert_eq!(okx.exchange, ExchangeId::OKX);
        assert_eq!(bybit.exchange, ExchangeId::ByBit);
        assert!(okx.best_ask().unwrap().price < bybit.best_bid().unwrap().price);
    }
}
