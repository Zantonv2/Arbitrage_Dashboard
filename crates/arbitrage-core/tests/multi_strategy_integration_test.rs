use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
    config::Config,
    execution_preparer::{ExecutionConfig, ExecutionPreparer},
    normalizer::Normalizer,
    size_calculator::{SizeCalculator, SizeConfig},
    storage::{StorageConfig, StorageService},
    strategies::{
        CexArbitrageStrategy, ConvergenceArbitrageStrategy, CrossExchangeArbitrageStrategy,
        FundingRate, FundingRateArbitrageStrategy, HedgedFundingStrategy, LatencyArbitrageStrategy,
        MarketBundle, NewListingArbitrageStrategy, SpotPerpArbitrageStrategy,
        SpreadCaptureStrategy, StablecoinArbitrageStrategy, Strategy, StrategyRegistry, Ticker,
    },
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio;

async fn create_test_engine(database_path: &str) -> Result<ArbitrageEngine> {
    let mut config = Config::default();
    config.trading.min_profit_threshold_percent = Decimal::new(1, 4);
    config.risk.max_position_size_usd = Decimal::from(200000);

    let normalizer = Arc::new(Normalizer::new());
    let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
    let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
    let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));
    let storage_config = StorageConfig {
        database_path: database_path.to_string(),
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

    Ok(engine)
}

#[tokio::test]
async fn test_multi_strategy_detection() -> Result<()> {
    let mut registry = StrategyRegistry::new();

    registry.register(Arc::new(CexArbitrageStrategy::new()))?;
    registry.register(Arc::new(FundingRateArbitrageStrategy::new()))?;
    registry.register(Arc::new(StablecoinArbitrageStrategy::new()))?;
    registry.register(Arc::new(SpotPerpArbitrageStrategy::new()))?;
    registry.register(Arc::new(CrossExchangeArbitrageStrategy::new()))?;
    registry.register(Arc::new(NewListingArbitrageStrategy::new()))?;
    registry.register(Arc::new(LatencyArbitrageStrategy::new()))?;
    registry.register(Arc::new(ConvergenceArbitrageStrategy::new()))?;
    registry.register(Arc::new(SpreadCaptureStrategy::new()))?;
    registry.register(Arc::new(HedgedFundingStrategy::new()))?;

    println!("Registered {} strategies", registry.count());
    assert_eq!(registry.count(), 10);

    let mut market_bundle = MarketBundle::new();

    let btc_symbol = Symbol::new("BTC", "USDT");

    let btc_okx_book = OrderBook::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    let btc_bybit_book = OrderBook::new(
        ExchangeId::ByBit,
        btc_symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50150), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50160), Decimal::from(1))],
    );

    market_bundle.add_order_book(Arc::new(btc_okx_book));
    market_bundle.add_order_book(Arc::new(btc_bybit_book));

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::new(15, 5),
        Utc::now() + Duration::hours(6),
    );
    market_bundle.add_funding_rate(Arc::new(funding_rate));

    let btc_ticker = Ticker::new(
        ExchangeId::OKX,
        btc_symbol.clone(),
        Decimal::from(50000),
        Decimal::from(50010),
        Decimal::from(50005),
    );
    market_bundle.add_ticker(Arc::new(btc_ticker));

    let usdt_symbol = Symbol::new("USDT", "USD");
    let usdt_ticker = Ticker::new(
        ExchangeId::Kraken,
        usdt_symbol.clone(),
        Decimal::new(1003, 3),
        Decimal::new(1005, 3),
        Decimal::new(1004, 3),
    );
    let usdt_book = OrderBook::new(
        ExchangeId::Kraken,
        usdt_symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(1003, 3),
            Decimal::from(5000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(1005, 3),
            Decimal::from(5000),
        )],
    );
    market_bundle.add_ticker(Arc::new(usdt_ticker));
    market_bundle.add_order_book(Arc::new(usdt_book));

    let usdt_usdc_symbol = Symbol::new("USDT", "USDC");
    let usdt_usdc_okx = OrderBook::new(
        ExchangeId::OKX,
        usdt_usdc_symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(1002, 3),
            Decimal::from(2000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(1003, 3),
            Decimal::from(2000),
        )],
    );
    let usdt_usdc_bybit = OrderBook::new(
        ExchangeId::ByBit,
        usdt_usdc_symbol.clone(),
        vec![OrderBookLevel::new(
            Decimal::new(999, 3),
            Decimal::from(2000),
        )],
        vec![OrderBookLevel::new(
            Decimal::new(1000, 3),
            Decimal::from(2000),
        )],
    );
    market_bundle.add_order_book(Arc::new(usdt_usdc_okx));
    market_bundle.add_order_book(Arc::new(usdt_usdc_bybit));

    let strategies = registry.get_all();
    let mut total_signals = 0;

    for strategy in strategies {
        let signals = strategy.detect(&market_bundle)?;
        println!(
            "Strategy '{}' detected {} signals",
            strategy.name(),
            signals.len()
        );

        for (i, signal) in signals.iter().enumerate() {
            println!(
                "  Signal {}: {} legs, {}bps profit",
                i + 1,
                signal.legs.len(),
                signal.expected_profit_bps
            );
        }

        total_signals += signals.len();
    }

    println!("\nTotal signals across all strategies: {}", total_signals);

    assert!(
        total_signals > 0,
        "Should detect arbitrage opportunities across strategies"
    );

    Ok(())
}

#[tokio::test]
async fn test_strategy_priority_and_conflicts() -> Result<()> {
    let mut registry = StrategyRegistry::new();

    registry.register(Arc::new(CexArbitrageStrategy::new()))?;
    registry.register(Arc::new(CrossExchangeArbitrageStrategy::new()))?;

    let mut market_bundle = MarketBundle::new();
    let symbol = Symbol::new("ETH", "USDT");

    let eth_okx = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3000), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3005), Decimal::from(5))],
    );

    let eth_bybit = OrderBook::new(
        ExchangeId::ByBit,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(3050), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(3055), Decimal::from(5))],
    );

    market_bundle.add_order_book(Arc::new(eth_okx));
    market_bundle.add_order_book(Arc::new(eth_bybit));

    let cex_strategy = CexArbitrageStrategy::new();
    let cross_strategy = CrossExchangeArbitrageStrategy::new();

    let cex_signals = cex_strategy.detect(&market_bundle)?;
    let cross_signals = cross_strategy.detect(&market_bundle)?;

    println!("CEX arbitrage signals: {}", cex_signals.len());
    println!("Cross-exchange signals: {}", cross_signals.len());

    Ok(())
}

#[tokio::test]
async fn test_strategy_performance() -> Result<()> {
    let strategy = CexArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    for i in 0..100 {
        let symbol = Symbol::new(&format!("TOKEN{}", i), "USDT");

        for exchange in [
            ExchangeId::OKX,
            ExchangeId::ByBit,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
            ExchangeId::Kraken,
            ExchangeId::Bitstamp,
        ] {
            let base_price = 100 + i;
            let spread = 1 + (i % 5);

            let order_book = OrderBook::new(
                exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(
                    Decimal::from(base_price),
                    Decimal::from(10),
                )],
                vec![OrderBookLevel::new(
                    Decimal::from(base_price + spread),
                    Decimal::from(10),
                )],
            );

            market_bundle.add_order_book(Arc::new(order_book));
        }
    }

    println!(
        "Created market bundle with {} order books",
        market_bundle.order_books.len()
    );

    let start = std::time::Instant::now();
    let signals = strategy.detect(&market_bundle)?;
    let duration = start.elapsed();

    println!(
        "Strategy detection took {:?} for {} signals",
        duration,
        signals.len()
    );

    assert!(
        duration.as_millis() < 1000,
        "Strategy detection should be fast"
    );

    Ok(())
}

#[tokio::test]
async fn test_strategy_configuration() -> Result<()> {
    let mut strategy = CexArbitrageStrategy::new();

    let initial_config = strategy.config().clone();
    println!("Initial min profit: {}bps", initial_config.min_profit_bps);

    let mut new_config = initial_config.clone();
    new_config.min_profit_bps = 50;
    new_config.max_exposure = Decimal::from(5000);

    strategy.update_config(new_config)?;

    let updated_config = strategy.config();
    assert_eq!(updated_config.min_profit_bps, 50);
    assert_eq!(updated_config.max_exposure, Decimal::from(5000));

    println!("Updated min profit: {}bps", updated_config.min_profit_bps);

    Ok(())
}

#[tokio::test]
async fn test_strategy_error_handling() -> Result<()> {
    let strategy = CexArbitrageStrategy::new();

    let empty_bundle = MarketBundle::new();
    let signals = strategy.detect(&empty_bundle)?;

    assert_eq!(signals.len(), 0);

    let mut invalid_bundle = MarketBundle::new();
    let symbol = Symbol::new("INVALID", "TOKEN");

    let invalid_book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::ZERO, Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::ZERO, Decimal::from(1))],
    );

    invalid_bundle.add_order_book(Arc::new(invalid_book));

    let signals = strategy.detect(&invalid_bundle)?;
    println!("Signals from invalid data: {}", signals.len());

    Ok(())
}

#[tokio::test]
async fn test_engine_strategy_signal_flow() -> Result<()> {
    let engine = create_test_engine(":memory:").await?;

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
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;

    let signals = engine.detect_opportunities(&registry).await?;

    assert!(
        !signals.is_empty(),
        "Engine should detect signals through strategy"
    );

    let signal = &signals[0];
    assert_eq!(signal.symbol.to_pair(), "BTC/USDT");

    Ok(())
}

#[tokio::test]
async fn test_signal_execution_order_flow() -> Result<()> {
    use arbitrage_core::types::{Order, OrderType, TimeInForce};

    let engine = create_test_engine(":memory:").await?;

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
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;

    let signals = engine.detect_opportunities(&registry).await?;

    if !signals.is_empty() {
        let signal = &signals[0];

        let buy_order = Order::new(
            signal.buy_exchange,
            symbol.clone(),
            arbitrage_core::types::Side::Buy,
            OrderType::Market,
            signal.recommended_size,
            None,
        );

        let sell_order = Order::new(
            signal.sell_exchange,
            symbol.clone(),
            arbitrage_core::types::Side::Sell,
            OrderType::Market,
            signal.recommended_size,
            None,
        );

        assert_eq!(buy_order.exchange, signal.buy_exchange);
        assert_eq!(sell_order.exchange, signal.sell_exchange);
    }

    Ok(())
}

#[tokio::test]
async fn test_config_propagation() -> Result<()> {
    let mut registry = StrategyRegistry::new();

    let mut cex_strategy = CexArbitrageStrategy::new();
    let mut config = cex_strategy.config().clone();
    config.min_profit_bps = 100;
    cex_strategy.update_config(config)?;

    registry.register(Arc::new(cex_strategy))?;

    let strategies = registry.get_all();
    let cex = strategies
        .iter()
        .find(|s| s.name() == "CEX ↔ CEX Price Arbitrage")
        .expect("Should have CEX arbitrage strategy");

    let cex_config = cex.config();
    assert_eq!(cex_config.min_profit_bps, 100);

    Ok(())
}

#[tokio::test]
async fn test_multi_exchange_scenarios() -> Result<()> {
    let strategy = CexArbitrageStrategy::new();

    let exchanges = [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Bitstamp,
        ExchangeId::Kraken,
    ];

    for i in 0..exchanges.len() {
        for j in (i + 1)..exchanges.len() {
            let buy_exchange = exchanges[i];
            let sell_exchange = exchanges[j];

            let symbol = Symbol::new("BTC", "USDT");

            let buy_book = OrderBook::new(
                buy_exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
                vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
            );

            let sell_book = OrderBook::new(
                sell_exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(Decimal::from(50200), Decimal::from(1))],
                vec![OrderBookLevel::new(Decimal::from(50210), Decimal::from(1))],
            );

            let mut market_bundle = MarketBundle::new();
            market_bundle.add_order_book(Arc::new(buy_book));
            market_bundle.add_order_book(Arc::new(sell_book));

            let signals = strategy.detect(&market_bundle)?;

            if !signals.is_empty() {
                let signal = &signals[0];
                assert!(
                    signal.expected_profit_bps > 0,
                    "Should detect profit for {} -> {}",
                    buy_exchange,
                    sell_exchange
                );
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_end_to_end_pipeline() -> Result<()> {
    let engine = create_test_engine(":memory:").await?;

    let symbols = ["BTC", "ETH", "XRP", "ADA", "SOL"]
        .iter()
        .map(|s| Symbol::new(*s, "USDT"))
        .collect::<Vec<_>>();

    let exchanges = [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
    ];

    for (i, symbol) in symbols.iter().enumerate() {
        for (j, exchange) in exchanges.iter().enumerate() {
            let base_price = 1000 + i * 100 + j * 10;
            let spread = 5 + j;

            let book = OrderBook::new(
                *exchange,
                symbol.clone(),
                vec![OrderBookLevel::new(
                    Decimal::from(base_price),
                    Decimal::from(10),
                )],
                vec![OrderBookLevel::new(
                    Decimal::from(base_price + spread),
                    Decimal::from(10),
                )],
            );

            engine.update_order_book(book).await?;
        }
    }

    let mut registry = StrategyRegistry::new();
    registry.register(Arc::new(CexArbitrageStrategy::new()))?;
    registry.register(Arc::new(FundingRateArbitrageStrategy::new()))?;
    registry.register(Arc::new(StablecoinArbitrageStrategy::new()))?;

    let signals = engine.detect_opportunities(&registry).await?;

    println!("E2E Pipeline detected {} signals", signals.len());

    for signal in &signals {
        assert!(
            signal.net_profit_percent > Decimal::ZERO,
            "Signal should have positive profit"
        );
        assert!(
            signal.recommended_size > Decimal::ZERO,
            "Signal should have positive size"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_strategy_registry_operations() -> Result<()> {
    let mut registry = StrategyRegistry::new();

    assert_eq!(registry.count(), 0);
    assert!(registry.get_all().is_empty());

    registry.register(Arc::new(CexArbitrageStrategy::new()))?;
    assert_eq!(registry.count(), 1);

    registry.register(Arc::new(FundingRateArbitrageStrategy::new()))?;
    assert_eq!(registry.count(), 2);

    let strategies = registry.get_all();
    assert_eq!(strategies.len(), 2);

    let names: Vec<String> = strategies.iter().map(|s| s.name().to_string()).collect();
    assert!(names.contains(&"CEX ↔ CEX Price Arbitrage".to_string()));
    assert!(names.contains(&"Funding Rate Arbitrage".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_market_bundle_data_integrity() -> Result<()> {
    let mut market_bundle = MarketBundle::new();

    let symbol = Symbol::new("BTC", "USDT");

    let book = OrderBook::new(
        ExchangeId::OKX,
        symbol.clone(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
    );

    market_bundle.add_order_book(Arc::new(book));

    assert!(
        market_bundle.has_data(ExchangeId::OKX, &symbol),
        "Market bundle should have data for added orderbook"
    );

    let ticker = Ticker::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::from(50000),
        Decimal::from(50010),
        Decimal::from(50005),
    );

    market_bundle.add_ticker(Arc::new(ticker));

    let funding_rate = FundingRate::new(
        ExchangeId::OKX,
        symbol.clone(),
        Decimal::new(10, 4),
        Utc::now() + Duration::hours(8),
    );

    market_bundle.add_funding_rate(Arc::new(funding_rate));

    Ok(())
}

#[tokio::test]
async fn test_signal_validation() -> Result<()> {
    let strategy = CexArbitrageStrategy::new();

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

    let mut market_bundle = MarketBundle::new();
    market_bundle.add_order_book(Arc::new(okx_book));
    market_bundle.add_order_book(Arc::new(bybit_book));

    let signals = strategy.detect(&market_bundle)?;

    for signal in &signals {
        assert!(
            signal.expected_profit_bps > 0,
            "Signal profit should be positive"
        );
        if let Some(first_leg) = signal.legs.first() {
            assert!(
                first_leg.price > Decimal::ZERO,
                "Leg price should be positive"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_concurrent_strategy_execution() -> Result<()> {
    let strategy = CexArbitrageStrategy::new();

    let mut market_bundle = MarketBundle::new();

    for i in 0..50 {
        let symbol = Symbol::new(&format!("SYM{}", i), "USDT");

        let okx_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(
                Decimal::from(100 + i),
                Decimal::from(1),
            )],
            vec![OrderBookLevel::new(
                Decimal::from(105 + i),
                Decimal::from(1),
            )],
        );

        let bybit_book = OrderBook::new(
            ExchangeId::ByBit,
            symbol.clone(),
            vec![OrderBookLevel::new(
                Decimal::from(110 + i),
                Decimal::from(1),
            )],
            vec![OrderBookLevel::new(
                Decimal::from(115 + i),
                Decimal::from(1),
            )],
        );

        market_bundle.add_order_book(Arc::new(okx_book));
        market_bundle.add_order_book(Arc::new(bybit_book));
    }

    let start = std::time::Instant::now();
    let signals = strategy.detect(&market_bundle)?;
    let duration = start.elapsed();

    println!("Detected {} signals in {:?}", signals.len(), duration);

    assert!(
        duration.as_millis() < 500,
        "Strategy detection should complete quickly"
    );

    Ok(())
}
