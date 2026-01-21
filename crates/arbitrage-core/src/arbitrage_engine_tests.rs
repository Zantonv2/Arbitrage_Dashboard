#[cfg(test)]
mod tests {
    use crate::{
        arbitrage_engine::{ArbitrageEngine, ExecutionMode},
        confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
        config::Config,
        execution_preparer::{ExecutionConfig, ExecutionPreparer},
        normalizer::Normalizer,
        size_calculator::{SizeCalculator, SizeConfig},
        storage::{StorageConfig, StorageService},
        strategies::{MarketBundle, RawSignal, Strategy, StrategyRegistry, TradeLeg},
        types::{ExchangeId, OrderBook, OrderBookLevel, Side, Symbol},
        Result,
    };
    use chrono::Duration;
    use rust_decimal::Decimal;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    fn btc_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    async fn create_test_engine() -> (ArbitrageEngine, broadcast::Receiver<crate::types::Signal>) {
        let config = Config::default();
        let normalizer = Arc::new(Normalizer::new());
        let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
        let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
        let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));
        let storage_config = StorageConfig {
            database_path: ":memory:".to_string(),
            ..Default::default()
        };
        let storage = Arc::new(StorageService::new(storage_config).await.unwrap());
        ArbitrageEngine::new(
            config,
            normalizer,
            confidence_scorer,
            size_calculator,
            execution_preparer,
            storage,
        )
        .expect("Failed to create test engine")
    }

    fn create_order_book(
        exchange: ExchangeId,
        symbol: Symbol,
        bid_price: Decimal,
        ask_price: Decimal,
    ) -> OrderBook {
        OrderBook::new(
            exchange,
            symbol,
            vec![OrderBookLevel::new(bid_price, Decimal::from(1))],
            vec![OrderBookLevel::new(ask_price, Decimal::from(1))],
        )
    }

    struct MockStrategy {
        id: &'static str,
        name: &'static str,
        filter_enabled: bool,
        detect_enabled: bool,
        config: crate::strategies::StrategyConfig,
    }

    impl MockStrategy {
        fn new(id: &'static str, name: &'static str) -> Self {
            Self {
                id,
                name,
                filter_enabled: true,
                detect_enabled: true,
                config: crate::strategies::StrategyConfig::default(),
            }
        }

        fn with_filter_disabled(mut self) -> Self {
            self.filter_enabled = false;
            self
        }

        fn with_disabled(mut self) -> Self {
            self.detect_enabled = false;
            self.config.enabled = false;
            self
        }
    }

    impl Strategy for MockStrategy {
        fn id(&self) -> &'static str {
            self.id
        }

        fn name(&self) -> &'static str {
            self.name
        }

        fn detect(&self, _market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
            if !self.detect_enabled {
                return Ok(Vec::new());
            }
            let symbol = btc_symbol();
            let mut signal = RawSignal::new(self.id.to_string(), Arc::new(symbol.clone()));
            signal.add_leg(TradeLeg::new(
                ExchangeId::OKX,
                Arc::new(symbol.clone()),
                Side::Buy,
                Decimal::from(50000),
                Decimal::from(1),
            ));
            signal.add_leg(TradeLeg::new(
                ExchangeId::ByBit,
                Arc::new(symbol),
                Side::Sell,
                Decimal::from(50500),
                Decimal::from(1),
            ));
            signal.set_profit_bps(100);
            Ok(vec![signal])
        }

        fn filter(
            &self,
            _signal: &RawSignal,
            _context: &crate::strategies::FilterContext,
        ) -> Result<bool> {
            Ok(self.filter_enabled)
        }

        fn config(&self) -> &crate::strategies::StrategyConfig {
            &self.config
        }

        fn update_config(&mut self, config: crate::strategies::StrategyConfig) -> Result<()> {
            self.config = config;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_detect_opportunities_empty_market_data() {
        let (engine, _receiver) = create_test_engine().await;
        let registry = StrategyRegistry::new();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_detect_opportunities_with_strategy() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("test", "Test")))
            .unwrap();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_detect_opportunities_disabled_strategy() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("test", "Test").with_disabled()))
            .unwrap();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_detect_opportunities_multiple_strategies() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("s1", "Strategy 1")))
            .unwrap();
        registry
            .register(Arc::new(MockStrategy::new("s2", "Strategy 2")))
            .unwrap();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_detect_opportunities_with_order_books() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("test", "Test")))
            .unwrap();
        let symbol = btc_symbol();
        let order_book = create_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        engine.update_order_book(order_book).await.unwrap();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_detect_opportunities_with_tickers() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("test", "Test")))
            .unwrap();
        let symbol = btc_symbol();
        let ticker = crate::strategies::Ticker::new(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );
        engine.update_ticker(ticker).await.unwrap();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_detect_opportunities_with_funding_rates() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("test", "Test")))
            .unwrap();
        let symbol = btc_symbol();
        let funding_rate = crate::strategies::FundingRate::new(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from_str_exact("0.0100").unwrap(),
            chrono::Utc::now() + Duration::hours(8),
        );
        engine.update_funding_rate(funding_rate).await.unwrap();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_engine_stats_empty() {
        let (engine, _receiver) = create_test_engine().await;
        let stats = engine.get_stats();
        assert_eq!(stats.order_books_count, 0);
        assert_eq!(stats.tickers_count, 0);
        assert_eq!(stats.funding_rates_count, 0);
    }

    #[tokio::test]
    async fn test_engine_stats_with_data() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let order_book = create_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        engine.update_order_book(order_book).await.unwrap();
        let stats = engine.get_stats();
        assert_eq!(stats.order_books_count, 1);
        assert_eq!(stats.active_symbols_count, 1);
    }

    #[tokio::test]
    async fn test_execution_mode_default() {
        let (engine, _receiver) = create_test_engine().await;
        assert_eq!(engine.get_execution_mode(), ExecutionMode::Manual);
    }

    #[tokio::test]
    async fn test_execution_mode_set() {
        let (mut engine, _receiver) = create_test_engine().await;
        engine.set_execution_mode(ExecutionMode::Auto {
            confidence_threshold: Decimal::from(70),
        });
        assert_eq!(
            engine.get_execution_mode(),
            ExecutionMode::Auto {
                confidence_threshold: Decimal::from(70)
            }
        );
    }

    #[tokio::test]
    async fn test_signal_subscription() {
        let (engine, mut receiver) = create_test_engine().await;
        let _sub = engine.subscribe();
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_cleanup_stale_data() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let order_book = create_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        engine.update_order_book(order_book).await.unwrap();
        engine.cleanup_stale_data();
        let stats = engine.get_stats();
        assert_eq!(stats.order_books_count, 1);
    }

    #[tokio::test]
    async fn test_get_order_book_empty() {
        let (engine, _receiver) = create_test_engine().await;
        assert!(engine
            .get_order_book(ExchangeId::OKX, Arc::new(btc_symbol()))
            .is_none());
    }

    #[tokio::test]
    async fn test_get_order_book_with_data() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let order_book = create_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        engine.update_order_book(order_book).await.unwrap();
        assert!(engine
            .get_order_book(ExchangeId::OKX, Arc::new(symbol))
            .is_some());
    }

    #[tokio::test]
    async fn test_get_all_order_books() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let book1 = create_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        let book2 = create_order_book(
            ExchangeId::ByBit,
            symbol.clone(),
            Decimal::from(50020),
            Decimal::from(50030),
        );
        engine.update_order_book(book1).await.unwrap();
        engine.update_order_book(book2).await.unwrap();
        assert_eq!(engine.get_all_order_books().len(), 2);
    }

    #[tokio::test]
    async fn test_update_order_book_valid() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let order_book = create_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        assert!(engine.update_order_book(order_book).await.is_ok());
    }

    #[tokio::test]
    async fn test_update_order_book_invalid() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let mut order_book = create_order_book(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
        );
        order_book.asks.clear();
        assert!(engine.update_order_book(order_book).await.is_err());
    }

    #[tokio::test]
    async fn test_update_ticker_valid() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let ticker = crate::strategies::Ticker::new(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );
        assert!(engine.update_ticker(ticker).await.is_ok());
    }

    #[tokio::test]
    async fn test_update_funding_rate_valid() {
        let (engine, _receiver) = create_test_engine().await;
        let symbol = btc_symbol();
        let funding_rate = crate::strategies::FundingRate::new(
            ExchangeId::OKX,
            symbol.clone(),
            Decimal::from_str_exact("0.0100").unwrap(),
            chrono::Utc::now() + Duration::hours(8),
        );
        assert!(engine.update_funding_rate(funding_rate).await.is_ok());
    }

    #[tokio::test]
    async fn test_detect_opportunities_runs_successfully() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("test", "Test")))
            .unwrap();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_detect_calls() {
        let (engine, _receiver) = create_test_engine().await;
        let mut registry = StrategyRegistry::new();
        registry
            .register(Arc::new(MockStrategy::new("test", "Test")))
            .unwrap();
        let result1 = engine.detect_opportunities(&registry).await;
        let result2 = engine.detect_opportunities(&registry).await;
        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}
