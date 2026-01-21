//! # Secure Key Store
//!
//! Secure storage and management of API keys and credentials for exchange connections.
//! Uses encryption at rest and secure memory handling.

use aes_gcm::{aead::Aead, Aes256Gcm, Key, KeyInit, Nonce};
use arbitrage_core::types::ExchangeId;
use pbkdf2::pbkdf2_hmac_array;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use thiserror::Error;

const KEY_SIZE: usize = 32;
const PBKDF2_ITERATIONS: u32 = 100000;

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

impl std::convert::From<String> for KeyStoreError {
    fn from(s: String) -> Self {
        KeyStoreError::Encryption(s)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedCredential {
    ciphertext: Vec<u8>,
    nonce: Vec<u8>,
    salt: Vec<u8>,
}

impl EncryptedCredential {
    fn derive_key(master_password: &[u8], salt: &[u8]) -> Key<Aes256Gcm> {
        let key_bytes =
            pbkdf2_hmac_array::<Sha256, KEY_SIZE>(master_password, salt, PBKDF2_ITERATIONS);
        *Key::<Aes256Gcm>::from_slice(&key_bytes)
    }

    pub fn encrypt(plaintext: &str, master_password: &str) -> Self {
        let mut salt = [0u8; 16];
        OsRng
            .try_fill_bytes(&mut salt)
            .map_err(|_| "Failed to generate salt")
            .unwrap();
        let key = Self::derive_key(master_password.as_bytes(), &salt);
        let mut nonce_array = [0u8; 12];
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
}

#[derive(Debug, Clone)]
pub struct StoredCredential {
    pub exchange: ExchangeId,
    pub encrypted_api_key: EncryptedCredential,
    pub encrypted_api_secret: EncryptedCredential,
    pub encrypted_passphrase: Option<EncryptedCredential>,
    pub sandbox: bool,
    pub enabled: bool,
}

/// Exchange API credentials (decrypted for use)
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
    credentials: HashMap<ExchangeId, StoredCredential>,
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

    /// Store credentials for an exchange (encrypts before storing)
    pub fn store_credentials(
        &mut self,
        credentials: ExchangeCredentials,
    ) -> Result<(), KeyStoreError> {
        self.validate_credentials(&credentials)?;

        let master_key = self.master_key.as_ref().ok_or_else(|| {
            KeyStoreError::Authentication("Master key not set for encryption".to_string())
        })?;

        let stored = StoredCredential {
            exchange: credentials.exchange,
            encrypted_api_key: EncryptedCredential::encrypt(&credentials.api_key, master_key),
            encrypted_api_secret: EncryptedCredential::encrypt(&credentials.api_secret, master_key),
            encrypted_passphrase: credentials
                .passphrase
                .as_ref()
                .map(|p| EncryptedCredential::encrypt(p, master_key)),
            sandbox: credentials.sandbox,
            enabled: credentials.enabled,
        };

        self.credentials.insert(credentials.exchange, stored);
        Ok(())
    }

    /// Retrieve credentials for an exchange (decrypts on demand)
    pub fn get_credentials(
        &self,
        exchange: &ExchangeId,
    ) -> Result<ExchangeCredentials, KeyStoreError> {
        let stored = self.credentials.get(exchange).ok_or_else(|| {
            KeyStoreError::KeyNotFound(format!("No credentials for {}", exchange))
        })?;

        let master_key = self.master_key.as_ref().ok_or_else(|| {
            KeyStoreError::Authentication("Master key not set for decryption".to_string())
        })?;

        let api_key = stored.encrypted_api_key.decrypt(master_key)?;
        let api_secret = stored.encrypted_api_secret.decrypt(master_key)?;
        let passphrase = match &stored.encrypted_passphrase {
            Some(enc_pass) => Some(enc_pass.decrypt(master_key)?),
            None => None,
        };

        Ok(ExchangeCredentials {
            exchange: stored.exchange,
            api_key,
            api_secret,
            passphrase,
            sandbox: stored.sandbox,
            enabled: stored.enabled,
        })
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

        match credentials.exchange {
            ExchangeId::OKX => {
                if credentials.passphrase.is_none() {
                    return Err(KeyStoreError::InvalidKeyFormat(
                        "OKX requires passphrase".to_string(),
                    ));
                }
            }
            _ => {}
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
        Ok(())
    }

    /// Save credentials to configuration file (placeholder)
    pub fn save_to_config(&self, _config_path: &str) -> Result<(), KeyStoreError> {
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
        self.clear();
    }
}
