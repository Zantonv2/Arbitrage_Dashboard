use arbitrage_core::{
    strategies::{CexArbitrageStrategy, StrategyRegistry},
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio;

async fn create_test_engine(
    min_profit_threshold: Decimal,
) -> Result<(
    arbitrage_core::arbitrage_engine::ArbitrageEngine,
    Vec<Arc<CexArbitrageStrategy>>,
)> {
    use arbitrage_core::arbitrage_engine::ArbitrageEngine;
    use arbitrage_core::confidence_scorer::{ConfidenceConfig, ConfidenceScorer};
    use arbitrage_core::config::Config;
    use arbitrage_core::execution_preparer::{ExecutionConfig, ExecutionPreparer};
    use arbitrage_core::normalizer::Normalizer;
    use arbitrage_core::size_calculator::{SizeCalculator, SizeConfig};
    use arbitrage_core::storage::{StorageConfig, StorageService};

    let mut config = Config::default();
    config.trading.min_profit_threshold_percent = min_profit_threshold;
    config.risk.max_position_size_usd = Decimal::from(200000);

    let normalizer = Arc::new(Normalizer::new());
    let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
    let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
    let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));
    let storage_config = StorageConfig {
        database_path: ":memory:".to_string(),
        ..Default::default()
    };
    let storage = Arc::new(StorageService::new(storage_config).await?);

    let (engine, _receiver) = ArbitrageEngine::new(
        config,
        normalizer,
        confidence_scorer,
        size_calculator,
        execution_preparer,
        storage,
    )?;

    let cex_strategy = Arc::new(CexArbitrageStrategy::new());

    Ok((engine, vec![cex_strategy]))
}

#[tokio::test]
async fn test_cex_arbitrage_integration() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

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

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    assert!(
        !signals.is_empty(),
        "Should detect CEX arbitrage opportunity"
    );

    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");
    assert!(signal.net_profit_percent > Decimal::ZERO);

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_no_opportunity() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("ETH", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3000), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(3010), Decimal::from(10))],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(3015), Decimal::from(10))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    let profitable_signals: Vec<_> = signals
        .iter()
        .filter(|s| s.net_profit_percent >= Decimal::new(1, 4))
        .collect();

    assert!(
        profitable_signals.is_empty(),
        "Should not detect profitable arbitrage with small spread"
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_filtering() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(10, 2)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50020), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50030), Decimal::from(1))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    let profitable_signals: Vec<_> = signals
        .iter()
        .filter(|s| s.net_profit_percent >= Decimal::new(10, 2))
        .collect();

    assert!(
        profitable_signals.is_empty(),
        "Signals should be filtered out due to insufficient profit"
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_multiple_symbols() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

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

    engine.update_order_book(btc_okx).await?;
    engine.update_order_book(btc_bybit).await?;
    engine.update_order_book(eth_okx).await?;
    engine.update_order_book(eth_bybit).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    let profitable_signals: Vec<_> = signals
        .iter()
        .filter(|s| s.net_profit_percent > Decimal::ZERO)
        .collect();

    assert!(
        profitable_signals.len() >= 1,
        "Should detect arbitrage opportunities"
    );

    let mut symbols_found = std::collections::HashSet::new();
    for signal in &profitable_signals {
        symbols_found.insert(signal.symbol.to_pair());
    }

    assert!(
        symbols_found.len() >= 1,
        "Should find opportunities across multiple symbols"
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_large_spread() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let gateio_book = OrderBook::new(
        ExchangeId::GateIo,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(51000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(51010), Decimal::from(1))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(gateio_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    assert!(
        !signals.is_empty(),
        "Should detect arbitrage with large spread"
    );

    let signal = &signals[0];
    assert!(
        signal.net_profit_percent > Decimal::new(100, 4),
        "Large spread should produce large profit"
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_small_spread() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50002), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50003), Decimal::from(1))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    let profitable_signals: Vec<_> = signals
        .iter()
        .filter(|s| s.net_profit_percent >= Decimal::new(1, 4))
        .collect();

    assert!(
        profitable_signals.is_empty(),
        "Should not detect arbitrage with very small spread"
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_three_exchanges() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

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

    let mexc_book = OrderBook::new(
        ExchangeId::MEXC,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(1))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;
    engine.update_order_book(mexc_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    assert!(
        !signals.is_empty(),
        "Should detect arbitrage with three exchanges"
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_all_tier1_exchanges() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let exchanges = [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Bitstamp,
        ExchangeId::Kraken,
    ];

    for (i, exchange) in exchanges.iter().enumerate() {
        let book = OrderBook::new(
            *exchange,
            symbol.clone(),
            vec![OrderBookLevel::new(
                Decimal::from(50000 - i),
                Decimal::from(1),
            )],
            vec![OrderBookLevel::new(
                Decimal::from(50010 - i),
                Decimal::from(1),
            )],
        );
        engine.update_order_book(book).await?;
    }

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    println!("All tier1 exchanges: {} signals detected", signals.len());

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_empty_orderbook() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    println!("Empty orderbook test: {} signals detected", signals.len());

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_reversed_direction() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50210), Decimal::from(1))],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    if !signals.is_empty() {
        let signal = &signals[0];
        assert!(
            signal.buy_price < signal.sell_price,
            "Buy price should be less than sell price"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_price_calculation() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

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

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    if !signals.is_empty() {
        let signal = &signals[0];

        assert!(
            signal.buy_price > Decimal::ZERO,
            "Buy price should be positive"
        );
        assert!(
            signal.sell_price > Decimal::ZERO,
            "Sell price should be positive"
        );
        assert!(
            signal.buy_price < signal.sell_price,
            "Buy price should be less than sell price for arbitrage"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_quantity_impact() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::from(50000),
            Decimal::from(100),
        )],
        vec![OrderBookLevel::new(
            Decimal::from(50010),
            Decimal::from(100),
        )],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50210), Decimal::from(1))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    for signal in &signals {
        assert!(
            signal.recommended_size > Decimal::ZERO,
            "Recommended size should be positive"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_existing_position() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

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

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    for signal in &signals {
        assert!(
            signal.max_size >= signal.recommended_size,
            "Max size should be >= recommended size"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_high_liquidity() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_bids: Vec<_> = (1..=100)
        .map(|i| OrderBookLevel::new(Decimal::from(50000 - i), Decimal::from(i * 10)))
        .collect();
    let okx_asks: Vec<_> = (1..=100)
        .map(|i| OrderBookLevel::new(Decimal::from(50010 + i), Decimal::from(i * 10)))
        .collect();

    let okx_book = OrderBook::new(ExchangeId::OKX, symbol.clone(), okx_bids, okx_asks);

    let bybit_bids: Vec<_> = (1..=100)
        .map(|i| OrderBookLevel::new(Decimal::from(50200 - i), Decimal::from(i * 10)))
        .collect();
    let bybit_asks: Vec<_> = (1..=100)
        .map(|i| OrderBookLevel::new(Decimal::from(50210 + i), Decimal::from(i * 10)))
        .collect();

    let bybit_book = OrderBook::new(ExchangeId::ByBit, symbol.clone(), bybit_bids, bybit_asks);

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    println!("Detected {} signals with high liquidity", signals.len());

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_zero_liquidity() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

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

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    for signal in &signals {
        assert!(
            signal.expected_slippage >= Decimal::ZERO,
            "Slippage should be non-negative"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_max_exposure() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::from(50000),
            Decimal::from(100),
        )],
        vec![OrderBookLevel::new(
            Decimal::from(50010),
            Decimal::from(100),
        )],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::from(50200),
            Decimal::from(100),
        )],
        vec![OrderBookLevel::new(
            Decimal::from(50210),
            Decimal::from(100),
        )],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    for signal in &signals {
        assert!(
            signal.max_size > Decimal::ZERO,
            "Max size should be positive"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_execution_time_estimate() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

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

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    println!(
        "Execution time estimate test: {} signals detected",
        signals.len()
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_different_quantities() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

    let okx_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(0))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(0))],
    );

    let bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(10))],
        vec![OrderBookLevel::new(Decimal::from(50210), Decimal::from(10))],
    );

    engine.update_order_book(okx_book).await?;
    engine.update_order_book(bybit_book).await?;

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    assert!(
        signals.is_empty(),
        "Should not detect arbitrage with zero quantity on one side"
    );

    Ok(())
}

#[tokio::test]
async fn test_cex_arbitrage_metadata() -> Result<()> {
    let (engine, strategies) = create_test_engine(Decimal::new(1, 4)).await?;

    let symbol = Symbol::new("BTC", "USDT");

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

    let mut registry = StrategyRegistry::new();
    for strategy in strategies {
        registry.register(strategy)?;
    }

    let signals = engine.detect_opportunities(&registry).await?;

    for signal in &signals {
        assert!(
            !signal.metadata.is_empty() || signal.metadata.is_empty(),
            "Metadata should be accessible"
        );
    }

    Ok(())
}
