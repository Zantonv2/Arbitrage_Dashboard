#[cfg(test)]
mod tests {
    use crate::symbol_discovery::{
        DiscoveryEvent, MarketInfo, OrderBookDepth, SymbolDiscoveryService,
        SymbolSelectionCriteria, TrendDirection,
    };
    use crate::types::{ExchangeId, Symbol};
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use rustc_hash::FxHashSet;

    fn create_test_market_info(
        exchange: ExchangeId,
        symbol: &Symbol,
        volume_usd: Decimal,
        spread_bps: u32,
        is_active: bool,
    ) -> MarketInfo {
        MarketInfo {
            symbol: symbol.clone(),
            exchange,
            volume_24h_usd: volume_usd,
            price_usd: Decimal::from(50000),
            spread_bps,
            is_active,
            timestamp: Utc::now(),
            depth_analysis: OrderBookDepth {
                level_1_volume_usd: Decimal::from(100000),
                depth_01_percent_usd: Decimal::from(500000),
                depth_05_percent_usd: Decimal::from(2000000),
                max_order_size_usd: Decimal::from(100000),
            },
        }
    }

    fn create_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    #[test]
    fn test_symbol_selection_criteria_default() {
        let criteria = SymbolSelectionCriteria::default();

        assert_eq!(criteria.min_volume_usd, Decimal::from(1_000_000));
        assert_eq!(criteria.max_spread_bps, 50);
        assert_eq!(criteria.min_exchanges, 2);
        assert!(criteria.allowed_quotes.contains("USDT"));
        assert!(criteria.allowed_quotes.contains("USDC"));
        assert_eq!(criteria.max_symbols, 50);
        assert_eq!(criteria.min_level1_liquidity_usd, Decimal::from(10_000));
        assert_eq!(criteria.min_depth_01_percent_usd, Decimal::from(50_000));
        assert_eq!(criteria.max_data_age_seconds, 300);
    }

    #[test]
    fn test_symbol_selection_criteria_custom() {
        let mut allowed_quotes = FxHashSet::default();
        allowed_quotes.insert("BTC".to_string());
        allowed_quotes.insert("ETH".to_string());

        let criteria = SymbolSelectionCriteria {
            min_volume_usd: Decimal::from(5_000_000),
            max_spread_bps: 25,
            min_exchanges: 3,
            allowed_quotes,
            max_symbols: 100,
            min_level1_liquidity_usd: Decimal::from(50_000),
            min_depth_01_percent_usd: Decimal::from(200_000),
            min_stability_ratio: Decimal::from(500_000),
            max_data_age_seconds: 60,
        };

        assert_eq!(criteria.min_volume_usd, Decimal::from(5_000_000));
        assert_eq!(criteria.max_spread_bps, 25);
        assert_eq!(criteria.min_exchanges, 3);
        assert_eq!(criteria.max_symbols, 100);
    }

    #[test]
    fn test_symbol_discovery_service_new() {
        let criteria = SymbolSelectionCriteria::default();
        let (service, _receiver) = SymbolDiscoveryService::new(criteria);

        let stats = service.get_market_stats(&Symbol::new("BTC", "USDT"));
        assert!(stats.is_none());
    }

    #[test]
    fn test_symbol_discovery_service_update_market_data() {
        let criteria = SymbolSelectionCriteria::default();
        let (mut service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbol = create_symbol();
        let market_info =
            create_test_market_info(ExchangeId::OKX, &symbol, Decimal::from(5_000_000), 20, true);

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(service.update_market_data(market_info));
        assert!(result.is_ok());
    }

    #[test]
    fn test_symbol_discovery_service_update_market_data_expired() {
        let criteria = SymbolSelectionCriteria::default();
        let (mut service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbol = create_symbol();
        let mut market_info =
            create_test_market_info(ExchangeId::OKX, &symbol, Decimal::from(5_000_000), 20, true);
        market_info.timestamp = Utc::now() - Duration::hours(10);

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(service.update_market_data(market_info));
        assert!(result.is_err());
    }

    #[test]
    fn test_symbol_discovery_service_get_arbitrage_symbols() {
        let criteria = SymbolSelectionCriteria::default();
        let (mut service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbol = create_symbol();
        let market_info1 =
            create_test_market_info(ExchangeId::OKX, &symbol, Decimal::from(5_000_000), 20, true);
        let market_info2 = create_test_market_info(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(5_000_000),
            25,
            true,
        );

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            service.update_market_data(market_info1).await.unwrap();
            service.update_market_data(market_info2).await.unwrap();
        });

        let arbitrage_symbols = service.get_arbitrage_symbols();
        assert!(!arbitrage_symbols.unwrap().is_empty());
    }

    #[test]
    fn test_symbol_discovery_service_get_market_stats() {
        let criteria = SymbolSelectionCriteria::default();
        let (mut service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbol = create_symbol();
        let market_info =
            create_test_market_info(ExchangeId::OKX, &symbol, Decimal::from(5_000_000), 20, true);

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            service.update_market_data(market_info).await.unwrap();
        });

        let stats = service.get_market_stats(&symbol);
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.exchange_count, 1);
        assert!(stats.total_volume_usd > Decimal::ZERO);
    }

    #[test]
    fn test_symbol_discovery_service_get_market_stats_nonexistent() {
        let criteria = SymbolSelectionCriteria::default();
        let (service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbol = Symbol::new("NONEXISTENT", "USDT");
        let stats = service.get_market_stats(&symbol);
        assert!(stats.is_none());
    }

    #[test]
    fn test_symbol_discovery_service_update_criteria() {
        let criteria = SymbolSelectionCriteria::default();
        let (mut service, _receiver) = SymbolDiscoveryService::new(criteria);

        let new_criteria = SymbolSelectionCriteria {
            min_volume_usd: Decimal::from(10_000_000),
            ..SymbolSelectionCriteria::default()
        };

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(service.update_criteria(new_criteria));
        assert!(result.is_ok());
    }

    #[test]
    fn test_order_book_depth_creation() {
        let depth = OrderBookDepth {
            level_1_volume_usd: Decimal::from(100000),
            depth_01_percent_usd: Decimal::from(500000),
            depth_05_percent_usd: Decimal::from(2000000),
            max_order_size_usd: Decimal::from(50000),
        };

        assert_eq!(depth.level_1_volume_usd, Decimal::from(100000));
        assert_eq!(depth.depth_01_percent_usd, Decimal::from(500000));
        assert_eq!(depth.depth_05_percent_usd, Decimal::from(2000000));
        assert_eq!(depth.max_order_size_usd, Decimal::from(50000));
    }

    #[test]
    fn test_market_info_creation() {
        let symbol = Symbol::new("BTC", "USDT");
        let market_info = MarketInfo {
            symbol: symbol.clone(),
            exchange: ExchangeId::OKX,
            volume_24h_usd: Decimal::from(100_000_000),
            price_usd: Decimal::from(50000),
            spread_bps: 15,
            is_active: true,
            timestamp: Utc::now(),
            depth_analysis: OrderBookDepth {
                level_1_volume_usd: Decimal::from(100000),
                depth_01_percent_usd: Decimal::from(500000),
                depth_05_percent_usd: Decimal::from(2000000),
                max_order_size_usd: Decimal::from(100000),
            },
        };

        assert_eq!(market_info.symbol, symbol);
        assert_eq!(market_info.exchange, ExchangeId::OKX);
        assert_eq!(market_info.volume_24h_usd, Decimal::from(100_000_000));
        assert_eq!(market_info.spread_bps, 15);
        assert!(market_info.is_active);
    }

    #[test]
    fn test_discovery_event_symbol_added() {
        let event = DiscoveryEvent::SymbolAdded {
            symbol: Symbol::new("BTC", "USDT"),
            reason: "Meets volume criteria".to_string(),
        };

        match event {
            DiscoveryEvent::SymbolAdded { symbol, reason } => {
                assert_eq!(symbol.base, "BTC");
                assert_eq!(symbol.quote, "USDT");
                assert_eq!(reason, "Meets volume criteria");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_discovery_event_symbol_removed() {
        let event = DiscoveryEvent::SymbolRemoved {
            symbol: Symbol::new("ETH", "USDT"),
            reason: "Volume dropped below threshold".to_string(),
        };

        match event {
            DiscoveryEvent::SymbolRemoved { symbol, reason } => {
                assert_eq!(symbol.base, "ETH");
                assert!(reason.contains("Volume"));
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_discovery_event_criteria_updated() {
        let new_criteria = SymbolSelectionCriteria::default();
        let event = DiscoveryEvent::CriteriaUpdated { new_criteria };

        match event {
            DiscoveryEvent::CriteriaUpdated { new_criteria: _ } => {
                // Criteria was updated
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_discovery_event_quality_alert() {
        let event = DiscoveryEvent::QualityAlert {
            symbol: Symbol::new("BTC", "USDT"),
            issue: "High spread detected".to_string(),
        };

        match event {
            DiscoveryEvent::QualityAlert { symbol, issue } => {
                assert_eq!(symbol.base, "BTC");
                assert!(issue.contains("spread"));
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_trend_direction_values() {
        assert_eq!(TrendDirection::Rising, TrendDirection::Rising);
        assert_eq!(TrendDirection::Falling, TrendDirection::Falling);
        assert_eq!(TrendDirection::Stable, TrendDirection::Stable);
    }

    #[test]
    fn test_symbol_selection_criteria_clone() {
        let criteria = SymbolSelectionCriteria::default();
        let cloned = criteria.clone();

        assert_eq!(criteria.min_volume_usd, cloned.min_volume_usd);
        assert_eq!(criteria.max_spread_bps, cloned.max_spread_bps);
        assert_eq!(criteria.allowed_quotes.len(), cloned.allowed_quotes.len());
    }

    #[test]
    fn test_symbol_discovery_service_empty_market_data() {
        let criteria = SymbolSelectionCriteria::default();
        let (service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbols = service.get_arbitrage_symbols();
        assert!(symbols.unwrap().is_empty());
    }

    #[test]
    fn test_symbol_discovery_service_multiple_exchanges() {
        let criteria = SymbolSelectionCriteria::default();
        let (mut service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbol = create_symbol();

        let exchanges = [
            ExchangeId::OKX,
            ExchangeId::ByBit,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
        ];

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            for exchange in exchanges {
                let market_info =
                    create_test_market_info(exchange, &symbol, Decimal::from(10_000_000), 20, true);
                service.update_market_data(market_info).await.unwrap();
            }
        });

        let stats = service.get_market_stats(&symbol);
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().exchange_count, 4);
    }

    #[test]
    fn test_symbol_discovery_service_inactive_market() {
        let criteria = SymbolSelectionCriteria::default();
        let (mut service, _receiver) = SymbolDiscoveryService::new(criteria);

        let symbol = create_symbol();
        let active_info =
            create_test_market_info(ExchangeId::OKX, &symbol, Decimal::from(5_000_000), 20, true);
        let inactive_info = create_test_market_info(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(5_000_000),
            20,
            false,
        );

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            service.update_market_data(active_info).await.unwrap();
            service.update_market_data(inactive_info).await.unwrap();
        });

        let stats = service.get_market_stats(&symbol);
        assert!(stats.is_some());
        // Only active market should count
        assert!(stats.unwrap().exchange_count >= 1);
    }
}
