use exchange_connectors::{
    bingx::BingXConnector,
    bybit::ByBitConnector,
    hyperliquid::HyperliquidConnector,
    connector::{ConnectorConfig, ExchangeConnector},
};
use arbitrage_core::types::{ExchangeId, Symbol, ConnectionStatus};
use tokio::time::{timeout, Duration};

/// Create a test connector config
fn create_test_config(exchange: ExchangeId) -> ConnectorConfig {
    ConnectorConfig {
        exchange,
        api_key: None,
        api_secret: None,
        passphrase: None,
        testnet: true,
        symbols: vec![Symbol::new("BTC", "USDT")],
        rate_limit_per_second: 10,
        reconnect_delay_ms: 1000,
        max_reconnect_attempts: 3,
        heartbeat_interval_ms: 30000,
    }
}

#[tokio::test]
async fn test_bybit_connector_creation() {
    let config = create_test_config(ExchangeId::ByBit);
    let connector = ByBitConnector::new(config);
    
    assert_eq!(connector.exchange_id(), ExchangeId::ByBit);
    assert!(matches!(connector.status(), ConnectionStatus::Disconnected));
    assert_eq!(connector.subscribed_symbols().len(), 0);
}

#[tokio::test]
async fn test_bingx_connector_creation() {
    let config = create_test_config(ExchangeId::BingX);
    let connector = BingXConnector::new(config);
    
    assert_eq!(connector.exchange_id(), ExchangeId::BingX);
    assert!(matches!(connector.status(), ConnectionStatus::Disconnected));
    assert_eq!(connector.subscribed_symbols().len(), 0);
}

#[tokio::test]
async fn test_hyperliquid_connector_creation() {
    let config = create_test_config(ExchangeId::Hyperliquid);
    let connector = HyperliquidConnector::new(config);
    
    assert_eq!(connector.exchange_id(), ExchangeId::Hyperliquid);
    assert!(matches!(connector.status(), ConnectionStatus::Disconnected));
    assert_eq!(connector.subscribed_symbols().len(), 0);
}

#[tokio::test]
async fn test_connector_subscription() {
    let config = create_test_config(ExchangeId::ByBit);
    let mut connector = ByBitConnector::new(config);
    
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDT"),
    ];
    
    // Test subscription
    let result = connector.subscribe(symbols.clone()).await;
    assert!(result.is_ok());
    assert_eq!(connector.subscribed_symbols().len(), 2);
    
    // Test unsubscription
    let result = connector.unsubscribe(vec![symbols[0].clone()]).await;
    assert!(result.is_ok());
    assert_eq!(connector.subscribed_symbols().len(), 1);
    assert_eq!(connector.subscribed_symbols()[0], symbols[1]);
}

#[tokio::test]
async fn test_connector_disconnect() {
    let config = create_test_config(ExchangeId::BingX);
    let mut connector = BingXConnector::new(config);
    
    // Subscribe to some symbols
    let symbols = vec![Symbol::new("BTC", "USDT")];
    let _ = connector.subscribe(symbols).await;
    assert_eq!(connector.subscribed_symbols().len(), 1);
    
    // Disconnect should clear subscriptions
    let result = connector.disconnect().await;
    assert!(result.is_ok());
    assert!(matches!(connector.status(), ConnectionStatus::Disconnected));
    assert_eq!(connector.subscribed_symbols().len(), 0);
}