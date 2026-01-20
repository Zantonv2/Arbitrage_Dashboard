use crate::{
    arbitrage_engine::ArbitrageEngine,
    confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
    config::Config,
    execution_preparer::{ExecutionConfig, ExecutionPreparer},
    normalizer::Normalizer,
    size_calculator::{SizeCalculator, SizeConfig},
    storage::{StorageConfig, StorageService},
    strategies::{FilterContext, MarketBundle, RawSignal, Ticker},
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;

pub mod fixtures;

pub use fixtures::*;

pub async fn create_test_engine(
    config: Config,
    database_path: &str,
) -> Result<(
    ArbitrageEngine,
    tokio::sync::broadcast::Receiver<crate::types::Signal>,
)> {
    let normalizer = Arc::new(Normalizer::new());
    let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
    let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
    let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));
    let storage_config = StorageConfig {
        database_path: database_path.to_string(),
        ..Default::default()
    };
    let storage = Arc::new(StorageService::new(storage_config).await?);

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

pub fn create_test_order_book(exchange: ExchangeId, symbol: &str) -> OrderBook {
    let symbol = Symbol::new(symbol.split('/').next().unwrap_or("BTC"), "USDT");
    OrderBook::new(
        exchange,
        symbol,
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    )
}

pub fn create_test_raw_signal() -> RawSignal {
    let symbol = Symbol::new("BTC", "USDT");
    let mut signal = RawSignal::new("test_strategy", symbol);
    signal.set_profit_bps(15);
    signal
}

pub fn create_test_config() -> Config {
    Config::default()
}

pub fn create_filter_context() -> FilterContext {
    FilterContext::new(10)
}

pub fn create_test_ticker(
    exchange: ExchangeId,
    symbol: &str,
    bid: Decimal,
    ask: Decimal,
) -> Ticker {
    let symbol = Symbol::new(symbol.split('/').next().unwrap_or("BTC"), "USDT");
    Ticker::new(exchange, symbol, bid, ask, (bid + ask) / Decimal::from(2))
}

pub fn create_test_order_book_with_spread(
    exchange: ExchangeId,
    symbol: &str,
    bid: Decimal,
    ask: Decimal,
) -> OrderBook {
    let symbol_parts: Vec<&str> = symbol.split('/').collect();
    let base = symbol_parts.first().copied().unwrap_or("BTC");
    let quote = symbol_parts.get(1).copied().unwrap_or("USDT");
    let symbol = Symbol::new(base, quote);
    OrderBook::new(
        exchange,
        symbol,
        vec![OrderBookLevel::new(bid, Decimal::from(1))],
        vec![OrderBookLevel::new(ask, Decimal::from(1))],
    )
}

pub fn create_test_market_bundle_with_order_books(
    exchanges: &[ExchangeId],
    symbol: &str,
) -> MarketBundle {
    let mut bundle = MarketBundle::new();
    let symbol_parts: Vec<&str> = symbol.split('/').collect();
    let base = symbol_parts.first().copied().unwrap_or("BTC");
    let quote = symbol_parts.get(1).copied().unwrap_or("USDT");
    let symbol_obj = Symbol::new(base, quote);

    for (i, &exchange) in exchanges.iter().enumerate() {
        let bid_price = Decimal::from(50000) + Decimal::from(i * 10);
        let ask_price = Decimal::from(50010) + Decimal::from(i * 10);
        let order_book = OrderBook::new(
            exchange,
            symbol_obj.clone(),
            vec![OrderBookLevel::new(bid_price, Decimal::from(1))],
            vec![OrderBookLevel::new(ask_price, Decimal::from(1))],
        );
        bundle.add_order_book(Arc::new(order_book));
    }

    bundle
}
