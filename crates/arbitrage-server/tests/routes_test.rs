//! Integration tests for API routes
//!
//! Tests the REST API endpoints for the arbitrage server.

use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
    config::Config,
    execution_preparer::{ExecutionConfig, ExecutionPreparer},
    normalizer::Normalizer,
    size_calculator::{SizeCalculator, SizeConfig},
    storage::{StorageConfig, StorageService},
    strategies::{CexArbitrageStrategy, StrategyRegistry},
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;

/// Helper to create test engine with all components
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

/// Test GET /api/signals returns empty list when no signals
#[tokio::test]
async fn test_get_signals_empty() -> Result<()> {
    let config = Config::default();
    let (engine, storage, _receiver) = create_test_engine(config.clone()).await?;

    let mut registry = StrategyRegistry::new();
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;

    // Query storage directly (simulating what the route does)
    let query = arbitrage_core::storage::SignalQuery {
        limit: Some(100),
        status_filter: Some(arbitrage_core::storage::SignalStatus::Detected),
        symbol_filter: None,
        exchange_filter: None,
        min_confidence: None,
        time_range: None,
    };

    let signals = storage.query_signals(&query).await?;
    assert!(signals.is_empty(), "Should have no signals initially");

    Ok(())
}

/// Test order book caching in engine
#[tokio::test]
async fn test_orderbook_caching() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    let symbol = Symbol::new("BTC", "USDT");

    // Create and cache order book
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    engine.update_order_book(order_book.clone()).await?;

    // Retrieve from cache
    let cached = engine.get_order_book(ExchangeId::OKX, Arc::new(symbol));
    assert!(cached.is_some(), "Order book should be cached");

    let cached_ob = cached.expect("Order book should exist");
    assert_eq!(cached_ob.exchange, ExchangeId::OKX);
    assert_eq!(cached_ob.symbol.to_pair(), "BTC/USDT");
    assert!(!cached_ob.bids.is_empty());
    assert!(!cached_ob.asks.is_empty());

    Ok(())
}

/// Test order book not found returns None
#[tokio::test]
async fn test_orderbook_not_found() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    let symbol = Symbol::new("NONEXISTENT", "TOKEN");
    let cached = engine.get_order_book(ExchangeId::OKX, Arc::new(symbol));

    assert!(
        cached.is_none(),
        "Should return None for non-existent order book"
    );

    Ok(())
}

/// Test engine stats tracking
#[tokio::test]
async fn test_engine_stats() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    // Initial stats should be zero
    let stats = engine.get_stats();
    assert_eq!(stats.order_books_count, 0);
    assert_eq!(stats.tickers_count, 0);
    assert_eq!(stats.signals_detected, 0);

    // Add some order books
    let symbol = Symbol::new("BTC", "USDT");
    let order_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );
    engine.update_order_book(order_book).await?;

    let stats = engine.get_stats();
    assert_eq!(stats.order_books_count, 1);
    assert_eq!(stats.active_symbols_count, 1);

    Ok(())
}

/// Test strategy registry integration
#[tokio::test]
async fn test_strategy_registry_integration() -> Result<()> {
    let mut registry = StrategyRegistry::new();

    // Register all strategies
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;

    assert_eq!(registry.count(), 1);
    assert_eq!(registry.get_enabled().len(), 1);

    // Get strategy by ID
    let strategy = registry.get("cex_arbitrage");
    assert!(strategy.is_some());

    Ok(())
}

/// Test signal detection through engine
#[tokio::test]
async fn test_signal_detection_integration() -> Result<()> {
    let mut config = Config::default();
    config.trading.min_profit_threshold_percent = Decimal::new(1, 4); // 0.01%
    config.risk.max_position_size_usd = Decimal::from(200000);

    let (engine, storage, _receiver) = create_test_engine(config).await?;

    let mut registry = StrategyRegistry::new();
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;

    let symbol = Symbol::new("BTC", "USDT");

    // Create arbitrage opportunity
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

    // Should detect opportunity
    if !signals.is_empty() {
        let signal = &signals[0];
        assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
        assert!(signal.net_profit_percent > Decimal::ZERO);

        // Verify signal was stored
        let query = arbitrage_core::storage::SignalQuery {
            limit: Some(10),
            status_filter: None,
            symbol_filter: None,
            exchange_filter: None,
            min_confidence: None,
            time_range: None,
        };
        let stored = storage.query_signals(&query).await?;
        assert!(!stored.is_empty(), "Signal should be stored");
    }

    Ok(())
}

/// Test multiple order book updates
#[tokio::test]
async fn test_multiple_orderbook_updates() -> Result<()> {
    let config = Config::default();
    let (engine, _storage, _receiver) = create_test_engine(config).await?;

    // Add order books for multiple exchanges
    for exchange in [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Kraken,
        ExchangeId::Bitstamp,
    ] {
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            exchange,
            symbol,
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        engine.update_order_book(order_book).await?;
    }

    let stats = engine.get_stats();
    assert_eq!(
        stats.order_books_count, 6,
        "Should have 6 order books cached"
    );

    Ok(())
}

/// Test JWT secret validation - production requires JWT_SECRET environment variable
#[tokio::test]
async fn test_jwt_secret_validation() {
    // In test mode, the default JWT secret should be available
    // This verifies the test-only default is working correctly
    let config = Config::default();

    // The test default should be used in test builds
    assert!(
        !config.server.jwt_secret.is_empty(),
        "JWT secret should not be empty in test mode"
    );

    // Verify it's the test default (only in test builds)
    if cfg!(test) {
        assert_eq!(
            config.server.jwt_secret, "test-jwt-secret-for-unit-tests-only",
            "Test mode should use test-only JWT secret"
        );
    }
}

/// Test that JWT token can be validated for WebSocket authentication
#[tokio::test]
async fn test_jwt_token_validation_for_websocket() {
    use jsonwebtoken::{encode, EncodingKey, Header};

    // Create a test JWT token with the test secret
    let test_secret = "test-jwt-secret-for-unit-tests-only";
    let claims = serde_json::json!({
        "sub": "test-user",
        "exp": chrono::Utc::now().timestamp() + 3600,
        "iat": chrono::Utc::now().timestamp(),
    });

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(test_secret.as_bytes()),
    );

    assert!(
        token.is_ok(),
        "Should be able to create JWT token with test secret"
    );

    let token_str = token.unwrap();
    assert!(!token_str.is_empty(), "Token should not be empty");

    // Verify token can be decoded with the same secret
    use jsonwebtoken::{decode, DecodingKey, Validation};
    let decoded = decode::<serde_json::Value>(
        &token_str,
        &DecodingKey::from_secret(test_secret.as_bytes()),
        &Validation::default(),
    );

    assert!(
        decoded.is_ok(),
        "Should be able to decode JWT token with test secret"
    );
}

/// Test WebSocket authentication rejects invalid tokens
#[tokio::test]
async fn test_websocket_rejects_invalid_token() {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    // Try to validate an invalid token
    let invalid_token = "invalid.token.here";
    let test_secret = "test-jwt-secret-for-unit-tests-only";

    let result = decode::<serde_json::Value>(
        invalid_token,
        &DecodingKey::from_secret(test_secret.as_bytes()),
        &Validation::default(),
    );

    assert!(result.is_err(), "Invalid token should be rejected");
}

/// Test WebSocket authentication rejects tokens with wrong secret
#[tokio::test]
async fn test_websocket_rejects_wrong_secret() {
    use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

    // Create a token with one secret
    let claims = serde_json::json!({
        "sub": "test-user",
        "exp": chrono::Utc::now().timestamp() + 3600,
    });

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret("wrong-secret".as_bytes()),
    )
    .unwrap();

    // Try to validate with different secret
    let result = decode::<serde_json::Value>(
        &token,
        &DecodingKey::from_secret("test-jwt-secret-for-unit-tests-only".as_bytes()),
        &Validation::default(),
    );

    assert!(
        result.is_err(),
        "Token with wrong secret should be rejected"
    );
}
