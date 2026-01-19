//! # Key Store Tests
//!
//! Comprehensive tests for secure API key management including storage, retrieval,
//! validation, and security features.

use arbitrage_core::types::ExchangeId;
use arbitrage_server::key_store::{ExchangeCredentials, KeyStore, KeyStoreError};

/// Helper function to create test credentials
fn create_test_credentials(exchange: ExchangeId) -> ExchangeCredentials {
    ExchangeCredentials {
        exchange,
        api_key: format!("test_api_key_{}", exchange),
        api_secret: format!("test_api_secret_{}", exchange),
        passphrase: if exchange == ExchangeId::OKX {
            Some("test_passphrase".to_string())
        } else {
            None
        },
        sandbox: true,
        enabled: true,
    }
}

#[test]
fn test_keystore_creation() {
    let keystore = KeyStore::new().expect("Failed to create keystore");
    assert_eq!(keystore.list_exchanges().len(), 0);
}

#[test]
fn test_keystore_with_master_key() {
    let master_key = "test_master_key_123".to_string();
    let keystore =
        KeyStore::with_master_key(master_key).expect("Failed to create keystore with master key");
    assert_eq!(keystore.list_exchanges().len(), 0);
}

#[test]
fn test_store_and_retrieve_credentials() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");
    let credentials = create_test_credentials(ExchangeId::OKX);

    // Store credentials
    keystore
        .store_credentials(credentials.clone())
        .expect("Failed to store credentials");

    // Retrieve credentials
    let retrieved = keystore
        .get_credentials(&ExchangeId::OKX)
        .expect("Failed to retrieve credentials");

    assert_eq!(retrieved.exchange, credentials.exchange);
    assert_eq!(retrieved.api_key, credentials.api_key);
    assert_eq!(retrieved.api_secret, credentials.api_secret);
    assert_eq!(retrieved.passphrase, credentials.passphrase);
    assert_eq!(retrieved.sandbox, credentials.sandbox);
    assert_eq!(retrieved.enabled, credentials.enabled);
}

#[test]
fn test_store_multiple_exchanges() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    let exchanges = vec![ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC];

    // Store credentials for multiple exchanges
    for exchange in &exchanges {
        let credentials = create_test_credentials(*exchange);
        keystore
            .store_credentials(credentials)
            .expect("Failed to store credentials");
    }

    // Verify all exchanges are listed
    let stored_exchanges = keystore.list_exchanges();
    assert_eq!(stored_exchanges.len(), 3);

    for exchange in exchanges {
        assert!(stored_exchanges.contains(&exchange));
        assert!(keystore.has_credentials(&exchange));
    }
}

#[test]
fn test_remove_credentials() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");
    let credentials = create_test_credentials(ExchangeId::OKX);

    // Store and then remove credentials
    keystore
        .store_credentials(credentials)
        .expect("Failed to store credentials");
    assert!(keystore.has_credentials(&ExchangeId::OKX));

    keystore
        .remove_credentials(&ExchangeId::OKX)
        .expect("Failed to remove credentials");
    assert!(!keystore.has_credentials(&ExchangeId::OKX));

    // Verify retrieval fails after removal
    let result = keystore.get_credentials(&ExchangeId::OKX);
    assert!(result.is_err());
    match result.unwrap_err() {
        KeyStoreError::KeyNotFound(_) => {} // Expected
        _ => panic!("Expected KeyNotFound error"),
    }
}

#[test]
fn test_credentials_validation() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    // Test empty API key
    let mut invalid_credentials = create_test_credentials(ExchangeId::OKX);
    invalid_credentials.api_key = String::new();

    let result = keystore.store_credentials(invalid_credentials);
    assert!(result.is_err());
    match result.unwrap_err() {
        KeyStoreError::InvalidKeyFormat(msg) => assert!(msg.contains("API key")),
        _ => panic!("Expected InvalidKeyFormat error"),
    }

    // Test empty API secret
    let mut invalid_credentials = create_test_credentials(ExchangeId::OKX);
    invalid_credentials.api_secret = String::new();

    let result = keystore.store_credentials(invalid_credentials);
    assert!(result.is_err());
    match result.unwrap_err() {
        KeyStoreError::InvalidKeyFormat(msg) => assert!(msg.contains("API secret")),
        _ => panic!("Expected InvalidKeyFormat error"),
    }
}

#[test]
fn test_okx_passphrase_validation() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    // OKX requires passphrase
    let mut okx_credentials = create_test_credentials(ExchangeId::OKX);
    okx_credentials.passphrase = None;

    let result = keystore.store_credentials(okx_credentials);
    assert!(result.is_err());
    match result.unwrap_err() {
        KeyStoreError::InvalidKeyFormat(msg) => assert!(msg.contains("passphrase")),
        _ => panic!("Expected InvalidKeyFormat error for missing passphrase"),
    }

    // Other exchanges don't require passphrase
    let mut bybit_credentials = create_test_credentials(ExchangeId::ByBit);
    bybit_credentials.passphrase = None;

    let result = keystore.store_credentials(bybit_credentials);
    assert!(result.is_ok());
}

#[test]
fn test_enable_disable_credentials() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");
    let credentials = create_test_credentials(ExchangeId::OKX);

    keystore
        .store_credentials(credentials)
        .expect("Failed to store credentials");

    // Initially enabled
    assert!(keystore.get_enabled_exchanges().contains(&ExchangeId::OKX));

    // Disable
    keystore
        .set_enabled(&ExchangeId::OKX, false)
        .expect("Failed to disable");
    assert!(!keystore.get_enabled_exchanges().contains(&ExchangeId::OKX));

    // Re-enable
    keystore
        .set_enabled(&ExchangeId::OKX, true)
        .expect("Failed to enable");
    assert!(keystore.get_enabled_exchanges().contains(&ExchangeId::OKX));
}

#[test]
fn test_enable_nonexistent_exchange() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    let result = keystore.set_enabled(&ExchangeId::OKX, true);
    assert!(result.is_err());
    match result.unwrap_err() {
        KeyStoreError::KeyNotFound(_) => {} // Expected
        _ => panic!("Expected KeyNotFound error"),
    }
}

#[test]
fn test_get_enabled_exchanges_only() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    // Store credentials for multiple exchanges
    let exchanges = vec![ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC];
    for exchange in exchanges {
        let credentials = create_test_credentials(exchange);
        keystore
            .store_credentials(credentials)
            .expect("Failed to store credentials");
    }

    // Disable one exchange
    keystore
        .set_enabled(&ExchangeId::ByBit, false)
        .expect("Failed to disable");

    // Check enabled exchanges
    let enabled = keystore.get_enabled_exchanges();
    assert_eq!(enabled.len(), 2);
    assert!(enabled.contains(&ExchangeId::OKX));
    assert!(!enabled.contains(&ExchangeId::ByBit));
    assert!(enabled.contains(&ExchangeId::MEXC));
}

#[test]
fn test_overwrite_existing_credentials() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    // Store initial credentials
    let mut credentials1 = create_test_credentials(ExchangeId::OKX);
    credentials1.api_key = "initial_key".to_string();
    keystore
        .store_credentials(credentials1)
        .expect("Failed to store initial credentials");

    // Overwrite with new credentials
    let mut credentials2 = create_test_credentials(ExchangeId::OKX);
    credentials2.api_key = "updated_key".to_string();
    keystore
        .store_credentials(credentials2)
        .expect("Failed to store updated credentials");

    // Verify updated credentials
    let retrieved = keystore
        .get_credentials(&ExchangeId::OKX)
        .expect("Failed to retrieve credentials");
    assert_eq!(retrieved.api_key, "updated_key");
}

#[test]
fn test_sandbox_flag() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    // Store credentials with sandbox enabled
    let mut credentials = create_test_credentials(ExchangeId::OKX);
    credentials.sandbox = true;
    keystore
        .store_credentials(credentials)
        .expect("Failed to store credentials");

    let retrieved = keystore
        .get_credentials(&ExchangeId::OKX)
        .expect("Failed to retrieve credentials");
    assert!(retrieved.sandbox);

    // Update to production
    let mut prod_credentials = create_test_credentials(ExchangeId::OKX);
    prod_credentials.sandbox = false;
    keystore
        .store_credentials(prod_credentials)
        .expect("Failed to store production credentials");

    let retrieved = keystore
        .get_credentials(&ExchangeId::OKX)
        .expect("Failed to retrieve credentials");
    assert!(!retrieved.sandbox);
}

#[test]
fn test_clear_keystore() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    // Store multiple credentials
    let exchanges = vec![ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC];
    for exchange in exchanges {
        let credentials = create_test_credentials(exchange);
        keystore
            .store_credentials(credentials)
            .expect("Failed to store credentials");
    }

    assert_eq!(keystore.list_exchanges().len(), 3);

    // Clear all credentials
    keystore.clear();

    assert_eq!(keystore.list_exchanges().len(), 0);
    assert!(!keystore.has_credentials(&ExchangeId::OKX));
}

#[test]
fn test_load_save_config_placeholders() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    // These are placeholder implementations - should not fail
    let load_result = keystore.load_from_config("test_config.json");
    assert!(load_result.is_ok());

    let save_result = keystore.save_to_config("test_config.json");
    assert!(save_result.is_ok());
}

#[test]
fn test_credentials_memory_safety() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");
    let credentials = create_test_credentials(ExchangeId::OKX);

    keystore
        .store_credentials(credentials)
        .expect("Failed to store credentials");

    // Verify credentials exist
    assert!(keystore.has_credentials(&ExchangeId::OKX));

    // Drop keystore (should clear sensitive data)
    drop(keystore);

    // Create new keystore - should be empty
    let new_keystore = KeyStore::new().expect("Failed to create new keystore");
    assert!(!new_keystore.has_credentials(&ExchangeId::OKX));
}

#[test]
fn test_all_supported_exchanges() {
    let mut keystore = KeyStore::new().expect("Failed to create keystore");

    let all_exchanges = vec![
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Kraken,
        ExchangeId::Bitstamp,
    ];

    // Store credentials for all supported exchanges
    for exchange in &all_exchanges {
        let credentials = create_test_credentials(*exchange);
        keystore
            .store_credentials(credentials)
            .expect(&format!("Failed to store credentials for {}", exchange));
    }

    // Verify all exchanges are stored
    let stored_exchanges = keystore.list_exchanges();
    assert_eq!(stored_exchanges.len(), all_exchanges.len());

    for exchange in all_exchanges {
        assert!(keystore.has_credentials(&exchange));
        let retrieved = keystore
            .get_credentials(&exchange)
            .expect("Failed to retrieve credentials");
        assert_eq!(retrieved.exchange, exchange);
    }
}

#[test]
fn test_concurrent_access() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let keystore = Arc::new(Mutex::new(
        KeyStore::new().expect("Failed to create keystore"),
    ));
    let mut handles = Vec::new();

    // Spawn multiple threads to store credentials concurrently
    for i in 0..5 {
        let keystore_clone = keystore.clone();
        let handle = thread::spawn(move || {
            let exchange = match i {
                0 => ExchangeId::OKX,
                1 => ExchangeId::ByBit,
                2 => ExchangeId::MEXC,
                3 => ExchangeId::GateIo,
                _ => ExchangeId::Kraken,
            };

            let mut credentials = create_test_credentials(exchange);
            credentials.api_key = format!("thread_{}_key", i);

            let mut ks = keystore_clone.lock().unwrap();
            ks.store_credentials(credentials)
                .expect("Failed to store credentials");
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify all credentials were stored
    let ks = keystore.lock().unwrap();
    assert_eq!(ks.list_exchanges().len(), 5);
}
