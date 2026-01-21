use crate::{ExchangeId, Symbol};
use aes_gcm::{aead::Aead, Aes256Gcm, Key, KeyInit, Nonce};
use pbkdf2::pbkdf2_hmac_array;
use rand::{rngs::OsRng, TryRngCore};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;

const KEY_SIZE: usize = 32;
const NONCE_SIZE: usize = 12;
const PBKDF2_ITERATIONS: u32 = 100000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedString {
    ciphertext: Vec<u8>,
    nonce: Vec<u8>,
    salt: Vec<u8>,
}

impl EncryptedString {
    fn derive_key(master_password: &[u8], salt: &[u8]) -> Key<Aes256Gcm> {
        let key_bytes =
            pbkdf2_hmac_array::<Sha256, KEY_SIZE>(master_password, salt, PBKDF2_ITERATIONS);
        *Key::<Aes256Gcm>::from_slice(&key_bytes)
    }

    pub fn new(plaintext: &str, master_password: &str) -> Self {
        let mut salt = [0u8; 16];
        OsRng
            .try_fill_bytes(&mut salt)
            .map_err(|_| "Failed to generate salt")
            .unwrap();
        let key = Self::derive_key(master_password.as_bytes(), &salt);
        let mut nonce_array = [0u8; NONCE_SIZE];
        OsRng
            .try_fill_bytes(&mut nonce_array)
            .map_err(|_| "Failed to generate nonce")
            .unwrap();
        let nonce = Nonce::from_slice(&nonce_array);

        let cipher = Aes256Gcm::new(&key);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .expect("Encryption failed");

        Self {
            ciphertext,
            nonce: nonce.to_vec(),
            salt: salt.to_vec(),
        }
    }

    pub fn decrypt(&self, master_password: &str) -> Result<String, String> {
        let salt: [u8; 16] = self
            .salt
            .clone()
            .try_into()
            .map_err(|_| "Invalid salt length")?;
        let key = Self::derive_key(master_password.as_bytes(), &salt);
        let nonce = Nonce::from_slice(&self.nonce);

        let cipher = Aes256Gcm::new(&key);
        cipher
            .decrypt(nonce, &*self.ciphertext)
            .map(|bytes| String::from_utf8(bytes).unwrap_or_default())
            .map_err(|e| format!("Decryption failed: {}", e))
    }

    pub fn is_empty(&self) -> bool {
        self.ciphertext.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub server: ServerConfig,
    pub exchanges: HashMap<ExchangeId, ExchangeConfig>,
    pub trading: TradingConfig,
    pub risk: RiskConfig,
    pub inventory: InventoryConfig,
    pub notifications: NotificationConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
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

fn get_master_key_from_env() -> String {
    std::env::var("MASTER_KEY").expect("MASTER_KEY environment variable must be set for encryption")
}

fn default_encrypted_string() -> EncryptedString {
    EncryptedString::new("", &get_master_key_from_env())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub enabled: bool,
    #[serde(skip_serializing, default = "default_encrypted_string")]
    pub api_key: EncryptedString,
    #[serde(skip_serializing, default = "default_encrypted_string")]
    pub api_secret: EncryptedString,
    #[serde(skip_serializing, default = "default_encrypted_string")]
    pub passphrase: EncryptedString,
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
            api_key: default_encrypted_string(),
            api_secret: default_encrypted_string(),
            passphrase: default_encrypted_string(),
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
            min_profit_threshold_percent: Decimal::from_str_exact("0.1")
                .expect("Failed to parse default min_profit_threshold_percent: '0.1'"),
            min_confidence_threshold: Decimal::from_str_exact("0.3")
                .expect("Failed to parse default min_confidence_threshold: '0.3'"),
            max_signal_age_seconds: 300, // 5 minutes
            signal_deduplication_window_ms: 5000,
            stale_orderbook_threshold_ms: 10000,
            slippage_buffer_percent: Decimal::from_str_exact("0.05")
                .expect("Failed to parse default slippage_buffer_percent: '0.05'"),
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
            max_slippage_percent: Decimal::from_str_exact("0.2")
                .expect("Failed to parse default max_slippage_percent: '0.2'"),
            conservative_size_multiplier: Decimal::from_str_exact("0.8")
                .expect("Failed to parse default conservative_size_multiplier: '0.8'"),
            min_order_size_usd: Decimal::from(10),
            max_order_size_usd: Decimal::from(50000),
            max_concurrent_signals: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryConfig {
    /// Default inventory limits per exchange and asset
    pub default_limits: HashMap<String, Decimal>,
    /// Exchange-specific inventory limits
    pub exchange_limits: HashMap<ExchangeId, HashMap<String, Decimal>>,
    /// Enable inventory-based filtering
    pub enable_inventory_checks: bool,
}

impl Default for InventoryConfig {
    fn default() -> Self {
        let mut default_limits = HashMap::new();
        // Set generous default limits for testing
        default_limits.insert("BTC".to_string(), Decimal::from(100));
        default_limits.insert("ETH".to_string(), Decimal::from(1000));
        default_limits.insert("USDT".to_string(), Decimal::from(1000000));
        default_limits.insert("USDC".to_string(), Decimal::from(1000000));
        default_limits.insert("USD".to_string(), Decimal::from(1000000));

        Self {
            default_limits,
            exchange_limits: HashMap::new(),
            enable_inventory_checks: true,
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
            min_profit_for_notification: Decimal::from_str_exact("0.5")
                .expect("Failed to parse default min_profit_for_notification: '0.5'"),
            min_confidence_for_notification: Decimal::from_str_exact("0.7")
                .expect("Failed to parse default min_confidence_for_notification: '0.7'"),
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
