use arbitrage_core::{
    symbol_discovery::{MarketInfo, OrderBookDepth},
    symbol_manager::{
        CexArbitrageStrategy, ConvergenceArbitrageStrategy, FundingRateStrategy,
        LatencyArbitrageStrategy, NewListingArbitrageStrategy, SpotPerpetualStrategy,
        SpreadCaptureStrategy, StablecoinPegStrategy, SymbolManager, SymbolManagerConfig,
        SymbolStrategy,
    },
    types::{ExchangeId, Symbol},
    Result,
};
use chrono::Utc;
use rust_decimal::Decimal;

#[test]
fn test_symbol_manager_core_symbols() {
    let config = SymbolManagerConfig::default();
    let manager = SymbolManager::new(config.clone());

    let active_symbols = manager.get_active_symbols();

    // Should contain all core symbols
    for core_symbol in &config.core_symbols {
        assert!(active_symbols.contains(core_symbol));
    }

    // Should have exactly the core symbols initially
    assert_eq!(active_symbols.len(), config.core_symbols.len());
}

#[test]
fn test_cex_arbitrage_strategy() {
    let strategy = CexArbitrageStrategy;
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "BTC"), // Should be filtered out
        Symbol::new("SOL", "USDC"),
        Symbol::new("BNB", "BUSD"),
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 3);
    assert!(strategy_symbols.contains(&Symbol::new("BTC", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("SOL", "USDC")));
    assert!(strategy_symbols.contains(&Symbol::new("BNB", "BUSD")));

    let requirements = strategy.get_requirements();
    assert_eq!(requirements.min_exchanges, 2);
    assert!(!requirements.requires_perpetuals);
    assert!(!requirements.requires_funding_data);
    assert_eq!(strategy.get_name(), "CEX Arbitrage");
}

#[test]
fn test_funding_rate_strategy() {
    let strategy = FundingRateStrategy;
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("DOGE", "USDT"), // Should be filtered out (not in major list)
        Symbol::new("ETH", "USDC"),
        Symbol::new("SHIB", "USDT"), // Should be filtered out
        Symbol::new("LINK", "USDT"),
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 3);
    assert!(strategy_symbols.contains(&Symbol::new("BTC", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("ETH", "USDC")));
    assert!(strategy_symbols.contains(&Symbol::new("LINK", "USDT")));

    let requirements = strategy.get_requirements();
    assert!(requirements.requires_funding_data);
    assert!(requirements.requires_perpetuals);
    assert_eq!(requirements.min_volume_usd, Decimal::from(20_000_000));
    assert_eq!(strategy.get_name(), "Funding Rate Arbitrage");
}

#[test]
fn test_stablecoin_peg_strategy() {
    let strategy = StablecoinPegStrategy;
    let symbols = vec![
        Symbol::new("USDT", "USDC"),
        Symbol::new("BTC", "USDT"), // Should be filtered out
        Symbol::new("BUSD", "USDT"),
        Symbol::new("DAI", "USDC"),
        Symbol::new("ETH", "USDT"), // Should be filtered out
        Symbol::new("TUSD", "USDT"),
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 4);
    assert!(strategy_symbols.contains(&Symbol::new("USDT", "USDC")));
    assert!(strategy_symbols.contains(&Symbol::new("BUSD", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("DAI", "USDC")));
    assert!(strategy_symbols.contains(&Symbol::new("TUSD", "USDT")));

    let requirements = strategy.get_requirements();
    assert_eq!(requirements.max_spread_bps, 100); // 1% - wider for stablecoins
    assert_eq!(strategy.get_name(), "Stablecoin Peg Arbitrage");
}

#[test]
fn test_latency_arbitrage_strategy() {
    let strategy = LatencyArbitrageStrategy;
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDT"),
        Symbol::new("SOL", "USDT"),
        Symbol::new("MATIC", "USDT"), // Should be filtered out (not in ultra-liquid list)
        Symbol::new("BTC", "USDC"),   // Should be filtered out (not USDT)
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 3);
    assert!(strategy_symbols.contains(&Symbol::new("BTC", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("ETH", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("SOL", "USDT")));

    let requirements = strategy.get_requirements();
    assert_eq!(requirements.max_spread_bps, 5); // Ultra tight spreads
    assert_eq!(requirements.min_exchanges, 3);
    assert_eq!(requirements.min_volume_usd, Decimal::from(100_000_000));
    assert_eq!(strategy.get_name(), "Latency Arbitrage");
}

#[test]
fn test_spot_perpetual_strategy() {
    let strategy = SpotPerpetualStrategy;
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDC"),
        Symbol::new("DOGE", "USDT"), // Should be filtered out (not in major list)
        Symbol::new("SOL", "USDT"),
        Symbol::new("BTC", "BTC"), // Should be filtered out (wrong quote)
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 3);
    assert!(strategy_symbols.contains(&Symbol::new("BTC", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("ETH", "USDC")));
    assert!(strategy_symbols.contains(&Symbol::new("SOL", "USDT")));

    let requirements = strategy.get_requirements();
    assert!(requirements.requires_perpetuals);
    assert!(!requirements.requires_funding_data);
    assert_eq!(strategy.get_name(), "Spot-Perpetual Arbitrage");
}

#[test]
fn test_spread_capture_strategy() {
    let strategy = SpreadCaptureStrategy;
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDC"),
        Symbol::new("SOL", "USDT"),
        Symbol::new("MATIC", "USDT"), // Should be filtered out (not in top-5 liquid)
        Symbol::new("XRP", "USDT"),
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 4);
    assert!(strategy_symbols.contains(&Symbol::new("BTC", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("ETH", "USDC")));
    assert!(strategy_symbols.contains(&Symbol::new("SOL", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("XRP", "USDT")));

    let requirements = strategy.get_requirements();
    assert_eq!(requirements.max_spread_bps, 10); // Very tight spreads
    assert_eq!(requirements.min_exchanges, 3);
    assert_eq!(strategy.get_name(), "Spread Capture");
}

#[test]
fn test_convergence_arbitrage_strategy() {
    let strategy = ConvergenceArbitrageStrategy;
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDC"),
        Symbol::new("AVAX", "USDT"),
        Symbol::new("DOGE", "USDT"), // Should be filtered out
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 3);
    assert!(strategy_symbols.contains(&Symbol::new("BTC", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("ETH", "USDC")));
    assert!(strategy_symbols.contains(&Symbol::new("AVAX", "USDT")));

    let requirements = strategy.get_requirements();
    assert!(requirements.requires_perpetuals);
    assert!(!requirements.requires_funding_data);
    assert_eq!(strategy.get_name(), "Convergence Arbitrage");
}

#[test]
fn test_new_listing_arbitrage_strategy() {
    let strategy = NewListingArbitrageStrategy;
    let symbols = vec![
        Symbol::new("NEWCOIN", "USDT"),
        Symbol::new("ANOTHERCOIN", "USDC"),
        Symbol::new("OLDCOIN", "BTC"), // Should be filtered out (wrong quote)
    ];

    let strategy_symbols = strategy.get_strategy_symbols(&symbols);

    assert_eq!(strategy_symbols.len(), 2);
    assert!(strategy_symbols.contains(&Symbol::new("NEWCOIN", "USDT")));
    assert!(strategy_symbols.contains(&Symbol::new("ANOTHERCOIN", "USDC")));

    let requirements = strategy.get_requirements();
    assert_eq!(requirements.min_volume_usd, Decimal::from(100_000)); // Low volume for new listings
    assert_eq!(requirements.max_spread_bps, 500); // 5% - very wide spreads allowed
    assert_eq!(requirements.min_exchanges, 1); // Can work with single exchange
    assert_eq!(strategy.get_name(), "New Listing Arbitrage");
}

#[tokio::test]
async fn test_symbol_manager_discovery_integration() {
    let mut config = SymbolManagerConfig::default();
    config.enable_discovery = true;

    let mut manager = SymbolManager::new(config);

    // Create market info for a new symbol
    let new_symbol = Symbol::new("AVAX", "USDT");
    let market_info = MarketInfo {
        symbol: new_symbol.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(10_000_000), // $10M volume
        price_usd: Decimal::from(35),
        spread_bps: 25, // 0.25% spread
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(50_000),
            depth_01_percent_usd: Decimal::from(200_000),
            depth_05_percent_usd: Decimal::from(1_000_000),
            max_order_size_usd: Decimal::from(100_000),
        },
    };

    // Update market data
    let result: Result<()> = manager.update_market_data(market_info.clone()).await;
    assert!(result.is_ok());

    // Symbol should not be active yet (needs 2+ exchanges)
    assert!(!manager.is_symbol_active(&new_symbol));

    // Add second exchange
    let market_info2 = MarketInfo {
        exchange: ExchangeId::BingX,
        price_usd: Decimal::new(3510, 2), // 35.10 - Slight price difference
        ..market_info.clone()
    };

    let result: Result<()> = manager.update_market_data(market_info2).await;
    assert!(result.is_ok());

    // Force refresh to trigger discovery
    let result: Result<()> = manager.refresh_discovered_symbols().await;
    assert!(result.is_ok());

    // Now symbol should be active
    assert!(manager.is_symbol_active(&new_symbol));

    // Should appear in active symbols list
    let active_symbols = manager.get_active_symbols();
    assert!(active_symbols.contains(&new_symbol));
}

#[test]
fn test_strategy_requirements_validation() {
    let strategies: Vec<Box<dyn SymbolStrategy>> = vec![
        Box::new(CexArbitrageStrategy),
        Box::new(FundingRateStrategy),
        Box::new(StablecoinPegStrategy),
        Box::new(LatencyArbitrageStrategy),
        Box::new(SpotPerpetualStrategy),
        Box::new(SpreadCaptureStrategy),
        Box::new(ConvergenceArbitrageStrategy),
        Box::new(NewListingArbitrageStrategy),
    ];

    for strategy in strategies {
        let requirements = strategy.get_requirements();

        // All strategies should have reasonable requirements
        assert!(requirements.min_volume_usd > Decimal::ZERO);
        assert!(requirements.max_spread_bps > 0);
        assert!(requirements.min_exchanges > 0);
        assert!(!requirements.quote_currencies.is_empty());

        // Strategy name should not be empty
        assert!(!strategy.get_name().is_empty());

        println!(
            "✅ {} - Volume: ${}, Spread: {}bps, Exchanges: {}",
            strategy.get_name(),
            requirements.min_volume_usd,
            requirements.max_spread_bps,
            requirements.min_exchanges
        );
    }
}
