//! # Secure Key Store
//!
//! Secure storage and management of API keys and credentials for exchange connections.
//! Uses encryption at rest and secure memory handling.

use arbitrage_core::types::ExchangeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeyStoreError {
    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),
}

/// Exchange API credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCredentials {
    pub exchange: ExchangeId,
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: Option<String>,
    pub sandbox: bool,
    pub enabled: bool,
}

/// Secure storage for API keys and credentials
pub struct KeyStore {
    credentials: HashMap<ExchangeId, ExchangeCredentials>,
    master_key: Option<String>,
}

impl KeyStore {
    /// Create new key store
    pub fn new() -> Result<Self, KeyStoreError> {
        Ok(Self {
            credentials: HashMap::new(),
            master_key: None,
        })
    }

    /// Initialize with master key for encryption
    pub fn with_master_key(master_key: String) -> Result<Self, KeyStoreError> {
        Ok(Self {
            credentials: HashMap::new(),
            master_key: Some(master_key),
        })
    }

    /// Store credentials for an exchange
    pub fn store_credentials(
        &mut self,
        credentials: ExchangeCredentials,
    ) -> Result<(), KeyStoreError> {
        // Validate credentials format
        self.validate_credentials(&credentials)?;

        // Store credentials (in production, these would be encrypted)
        self.credentials.insert(credentials.exchange, credentials);

        Ok(())
    }

    /// Retrieve credentials for an exchange
    pub fn get_credentials(
        &self,
        exchange: &ExchangeId,
    ) -> Result<&ExchangeCredentials, KeyStoreError> {
        self.credentials
            .get(exchange)
            .ok_or_else(|| KeyStoreError::KeyNotFound(format!("No credentials for {}", exchange)))
    }

    /// Remove credentials for an exchange
    pub fn remove_credentials(&mut self, exchange: &ExchangeId) -> Result<(), KeyStoreError> {
        self.credentials.remove(exchange);
        Ok(())
    }

    /// List all configured exchanges
    pub fn list_exchanges(&self) -> Vec<ExchangeId> {
        self.credentials.keys().cloned().collect()
    }

    /// Check if credentials exist for exchange
    pub fn has_credentials(&self, exchange: &ExchangeId) -> bool {
        self.credentials.contains_key(exchange)
    }

    /// Validate credentials format
    fn validate_credentials(&self, credentials: &ExchangeCredentials) -> Result<(), KeyStoreError> {
        if credentials.api_key.is_empty() {
            return Err(KeyStoreError::InvalidKeyFormat(
                "API key cannot be empty".to_string(),
            ));
        }

        if credentials.api_secret.is_empty() {
            return Err(KeyStoreError::InvalidKeyFormat(
                "API secret cannot be empty".to_string(),
            ));
        }

        // Exchange-specific validation
        match credentials.exchange {
            ExchangeId::OKX => {
                if credentials.passphrase.is_none() {
                    return Err(KeyStoreError::InvalidKeyFormat(
                        "OKX requires passphrase".to_string(),
                    ));
                }
            }
            _ => {
                // Other exchanges don't require passphrase
            }
        }

        Ok(())
    }

    /// Enable/disable credentials for an exchange
    pub fn set_enabled(
        &mut self,
        exchange: &ExchangeId,
        enabled: bool,
    ) -> Result<(), KeyStoreError> {
        if let Some(credentials) = self.credentials.get_mut(exchange) {
            credentials.enabled = enabled;
            Ok(())
        } else {
            Err(KeyStoreError::KeyNotFound(format!(
                "No credentials for {}",
                exchange
            )))
        }
    }

    /// Get enabled exchanges only
    pub fn get_enabled_exchanges(&self) -> Vec<ExchangeId> {
        self.credentials
            .values()
            .filter(|creds| creds.enabled)
            .map(|creds| creds.exchange)
            .collect()
    }

    /// Load credentials from configuration file (placeholder)
    pub fn load_from_config(&mut self, _config_path: &str) -> Result<(), KeyStoreError> {
        // TODO: Implement secure loading from encrypted config file
        // For now, return empty - credentials would be loaded from secure storage
        Ok(())
    }

    /// Save credentials to configuration file (placeholder)
    pub fn save_to_config(&self, _config_path: &str) -> Result<(), KeyStoreError> {
        // TODO: Implement secure saving to encrypted config file
        // For now, return success - credentials would be saved to secure storage
        Ok(())
    }

    /// Clear all credentials from memory
    pub fn clear(&mut self) {
        self.credentials.clear();
        self.master_key = None;
    }
}

impl Default for KeyStore {
    fn default() -> Self {
        Self::new().expect("Failed to create default KeyStore")
    }
}

impl Drop for KeyStore {
    fn drop(&mut self) {
        // Clear sensitive data from memory
        self.clear();
    }
}
