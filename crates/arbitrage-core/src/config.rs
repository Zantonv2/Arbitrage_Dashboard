use crate::{ExchangeId, Symbol};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub exchanges: HashMap<ExchangeId, ExchangeConfig>,
    pub trading: TradingConfig,
    pub risk: RiskConfig,
    pub notifications: NotificationConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            exchanges: HashMap::new(),
            trading: TradingConfig::default(),
            risk: RiskConfig::default(),
            notifications: NotificationConfig::default(),
            storage: StorageConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub static_files_path: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            cors_origins: vec!["http://localhost:5173".to_string()],
            static_files_path: "frontend/dist".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub passphrase: Option<String>,
    pub testnet: bool,
    pub symbols: Vec<Symbol>,
    pub rate_limit_per_second: u32,
    pub reconnect_delay_ms: u64,
    pub max_reconnect_attempts: u32,
    pub heartbeat_interval_ms: u64,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: None,
            api_secret: None,
            passphrase: None,
            testnet: true,
            symbols: vec![
                Symbol::new("BTC", "USDT"),
                Symbol::new("ETH", "USDT"),
                Symbol::new("SOL", "USDT"),
            ],
            rate_limit_per_second: 10,
            reconnect_delay_ms: 1000,
            max_reconnect_attempts: 10,
            heartbeat_interval_ms: 30000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub min_profit_threshold_percent: Decimal,
    pub min_confidence_threshold: Decimal,
    pub max_signal_age_seconds: u64,
    pub signal_deduplication_window_ms: u64,
    pub stale_orderbook_threshold_ms: u64,
    pub slippage_buffer_percent: Decimal,
    pub default_time_in_force: String,
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            min_profit_threshold_percent: Decimal::from_str_exact("0.1").unwrap(),
            min_confidence_threshold: Decimal::from_str_exact("0.3").unwrap(),
            max_signal_age_seconds: 300, // 5 minutes
            signal_deduplication_window_ms: 5000,
            stale_orderbook_threshold_ms: 10000,
            slippage_buffer_percent: Decimal::from_str_exact("0.05").unwrap(),
            default_time_in_force: "IOC".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_size_usd: Decimal,
    pub max_slippage_percent: Decimal,
    pub conservative_size_multiplier: Decimal,
    pub min_order_size_usd: Decimal,
    pub max_order_size_usd: Decimal,
    pub max_concurrent_signals: u32,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_size_usd: Decimal::from(10000),
            max_slippage_percent: Decimal::from_str_exact("0.2").unwrap(),
            conservative_size_multiplier: Decimal::from_str_exact("0.8").unwrap(),
            min_order_size_usd: Decimal::from(10),
            max_order_size_usd: Decimal::from(50000),
            max_concurrent_signals: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub min_profit_for_notification: Decimal,
    pub min_confidence_for_notification: Decimal,
    pub desktop_notifications: bool,
    pub sound_enabled: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_profit_for_notification: Decimal::from_str_exact("0.5").unwrap(),
            min_confidence_for_notification: Decimal::from_str_exact("0.7").unwrap(),
            desktop_notifications: true,
            sound_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub database_url: String,
    pub retention_days: u32,
    pub max_signals_in_memory: usize,
    pub cleanup_interval_hours: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite:data/arbitrage.db".to_string(),
            retention_days: 90,
            max_signals_in_memory: 10000,
            cleanup_interval_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_enabled: bool,
    pub file_path: String,
    pub max_file_size_mb: u64,
    pub max_files: u32,
    pub json_format: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_enabled: true,
            file_path: "logs/arbitrage.log".to_string(),
            max_file_size_mb: 100,
            max_files: 10,
            json_format: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMapping {
    pub canonical: Symbol,
    pub exchange_symbols: HashMap<ExchangeId, String>,
    pub precision: HashMap<ExchangeId, u32>,
    pub min_quantity: HashMap<ExchangeId, Decimal>,
    pub min_notional: HashMap<ExchangeId, Decimal>,
}

impl SymbolMapping {
    pub fn new(canonical: Symbol) -> Self {
        Self {
            canonical,
            exchange_symbols: HashMap::new(),
            precision: HashMap::new(),
            min_quantity: HashMap::new(),
            min_notional: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StablecoinGroup {
    pub name: String,
    pub symbols: Vec<String>,
}

impl Default for StablecoinGroup {
    fn default() -> Self {
        Self {
            name: "USD Stablecoins".to_string(),
            symbols: vec![
                "USDT".to_string(),
                "USDC".to_string(),
                "BUSD".to_string(),
                "DAI".to_string(),
            ],
        }
    }
}

