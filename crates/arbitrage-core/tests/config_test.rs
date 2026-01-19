use arbitrage_core::config::*;
use arbitrage_core::types::{ExchangeId, Symbol};
use rust_decimal::Decimal;

#[test]
fn test_default_config() {
    let config = Config::default();
    assert_eq!(config.server.port, 3000);
    assert_eq!(config.server.host, "127.0.0.1");
    assert!(config.trading.min_profit_threshold_percent > Decimal::ZERO);
    assert!(config.risk.max_position_size_usd > Decimal::ZERO);
}

#[test]
fn test_server_config_defaults() {
    let server_config = ServerConfig::default();
    assert_eq!(server_config.host, "127.0.0.1");
    assert_eq!(server_config.port, 3000);
    assert!(!server_config.cors_origins.is_empty());
    assert_eq!(server_config.static_files_path, "frontend/dist");
}

#[test]
fn test_exchange_config_defaults() {
    let exchange_config = ExchangeConfig::default();
    assert!(!exchange_config.enabled);
    assert!(exchange_config.testnet);
    assert!(exchange_config.api_key.is_none());
    assert!(exchange_config.api_secret.is_none());
    assert_eq!(exchange_config.symbols.len(), 3); // BTC, ETH, SOL
    assert!(exchange_config.rate_limit_per_second > 0);
}

#[test]
fn test_trading_config_defaults() {
    let trading_config = TradingConfig::default();
    assert_eq!(
        trading_config.min_profit_threshold_percent,
        Decimal::from_str_exact("0.1").unwrap()
    );
    assert_eq!(
        trading_config.min_confidence_threshold,
        Decimal::from_str_exact("0.3").unwrap()
    );
    assert_eq!(trading_config.max_signal_age_seconds, 300);
    assert_eq!(trading_config.default_time_in_force, "IOC");
}

#[test]
fn test_risk_config_defaults() {
    let risk_config = RiskConfig::default();
    assert_eq!(risk_config.max_position_size_usd, Decimal::from(10000));
    assert_eq!(
        risk_config.conservative_size_multiplier,
        Decimal::from_str_exact("0.8").unwrap()
    );
    assert_eq!(risk_config.min_order_size_usd, Decimal::from(10));
    assert_eq!(risk_config.max_concurrent_signals, 100);
}

#[test]
fn test_notification_config_defaults() {
    let notification_config = NotificationConfig::default();
    assert!(notification_config.enabled);
    assert!(notification_config.desktop_notifications);
    assert!(!notification_config.sound_enabled);
    assert!(notification_config.min_profit_for_notification > Decimal::ZERO);
}

#[test]
fn test_storage_config_defaults() {
    let storage_config = StorageConfig::default();
    assert_eq!(storage_config.database_url, "sqlite:data/arbitrage.db");
    assert_eq!(storage_config.retention_days, 90);
    assert_eq!(storage_config.max_signals_in_memory, 10000);
    assert_eq!(storage_config.cleanup_interval_hours, 24);
}

#[test]
fn test_logging_config_defaults() {
    let logging_config = LoggingConfig::default();
    assert_eq!(logging_config.level, "info");
    assert!(logging_config.file_enabled);
    assert!(!logging_config.json_format);
    assert_eq!(logging_config.max_files, 10);
}

#[test]
fn test_symbol_mapping() {
    let mut mapping = SymbolMapping::new(Symbol::new("BTC", "USDT"));
    mapping
        .exchange_symbols
        .insert(ExchangeId::ByBit, "BTCUSDT".to_string());
    mapping
        .exchange_symbols
        .insert(ExchangeId::BingX, "BTC-USDT".to_string());

    assert_eq!(mapping.canonical.base, "BTC");
    assert_eq!(mapping.canonical.quote, "USDT");
    assert_eq!(mapping.exchange_symbols.len(), 2);
    assert_eq!(mapping.exchange_symbols[&ExchangeId::ByBit], "BTCUSDT");
    assert_eq!(mapping.exchange_symbols[&ExchangeId::BingX], "BTC-USDT");
}

#[test]
fn test_stablecoin_group_default() {
    let group = StablecoinGroup::default();
    assert_eq!(group.name, "USD Stablecoins");
    assert!(group.symbols.contains(&"USDT".to_string()));
    assert!(group.symbols.contains(&"USDC".to_string()));
    assert!(group.symbols.contains(&"BUSD".to_string()));
    assert!(group.symbols.contains(&"DAI".to_string()));
}

#[test]
fn test_config_serialization() {
    let config = Config::default();
    let serialized = serde_json::to_string(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&serialized).unwrap();

    assert_eq!(config.server.port, deserialized.server.port);
    assert_eq!(
        config.trading.min_profit_threshold_percent,
        deserialized.trading.min_profit_threshold_percent
    );
}
