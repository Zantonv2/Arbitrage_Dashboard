use arbitrage_core::config::Config;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Config validation error: {0}")]
    Validation(String),
}

/// Configuration manager for loading and hot-reloading config
pub struct ConfigManager {
    config_path: String,
    config: Config,
}

impl ConfigManager {
    /// Create new config manager and load initial config
    pub fn new(config_path: &str) -> Result<Self, ConfigError> {
        let config = if Path::new(config_path).exists() {
            Self::load_config(config_path)?
        } else {
            tracing::warn!("Config file {} not found, using defaults", config_path);
            Config::default()
        };

        Ok(Self {
            config_path: config_path.to_string(),
            config,
        })
    }

    /// Load configuration from file
    fn load_config(path: &str) -> Result<Config, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;

        // Validate config
        Self::validate_config(&config)?;

        tracing::info!("Loaded configuration from {}", path);
        Ok(config)
    }

    /// Validate configuration
    fn validate_config(config: &Config) -> Result<(), ConfigError> {
        if config.server.port == 0 {
            return Err(ConfigError::Validation(
                "Server port cannot be 0".to_string(),
            ));
        }

        if config.trading.min_profit_threshold_percent < rust_decimal::Decimal::ZERO {
            return Err(ConfigError::Validation(
                "Min profit threshold cannot be negative".to_string(),
            ));
        }

        if config.risk.max_position_size_usd <= rust_decimal::Decimal::ZERO {
            return Err(ConfigError::Validation(
                "Max position size must be positive".to_string(),
            ));
        }

        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// Reload configuration from file
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        let new_config = Self::load_config(&self.config_path)?;
        self.config = new_config;
        tracing::info!("Configuration reloaded");
        Ok(())
    }

    /// Update configuration and save to file
    pub fn update_config(&mut self, new_config: Config) -> Result<(), ConfigError> {
        Self::validate_config(&new_config)?;

        // Save to file
        let content = toml::to_string_pretty(&new_config)
            .map_err(|e| ConfigError::Validation(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(&self.config_path, content)?;

        self.config = new_config;
        tracing::info!("Configuration updated and saved");
        Ok(())
    }

    /// Create default config file if it doesn't exist
    pub fn create_default_config(path: &str) -> Result<(), ConfigError> {
        if Path::new(path).exists() {
            return Ok(());
        }

        let default_config = Config::default();
        let content = toml::to_string_pretty(&default_config).map_err(|e| {
            ConfigError::Validation(format!("Failed to serialize default config: {}", e))
        })?;

        // Create directory if it doesn't exist
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;
        tracing::info!("Created default configuration file at {}", path);
        Ok(())
    }
}
