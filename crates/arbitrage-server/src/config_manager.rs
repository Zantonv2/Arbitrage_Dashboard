use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub enabled: bool,
    pub path: String,
    pub max_connections: usize,
    pub ping_interval: u64,
    pub ping_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub max_requests: u32,
    pub window_seconds: u32,
    pub burst_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub jwt_expiry_hours: u32,
    pub cors_origins: Vec<String>,
    pub request_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: Option<String>,
    pub max_file_size: u64,
    pub max_files: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub websocket: WebSocketConfig,
    pub rate_limit: RateLimitConfig,
    pub security: SecurityConfig,
    pub logging: LoggingConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            workers: num_cpus::get(),
            max_connections: 1000,
        }
    }
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/ws".to_string(),
            max_connections: 500,
            ping_interval: 30,
            ping_timeout: 10,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_requests: 100,
            window_seconds: 60,
            burst_size: 10,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "change-me-in-production".to_string(),
            jwt_expiry_hours: 24,
            cors_origins: vec!["http://localhost:3000".to_string()],
            request_timeout: 30,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_path: None,
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_files: 5,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            websocket: WebSocketConfig::default(),
            rate_limit: RateLimitConfig::default(),
            security: SecurityConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

#[derive(Debug)]
pub struct ConfigManager {
    config: AppConfig,
    config_path: Option<String>,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            config: AppConfig::default(),
            config_path: None,
        }
    }

    pub fn with_config(config: AppConfig) -> Self {
        Self {
            config,
            config_path: None,
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: AppConfig =
            toml::from_str(&content).with_context(|| "Failed to parse TOML config")?;

        Ok(Self {
            config,
            config_path: Some(path.as_ref().to_string_lossy().to_string()),
        })
    }

    pub fn load_from_env() -> Result<Self> {
        let mut config = AppConfig::default();

        // Server config
        if let Ok(host) = std::env::var("SERVER_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("SERVER_PORT") {
            config.server.port = port.parse().with_context(|| "Invalid SERVER_PORT value")?;
        }
        if let Ok(workers) = std::env::var("SERVER_WORKERS") {
            config.server.workers = workers
                .parse()
                .with_context(|| "Invalid SERVER_WORKERS value")?;
        }

        // Security config
        if let Ok(jwt_secret) = std::env::var("JWT_SECRET") {
            config.security.jwt_secret = jwt_secret;
        }
        if let Ok(jwt_expiry) = std::env::var("JWT_EXPIRY_HOURS") {
            config.security.jwt_expiry_hours = jwt_expiry
                .parse()
                .with_context(|| "Invalid JWT_EXPIRY_HOURS value")?;
        }

        // Logging config
        if let Ok(log_level) = std::env::var("LOG_LEVEL") {
            config.logging.level = log_level;
        }

        Ok(Self {
            config,
            config_path: None,
        })
    }

    pub fn get_config(&self) -> &AppConfig {
        &self.config
    }

    pub fn get_config_mut(&mut self) -> &mut AppConfig {
        &mut self.config
    }

    pub fn update_config<F>(&mut self, updater: F)
    where
        F: FnOnce(&mut AppConfig),
    {
        updater(&mut self.config);
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(&self.config)
            .with_context(|| "Failed to serialize config to TOML")?;

        std::fs::write(path, content).with_context(|| "Failed to write config file")?;

        Ok(())
    }

    pub fn validate(&self) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Security validations
        if self.config.security.jwt_secret == "change-me-in-production" {
            warnings.push("JWT secret is still the default value".to_string());
        }

        if self.config.security.jwt_secret.len() < 32 {
            warnings.push("JWT secret should be at least 32 characters".to_string());
        }

        // Server validations
        if self.config.server.port < 1024 {
            warnings.push("Using privileged port (< 1024) may require root".to_string());
        }

        if self.config.server.max_connections > 10000 {
            warnings.push("Very high max_connections may cause resource issues".to_string());
        }

        // Rate limit validations
        if self.config.rate_limit.max_requests > 1000 {
            warnings.push("High rate limit may affect performance".to_string());
        }

        // Logging validations
        if !["trace", "debug", "info", "warn", "error"]
            .contains(&self.config.logging.level.as_str())
        {
            warnings.push(
                "Invalid log level, should be one of: trace, debug, info, warn, error".to_string(),
            );
        }

        Ok(warnings)
    }

    pub fn get_overridden_settings(&self) -> HashMap<String, String> {
        let mut overrides = HashMap::new();
        let defaults = AppConfig::default();

        // Check for non-default values
        if self.config.server.host != defaults.server.host {
            overrides.insert("server.host".to_string(), self.config.server.host.clone());
        }
        if self.config.server.port != defaults.server.port {
            overrides.insert(
                "server.port".to_string(),
                self.config.server.port.to_string(),
            );
        }
        if self.config.server.workers != defaults.server.workers {
            overrides.insert(
                "server.workers".to_string(),
                self.config.server.workers.to_string(),
            );
        }

        if self.config.security.jwt_secret != defaults.security.jwt_secret {
            overrides.insert("security.jwt_secret".to_string(), "[REDACTED]".to_string());
        }
        if self.config.security.jwt_expiry_hours != defaults.security.jwt_expiry_hours {
            overrides.insert(
                "security.jwt_expiry_hours".to_string(),
                self.config.security.jwt_expiry_hours.to_string(),
            );
        }

        if self.config.logging.level != defaults.logging.level {
            overrides.insert(
                "logging.level".to_string(),
                self.config.logging.level.clone(),
            );
        }

        overrides
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_manager_new() {
        let manager = ConfigManager::new();
        let config = manager.get_config();

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.websocket.enabled, true);
        assert_eq!(config.rate_limit.max_requests, 100);
    }

    #[test]
    fn test_config_manager_with_config() {
        let mut config = AppConfig::default();
        config.server.port = 9090;
        config.websocket.enabled = false;

        let manager = ConfigManager::with_config(config);
        let manager_config = manager.get_config();

        assert_eq!(manager_config.server.port, 9090);
        assert_eq!(manager_config.websocket.enabled, false);
    }

    #[test]
    fn test_load_from_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("test_config.toml");

        let config_content = r#"
[server]
host = "192.168.1.1"
port = 9000
workers = 4

[websocket]
enabled = false
path = "/custom_ws"

[security]
jwt_secret = "test_secret_at_least_32_chars"
jwt_expiry_hours = 48

[logging]
level = "debug"
"#;

        fs::write(&config_path, config_content)?;

        let manager = ConfigManager::load_from_file(&config_path)?;
        let config = manager.get_config();

        assert_eq!(config.server.host, "192.168.1.1");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.server.workers, 4);
        assert_eq!(config.websocket.enabled, false);
        assert_eq!(config.websocket.path, "/custom_ws");
        assert_eq!(config.security.jwt_secret, "test_secret_at_least_32_chars");
        assert_eq!(config.security.jwt_expiry_hours, 48);
        assert_eq!(config.logging.level, "debug");

        Ok(())
    }

    #[test]
    fn test_load_from_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.toml");

        let result = ConfigManager::load_from_file(config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_env() -> Result<()> {
        // Set environment variables
        std::env::set_var("SERVER_HOST", "env.example.com");
        std::env::set_var("SERVER_PORT", "9999");
        std::env::set_var("SERVER_WORKERS", "8");
        std::env::set_var("JWT_SECRET", "env_jwt_secret_at_least_32_chars");
        std::env::set_var("JWT_EXPIRY_HOURS", "12");
        std::env::set_var("LOG_LEVEL", "warn");

        let manager = ConfigManager::load_from_env()?;
        let config = manager.get_config();

        assert_eq!(config.server.host, "env.example.com");
        assert_eq!(config.server.port, 9999);
        assert_eq!(config.server.workers, 8);
        assert_eq!(
            config.security.jwt_secret,
            "env_jwt_secret_at_least_32_chars"
        );
        assert_eq!(config.security.jwt_expiry_hours, 12);
        assert_eq!(config.logging.level, "warn");

        // Clean up
        std::env::remove_var("SERVER_HOST");
        std::env::remove_var("SERVER_PORT");
        std::env::remove_var("SERVER_WORKERS");
        std::env::remove_var("JWT_SECRET");
        std::env::remove_var("JWT_EXPIRY_HOURS");
        std::env::remove_var("LOG_LEVEL");

        Ok(())
    }

    #[test]
    fn test_load_from_env_invalid_values() {
        std::env::set_var("SERVER_PORT", "invalid_port");

        let result = ConfigManager::load_from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid SERVER_PORT"));

        std::env::remove_var("SERVER_PORT");
    }

    #[test]
    fn test_update_config() {
        let mut manager = ConfigManager::new();

        manager.update_config(|config| {
            config.server.port = 7777;
            config.rate_limit.max_requests = 200;
        });

        let config = manager.get_config();
        assert_eq!(config.server.port, 7777);
        assert_eq!(config.rate_limit.max_requests, 200);
    }

    #[test]
    fn test_save_to_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("saved_config.toml");

        let mut config = AppConfig::default();
        config.server.port = 8081;
        config.security.jwt_secret = "saved_secret_32_chars_long_test".to_string();

        let manager = ConfigManager::with_config(config);
        manager.save_to_file(&config_path)?;

        // Load it back and verify
        let loaded_manager = ConfigManager::load_from_file(&config_path)?;
        let loaded_config = loaded_manager.get_config();

        assert_eq!(loaded_config.server.port, 8081);
        assert_eq!(
            loaded_config.security.jwt_secret,
            "saved_secret_32_chars_long_test"
        );

        Ok(())
    }

    #[test]
    fn test_validate_config() -> Result<()> {
        let manager = ConfigManager::new();
        let warnings = manager.validate()?;

        // Default config should have warnings about JWT secret
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("default value")));

        Ok(())
    }

    #[test]
    fn test_validate_secure_config() -> Result<()> {
        let mut config = AppConfig::default();
        config.security.jwt_secret = "very_secure_secret_at_least_32_characters_long".to_string();
        config.server.port = 8080;
        config.server.max_connections = 5000;
        config.rate_limit.max_requests = 500;

        let manager = ConfigManager::with_config(config);
        let warnings = manager.validate()?;

        // Should have minimal warnings
        assert!(warnings.len() <= 2);

        Ok(())
    }

    #[test]
    fn test_get_overridden_settings() {
        let mut config = AppConfig::default();
        config.server.port = 9090;
        config.server.workers = 8;
        config.security.jwt_secret = "custom_secret".to_string();

        let manager = ConfigManager::with_config(config);
        let overrides = manager.get_overridden_settings();

        assert_eq!(overrides.get("server.port"), Some(&"9090".to_string()));
        assert_eq!(overrides.get("server.workers"), Some(&"8".to_string()));
        assert_eq!(
            overrides.get("security.jwt_secret"),
            Some(&"[REDACTED]".to_string())
        );

        // Default values shouldn't appear
        assert!(!overrides.contains_key("server.host"));
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(config.server.host, deserialized.server.host);
        assert_eq!(config.server.port, deserialized.server.port);
        assert_eq!(config.websocket.enabled, deserialized.websocket.enabled);
    }

    #[test]
    fn test_config_manager_default() {
        let manager = ConfigManager::default();
        let config = manager.get_config();

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
    }

    #[test]
    fn test_websocket_config_validation() {
        let config = WebSocketConfig::default();
        assert!(config.enabled);
        assert_eq!(config.path, "/ws");
        assert_eq!(config.max_connections, 500);
        assert_eq!(config.ping_interval, 30);
        assert_eq!(config.ping_timeout, 10);
    }

    #[test]
    fn test_rate_limit_config_validation() {
        let config = RateLimitConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_requests, 100);
        assert_eq!(config.window_seconds, 60);
        assert_eq!(config.burst_size, 10);
    }

    #[test]
    fn test_logging_config_validation() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert!(config.file_path.is_none());
        assert_eq!(config.max_file_size, 10 * 1024 * 1024);
        assert_eq!(config.max_files, 5);
    }

    #[test]
    fn test_security_config_validation() {
        let config = SecurityConfig::default();
        assert_eq!(config.jwt_secret, "change-me-in-production");
        assert_eq!(config.jwt_expiry_hours, 24);
        assert!(config
            .cors_origins
            .contains(&"http://localhost:3000".to_string()));
        assert_eq!(config.request_timeout, 30);
    }
}
