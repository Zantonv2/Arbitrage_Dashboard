#[cfg(test)]
mod tests {
    use crate::exchange_manager::{ExchangeManager, ExchangeManagerConfig};
    use crate::mock::MockConnector;
    use arbitrage_core::types::ExchangeId;
    use tokio::time::Duration;

    fn create_test_config() -> ExchangeManagerConfig {
        ExchangeManagerConfig {
            enabled_exchanges: vec![ExchangeId::MEXC],
            health_check_interval_seconds: 1,
            max_reconnect_attempts: 3,
            event_buffer_size: 100,
            rate_limits: Default::default(),
            auto_reconnect: true,
            max_parallel_connections: 5,
        }
    }

    #[tokio::test]
    async fn test_graceful_shutdown_completes_within_timeout() {
        let mut config = create_test_config();
        config.health_check_interval_seconds = 30;
        let mut manager = ExchangeManager::new(config);

        let mock_connector = Box::new(MockConnector::new(ExchangeId::MEXC));
        manager.add_connector(mock_connector).await.unwrap();

        manager.initialize().await.unwrap();

        assert!(
            manager.is_healthy(),
            "Manager should be healthy after initialize"
        );

        tokio::time::sleep(Duration::from_millis(100)).await;

        let start = std::time::Instant::now();
        manager.shutdown(5).await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(5),
            "Shutdown took longer than 5s: {:?}",
            elapsed
        );
        assert!(
            !manager.is_healthy(),
            "Manager should not be healthy after shutdown"
        );
    }

    #[tokio::test]
    async fn test_shutdown_cancels_health_monitor() {
        let mut config = create_test_config();
        config.health_check_interval_seconds = 60;
        let mut manager = ExchangeManager::new(config);

        let mock_connector = Box::new(MockConnector::new(ExchangeId::MEXC));
        manager.add_connector(mock_connector).await.unwrap();

        manager.initialize().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            manager.is_healthy(),
            "Manager should be healthy before shutdown"
        );
        manager.shutdown(1).await;

        assert!(
            !manager.is_healthy(),
            "Manager should not be healthy after shutdown"
        );
    }

    #[tokio::test]
    async fn test_shutdown_cancels_event_processor() {
        let mut config = create_test_config();
        config.health_check_interval_seconds = 60;
        let mut manager = ExchangeManager::new(config);

        let mock_connector = Box::new(MockConnector::new(ExchangeId::MEXC));
        manager.add_connector(mock_connector).await.unwrap();

        manager.initialize().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            manager.is_healthy(),
            "Manager should be healthy before shutdown"
        );
        manager.shutdown(1).await;

        assert!(
            !manager.is_healthy(),
            "Manager should not be healthy after shutdown"
        );
    }

    #[tokio::test]
    async fn test_multiple_shutdown_calls() {
        let mut config = create_test_config();
        config.health_check_interval_seconds = 60;
        let mut manager = ExchangeManager::new(config);

        let mock_connector = Box::new(MockConnector::new(ExchangeId::MEXC));
        manager.add_connector(mock_connector).await.unwrap();

        manager.initialize().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(
            manager.is_healthy(),
            "Manager should be healthy before first shutdown"
        );
        manager.shutdown(1).await;
        assert!(
            !manager.is_healthy(),
            "Manager should not be healthy after first shutdown"
        );
        manager.shutdown(1).await;
        assert!(
            !manager.is_healthy(),
            "Manager should not be healthy after second shutdown"
        );
    }

    #[tokio::test]
    async fn test_shutdown_with_multiple_exchanges() {
        let mut config = create_test_config();
        config.enabled_exchanges = vec![ExchangeId::MEXC, ExchangeId::Kraken, ExchangeId::OKX];
        config.health_check_interval_seconds = 60;
        let mut manager = ExchangeManager::new(config);

        let mexc_connector = Box::new(MockConnector::new(ExchangeId::MEXC));
        let kraken_connector = Box::new(MockConnector::new(ExchangeId::Kraken));
        let okx_connector = Box::new(MockConnector::new(ExchangeId::OKX));

        manager.add_connector(mexc_connector).await.unwrap();
        manager.add_connector(kraken_connector).await.unwrap();
        manager.add_connector(okx_connector).await.unwrap();

        manager.initialize().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let start = std::time::Instant::now();
        manager.shutdown(5).await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(5),
            "Shutdown with multiple exchanges took longer than 5s: {:?}",
            elapsed
        );
    }
}
