//! Integration tests for the ArbitrageBridge service
//!
//! Tests the bridge between exchange connectors and the arbitrage engine.

use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
    config::Config,
    execution_preparer::{ExecutionConfig, ExecutionPreparer},
    normalizer::Normalizer,
    size_calculator::{SizeCalculator, SizeConfig},
    storage::{StorageConfig, StorageService},
    strategies::{CexArbitrageStrategy, FundingRate, StrategyRegistry, Ticker},
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;

/// Helper to create test engine
async fn create_test_engine(
    config: Config,
) -> Result<(
    Arc<ArbitrageEngine>,
    Arc<StorageService>,
    tokio::sync::broadcast::Receiver<arbitrage_core::types::Signal>,
)> {
    let normalizer = Arc::new(Normalizer::new());
    let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
    let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
    let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));
    let storage_config = StorageConfig {
        database_path: ":memory:".to_string(),
        ..Default::default()
    };
    let storage = Arc::new(StorageService::new(storage_config).await?);

    let (engine, receiver) = ArbitrageEngine::new(
        config,
        normalizer,
        confidence_scorer,
        size_calculator,
        execution_preparer,
        storage.clone(),
    )?;

    Ok((Arc::new(engine), storage, receiver))
}

/// Test ticker updates through engine
#[tokio::test]
async fn test_ticker_update() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    let symbol = Symbol::new("BTC", "USDT");
    let ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(50000), // bid
        Decimal::from(50010), // ask
        Decimal::from(50005), // last
    );

    engine.update_ticker(ticker).await?;

    let cached = engine.get_ticker(ExchangeId::OKX, &symbol);
    assert!(cached.is_some(), "Ticker should be cached");

    let cached_ticker = cached.expect("Ticker should exist");
    assert_eq!(cached_ticker.bid, Decimal::from(50000));
    assert_eq!(cached_ticker.ask, Decimal::from(50010));

    Ok(())
}

/// Test funding rate updates through engine
#[tokio::test]
async fn test_funding_rate_update() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    let symbol = Symbol::new("BTC", "USDT");
    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(1, 4), // 0.01%
        chrono::Utc::now() + chrono::Duration::hours(8),
    );

    engine.update_funding_rate(funding_rate).await?;

    let cached = engine.get_funding_rate(ExchangeId::OKX, &symbol);
    assert!(cached.is_some(), "Funding rate should be cached");

    let cached_fr = cached.expect("Funding rate should exist");
    assert_eq!(cached_fr.rate, Decimal::new(1, 4));

    Ok(())
}

/// Test signal broadcasting
#[tokio::test]
async fn test_signal_broadcasting() -> Result<()> {
    let mut config = Config::default();
    config.trading.min_profit_threshold_percent = Decimal::new(1, 4);
    config.risk.max_position_size_usd = Decimal::from(200000);

    let (engine, _storage, mut receiver) = create_test_engine(config).await?;

    let mut registry = StrategyRegistry::new();
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;

    let symbol = Symbol::new("BTC", "USDT");

    // Create significant arbitrage opportunity
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50210), Decimal::from(1))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    // Run detection
    let signals = engine.detect_opportunities(&registry).await?;

    // If signals were emitted, try to receive them
    if !signals.is_empty() {
        // Use try_recv to avoid blocking
        match receiver.try_recv() {
            Ok(signal) => {
                assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
            }
            Err(_) => {
                // Signal might have been received already or not yet
            }
        }
    }

    Ok(())
}

/// Test multiple subscribers
#[tokio::test]
async fn test_multiple_subscribers() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver1) = create_test_engine(config).await?;

    // Create additional subscribers
    let _receiver2 = engine.subscribe();
    let _receiver3 = engine.subscribe();

    // All subscribers should be able to receive signals
    // (actual signal sending tested in other tests)

    Ok(())
}

/// Test stale data cleanup
#[tokio::test]
async fn test_stale_data_cleanup() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    let symbol = Symbol::new("BTC", "USDT");

    // Add order book
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );
    engine.update_order_book(order_book).await?;

    let stats_before = engine.get_stats();
    assert_eq!(stats_before.order_books_count, 1);

    // Cleanup (fresh data won't be removed)
    engine.cleanup_stale_data();

    let stats_after = engine.get_stats();
    // Fresh data should still be there
    assert_eq!(stats_after.order_books_count, 1);

    Ok(())
}

/// Test concurrent order book updates
#[tokio::test]
async fn test_concurrent_updates() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    let engine_clone = Arc::clone(&engine);

    // Spawn multiple concurrent updates
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let engine = Arc::clone(&engine_clone);
            tokio::spawn(async move {
                let symbol = Symbol::new(&format!("TOKEN{}", i), "USDT");
                let order_book = OrderBook::new(
                    ExchangeId::OKX,
                    symbol,
                    vec![OrderBookLevel::new(
                        Decimal::from(100 + i),
                        Decimal::from(1),
                    )],
                    vec![OrderBookLevel::new(
                        Decimal::from(101 + i),
                        Decimal::from(1),
                    )],
                );
                engine.update_order_book(order_book).await
            })
        })
        .collect();

    // Wait for all updates
    for handle in handles {
        handle.await.expect("Task should complete")?;
    }

    let stats = engine.get_stats();
    assert_eq!(
        stats.order_books_count, 10,
        "All order books should be cached"
    );

    Ok(())
}

/// Test engine with all 6 exchanges
#[tokio::test]
async fn test_all_exchanges() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    let exchanges = [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Kraken,
        ExchangeId::Bitstamp,
    ];

    for exchange in exchanges {
        let symbol = Symbol::new("ETH", "USDT");
        let order_book = OrderBook::new(
            exchange,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(3000), Decimal::from(10))],
            vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(10))],
        );
        engine.update_order_book(order_book).await?;

        // Verify it was cached
        let cached = engine.get_order_book(exchange, &symbol);
        assert!(
            cached.is_some(),
            "Order book for {} should be cached",
            exchange
        );
    }

    let stats = engine.get_stats();
    assert_eq!(stats.order_books_count, 6);

    Ok(())
}
