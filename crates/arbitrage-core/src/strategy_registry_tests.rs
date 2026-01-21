#[cfg(test)]
mod tests {
    use crate::strategies::registry::StrategyRegistry;
    use crate::strategies::{
        CexArbitrageStrategy, ConvergenceArbitrageStrategy, CrossExchangeArbitrageStrategy,
        FundingRateArbitrageStrategy, HedgedFundingStrategy, LatencyArbitrageStrategy,
        NewListingArbitrageStrategy, SpotPerpArbitrageStrategy, SpreadCaptureStrategy,
        StablecoinArbitrageStrategy, Strategy,
    };
    use rust_decimal::Decimal;
    use std::sync::Arc;

    #[test]
    fn test_strategy_registry_new() {
        let registry = StrategyRegistry::new();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_strategy_registry_default() {
        let registry = StrategyRegistry::default();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_strategy_registry_register_single() {
        let mut registry = StrategyRegistry::new();
        let strategy = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;

        let result = registry.register(strategy);
        assert!(result.is_ok());
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_strategy_registry_register_duplicate_fails() {
        let mut registry = StrategyRegistry::new();
        let strategy1 = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        let strategy2 = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;

        let result1 = registry.register(strategy1);
        assert!(result1.is_ok());

        let result2 = registry.register(strategy2);
        assert!(result2.is_err());
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_strategy_registry_get() {
        let mut registry = StrategyRegistry::new();
        let strategy = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        registry.register(strategy).unwrap();

        let retrieved = registry.get("convergence_arbitrage");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), "convergence_arbitrage");
    }

    #[test]
    fn test_strategy_registry_get_nonexistent() {
        let registry = StrategyRegistry::new();
        let retrieved = registry.get("nonexistent_strategy");
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_strategy_registry_get_all() {
        let mut registry = StrategyRegistry::new();
        let strat1 = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        let strat2 = Arc::new(SpotPerpArbitrageStrategy::new()) as Arc<dyn Strategy>;

        registry.register(strat1).unwrap();
        registry.register(strat2).unwrap();

        let all = registry.get_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_strategy_registry_get_enabled() {
        let mut registry = StrategyRegistry::new();
        let strat1 = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        let strat2 = Arc::new(SpotPerpArbitrageStrategy::new()) as Arc<dyn Strategy>;

        registry.register(strat1).unwrap();
        registry.register(strat2).unwrap();

        let enabled = registry.get_enabled();
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn test_strategy_registry_list_ids() {
        let mut registry = StrategyRegistry::new();
        let strat1 = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        let strat2 = Arc::new(SpotPerpArbitrageStrategy::new()) as Arc<dyn Strategy>;

        registry.register(strat1).unwrap();
        registry.register(strat2).unwrap();

        let ids = registry.list_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"convergence_arbitrage".to_string()));
        assert!(ids.contains(&"spot_perp_arbitrage".to_string()));
    }

    #[test]
    fn test_strategy_registry_list_names() {
        let mut registry = StrategyRegistry::new();
        let strat = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        registry.register(strat).unwrap();

        let names = registry.list_names();
        assert_eq!(names.len(), 1);
        assert!(names[0].contains("Convergence Arbitrage"));
        assert!(names[0].contains("convergence_arbitrage"));
    }

    #[test]
    fn test_strategy_registry_count() {
        let mut registry = StrategyRegistry::new();
        assert_eq!(registry.count(), 0);

        let strat = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        registry.register(strat).unwrap();
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_strategy_registry_count_enabled() {
        let mut registry = StrategyRegistry::new();
        let strat1 = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        let strat2 = Arc::new(SpotPerpArbitrageStrategy::new()) as Arc<dyn Strategy>;

        registry.register(strat1).unwrap();
        registry.register(strat2).unwrap();

        assert_eq!(registry.count_enabled(), 2);
    }

    #[test]
    fn test_strategy_registry_unregister() {
        let mut registry = StrategyRegistry::new();
        let strat = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        registry.register(strat).unwrap();

        let result = registry.unregister("convergence_arbitrage");
        assert!(result.is_ok());
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_strategy_registry_unregister_nonexistent() {
        let mut registry = StrategyRegistry::new();
        let result = registry.unregister("nonexistent_strategy");
        assert!(result.is_err());
    }

    #[test]
    fn test_strategy_registry_contains() {
        let mut registry = StrategyRegistry::new();
        let strat = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        registry.register(strat).unwrap();

        assert!(registry.contains("convergence_arbitrage"));
        assert!(!registry.contains("nonexistent_strategy"));
    }

    #[test]
    fn test_strategy_registry_clear() {
        let mut registry = StrategyRegistry::new();
        let strat1 = Arc::new(ConvergenceArbitrageStrategy::new()) as Arc<dyn Strategy>;
        let strat2 = Arc::new(SpotPerpArbitrageStrategy::new()) as Arc<dyn Strategy>;

        registry.register(strat1).unwrap();
        registry.register(strat2).unwrap();
        assert_eq!(registry.count(), 2);

        registry.clear();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_strategy_registry_all_strategies() {
        let mut registry = StrategyRegistry::new();

        let strategies: Vec<Arc<dyn Strategy>> = vec![
            Arc::new(ConvergenceArbitrageStrategy::new()),
            Arc::new(SpotPerpArbitrageStrategy::new()),
            Arc::new(LatencyArbitrageStrategy::new()),
            Arc::new(NewListingArbitrageStrategy::new()),
            Arc::new(FundingRateArbitrageStrategy::new()),
            Arc::new(HedgedFundingStrategy::new()),
            Arc::new(SpreadCaptureStrategy::new()),
            Arc::new(StablecoinArbitrageStrategy::new()),
            Arc::new(CexArbitrageStrategy::new()),
            Arc::new(CrossExchangeArbitrageStrategy::new()),
        ];

        for strategy in strategies {
            let result = registry.register(strategy);
            assert!(result.is_ok(), "Failed to register strategy");
        }

        assert_eq!(registry.count(), 10);

        let ids = registry.list_ids();
        assert!(ids.contains(&"convergence_arbitrage".to_string()));
        assert!(ids.contains(&"spot_perp_arbitrage".to_string()));
        assert!(ids.contains(&"latency_arbitrage".to_string()));
        assert!(ids.contains(&"new_listing_arbitrage".to_string()));
        assert!(ids.contains(&"funding_rate_arbitrage".to_string()));
        assert!(ids.contains(&"hedged_funding".to_string()));
        assert!(ids.contains(&"spread_capture".to_string()));
        assert!(ids.contains(&"stablecoin_arbitrage".to_string()));
        assert!(ids.contains(&"cex_arbitrage".to_string()));
        assert!(ids.contains(&"cross_exchange_arbitrage".to_string()));
    }

    #[test]
    fn test_convergence_arbitrage_strategy_properties() {
        let strategy = ConvergenceArbitrageStrategy::new();
        assert_eq!(strategy.id(), "convergence_arbitrage");
        assert_eq!(strategy.name(), "Convergence Arbitrage");
    }

    #[test]
    fn test_spot_perp_arbitrage_strategy_properties() {
        let strategy = SpotPerpArbitrageStrategy::new();
        assert_eq!(strategy.id(), "spot_perp_arbitrage");
        assert_eq!(strategy.name(), "Spot ↔ Perpetual Arbitrage");
    }

    #[test]
    fn test_latency_arbitrage_strategy_properties() {
        let strategy = LatencyArbitrageStrategy::new();
        assert_eq!(strategy.id(), "latency_arbitrage");
        assert_eq!(strategy.name(), "Latency Arbitrage");
    }

    #[test]
    fn test_new_listing_arbitrage_strategy_properties() {
        let strategy = NewListingArbitrageStrategy::new();
        assert_eq!(strategy.id(), "new_listing_arbitrage");
        assert_eq!(strategy.name(), "New Listing Arbitrage");
    }

    #[test]
    fn test_funding_rate_arbitrage_strategy_properties() {
        let strategy = FundingRateArbitrageStrategy::new();
        assert_eq!(strategy.id(), "funding_rate_arbitrage");
        assert_eq!(strategy.name(), "Funding Rate Arbitrage");
    }

    #[test]
    fn test_hedged_funding_strategy_properties() {
        let strategy = HedgedFundingStrategy::new();
        assert_eq!(strategy.id(), "hedged_funding");
        assert!(strategy.name().contains("Hedged Funding"));
    }

    #[test]
    fn test_spread_capture_strategy_properties() {
        let strategy = SpreadCaptureStrategy::new();
        assert_eq!(strategy.id(), "spread_capture");
        assert!(strategy.name().contains("Spread"));
    }

    #[test]
    fn test_stablecoin_arbitrage_strategy_properties() {
        let strategy = StablecoinArbitrageStrategy::new();
        assert_eq!(strategy.id(), "stablecoin_arbitrage");
        assert!(strategy.name().contains("Stablecoin"));
    }

    #[test]
    fn test_cex_arbitrage_strategy_properties() {
        let strategy = CexArbitrageStrategy::new();
        assert_eq!(strategy.id(), "cex_arbitrage");
        assert!(strategy.name().contains("CEX"));
    }

    #[test]
    fn test_cross_exchange_arbitrage_strategy_properties() {
        let strategy = CrossExchangeArbitrageStrategy::new();
        assert_eq!(strategy.id(), "cross_exchange_arbitrage");
        assert!(strategy.name().contains("Cross"));
    }

    #[test]
    fn test_strategy_config_access() {
        let strategy = ConvergenceArbitrageStrategy::new();
        let config = strategy.config();
        assert!(config.enabled);
        assert!(config.max_exposure > Decimal::ZERO);
    }

    #[test]
    fn test_strategy_update_config() {
        let mut strategy = ConvergenceArbitrageStrategy::new();
        let mut new_config = strategy.config().clone();
        new_config.min_profit_bps = 20;
        new_config.max_exposure = Decimal::from(50000);

        let result = strategy.update_config(new_config);
        assert!(result.is_ok());

        let updated_config = strategy.config();
        assert_eq!(updated_config.min_profit_bps, 20);
        assert_eq!(updated_config.max_exposure, Decimal::from(50000));
    }
}
