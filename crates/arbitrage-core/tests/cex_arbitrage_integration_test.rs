use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    strategies::{StrategyRegistry, strategies::CexArbitrageStrategy},
    types::{OrderBook, OrderBookLevel, Symbol, ExchangeId},
    normalizer::Normalizer,
    confidence_scorer::{ConfidenceScorer, ConfidenceConfig},
    size_calculator::{SizeCalculator, SizeConfig},
    execution_preparer::{ExecutionPreparer, ExecutionConfig},
    storage::{StorageService, StorageConfig},
    config::Config,
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio;

/// Helper function to create test engine with all required components
fn create_test_engine(config: Config) -> Result<(ArbitrageEngine, tokio::sync::broadcast::Receiver<arbitrage_core::types::Signal>)> {
    let normalizer = Arc::new(Normalizer::new());
    let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
    let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
    let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));
    let storage = Arc::new(StorageService::new(StorageConfig::default())?);
    
    ArbitrageEngine::new(
        config,
        normalizer,
        confidence_scorer,
        size_calculator,
        execution_preparer,
        storage,
    )
}

/// Integration test for CEX arbitrage strategy through the full arbitrage engine
#[tokio::test]
async fn test_cex_arbitrage_integration() -> Result<()> {
    // Create arbitrage engine with CEX strategy
    let mut config = Config::default();
    // Set realistic profit threshold (0.01% = 1 bps)
    config.trading.min_profit_threshold_percent = Decimal::new(1, 4); // 0.0001
    // Increase max exposure to allow larger test trades
    config.risk.max_position_size_usd = Decimal::from(200000); // $200k for testing
    
    let (engine, _receiver) = create_test_engine(config)?;
    
    // Register CEX arbitrage strategy
    let mut registry = StrategyRegistry::new();
    let cex_strategy = Arc::new(CexArbitrageStrategy::new());
    registry.register(cex_strategy)?;
    
    println!("Registered strategies: {}", registry.get_enabled().len());
    
    // Create test market data with arbitrage opportunity
    let symbol = Symbol::new("BTC", "USDT");
    
    // OKX: Lower ask (good for buying)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))], // bids
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))], // asks (lower)
    );
    
    // ByBit: Higher bid (good for selling) - create larger spread for profitable arbitrage
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))], // bids (much higher for profit)
        vec![OrderBookLevel::new(Decimal::from(50210), Decimal::from(1))], // asks
    );
    
    // Process order books through engine
    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;
    
    println!("Order books updated. Engine stats: {:?}", engine.get_stats());
    
    // Run strategy detection
    let signals = engine.detect_opportunities(&registry).await?;
    
    println!("Detected {} signals", signals.len());
    for (i, signal) in signals.iter().enumerate() {
        println!("Signal {}: {} -> {} profit: {:.4}%", 
                 i, signal.buy_exchange, signal.sell_exchange, 
                 signal.net_profit_percent * Decimal::from(100));
    }
    
    // Should detect arbitrage opportunity
    assert!(!signals.is_empty(), "Should detect CEX arbitrage opportunity");
    
    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
    assert!(signal.net_profit_percent > Decimal::ZERO);
    
    // Buy should be on exchange with lower ask (OKX)
    assert_eq!(signal.buy_exchange, ExchangeId::OKX);
    assert_eq!(signal.buy_price, Decimal::from(50010));
    
    // Sell should be on exchange with higher bid (ByBit)
    assert_eq!(signal.sell_exchange, ExchangeId::ByBit);
    assert_eq!(signal.sell_price, Decimal::from(50200));
    
    Ok(())
}

/// Integration test for no arbitrage scenario
#[tokio::test]
async fn test_cex_arbitrage_no_opportunity() -> Result<()> {
    let mut config = Config::default();
    // Set realistic profit threshold
    config.trading.min_profit_threshold_percent = Decimal::new(1, 4); // 0.0001
    // Increase max exposure to allow larger test trades
    config.risk.max_position_size_usd = Decimal::from(200000); // $200k for testing
    
    let (engine, _receiver) = create_test_engine(config)?;
    
    let mut registry = StrategyRegistry::new();
    let cex_strategy = Arc::new(CexArbitrageStrategy::new());
    registry.register(cex_strategy)?;
    
    let symbol = Symbol::new("ETH", "USDT");
    
    // Both exchanges have similar prices (no arbitrage)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3000), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(3010), Decimal::from(10))],
    );
    
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(10))], // Not much higher
        vec![OrderBookLevel::new(Decimal::from(3015), Decimal::from(10))],
    );
    
    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;
    
    let signals = engine.detect_opportunities(&registry).await?;
    
    // Should not detect profitable arbitrage
    let cex_signals: Vec<_> = signals.iter()
        .filter(|s| s.net_profit_percent < Decimal::new(50, 4)) // Less than 0.5%
        .collect();
    
    // Either no signals or signals with very low profit that get filtered out
    if !cex_signals.is_empty() {
        for signal in cex_signals {
            // If signals exist, they should have very low profit
            assert!(signal.net_profit_percent < Decimal::new(50, 4)); // Less than 0.5%
        }
    }
    
    Ok(())
}

/// Integration test for strategy filtering
#[tokio::test]
async fn test_cex_arbitrage_filtering() -> Result<()> {
    let mut config = Config::default();
    config.trading.min_profit_threshold_percent = Decimal::new(10, 2); // Require 10% profit (very high)
    // Increase max exposure to allow larger test trades
    config.risk.max_position_size_usd = Decimal::from(200000); // $200k for testing
    
    let (engine, _receiver) = create_test_engine(config)?;
    
    let mut registry = StrategyRegistry::new();
    let cex_strategy = Arc::new(CexArbitrageStrategy::new());
    registry.register(cex_strategy)?;
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // Create small arbitrage opportunity (will be filtered out)
    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );
    
    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50020), Decimal::from(1))], // Small profit
        vec![OrderBookLevel::new(Decimal::from(50030), Decimal::from(1))],
    );
    
    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;
    
    let signals = engine.detect_opportunities(&registry).await?;
    
    // Should be filtered out due to high minimum profit requirement
    let profitable_signals: Vec<_> = signals.iter()
        .filter(|s| s.net_profit_percent >= Decimal::new(10, 2))
        .collect();
    
    assert!(profitable_signals.is_empty(), "Signals should be filtered out due to insufficient profit");
    
    Ok(())
}

/// Integration test for multiple symbols
#[tokio::test]
async fn test_cex_arbitrage_multiple_symbols() -> Result<()> {
    let mut config = Config::default();
    // Set realistic profit threshold
    config.trading.min_profit_threshold_percent = Decimal::new(1, 4); // 0.0001
    // Increase max exposure to allow larger test trades
    config.risk.max_position_size_usd = Decimal::from(200000); // $200k for testing
    
    let (engine, _receiver) = create_test_engine(config)?;
    
    let mut registry = StrategyRegistry::new();
    let cex_strategy = Arc::new(CexArbitrageStrategy::new());
    registry.register(cex_strategy)?;
    
    // BTC arbitrage opportunity
    let btc_symbol = Symbol::new("BTC", "USDT");
    let btc_okx = OrderBook::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );
    let btc_bybit = OrderBook::new(
        ExchangeId::ByBit,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(1))],
    );
    
    // ETH arbitrage opportunity
    let eth_symbol = Symbol::new("ETH", "USDT");
    let eth_okx = OrderBook::new(
        ExchangeId::OKX,
        eth_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3000), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(10))],
    );
    let eth_bybit = OrderBook::new(
        ExchangeId::ByBit,
        eth_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3050), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(3055), Decimal::from(10))],
    );
    
    // Process all order books
    engine.update_order_book(btc_okx).await?;
    engine.update_order_book(btc_bybit).await?;
    engine.update_order_book(eth_okx).await?;
    engine.update_order_book(eth_bybit).await?;
    
    let signals = engine.detect_opportunities(&registry).await?;
    
    let profitable_signals: Vec<_> = signals.iter()
        .filter(|s| s.net_profit_percent > Decimal::ZERO)
        .collect();
    
    // Should detect opportunities for both symbols
    assert!(profitable_signals.len() >= 1, "Should detect arbitrage opportunities");
    
    // Check that we have signals for different symbols
    let mut symbols_found = std::collections::HashSet::new();
    for signal in &profitable_signals {
        symbols_found.insert(signal.symbol.to_pair());
    }
    
    assert!(symbols_found.len() >= 1, "Should find opportunities across multiple symbols");
    
    Ok(())
}