//! Authentication utilities for exchange API requests.
//!
//! This module provides HMAC signature generation and authentication
//! utilities for all supported cryptocurrency exchanges.

use arbitrage_core::types::ExchangeId;
use arbitrage_core::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;

/// Type alias for HMAC-SHA256
pub type HmacSha256 = Hmac<Sha256>;
/// Type alias for HMAC-SHA512
pub type HmacSha512 = Hmac<Sha512>;

/// Authentication credentials for an exchange
#[derive(Debug, Clone)]
pub struct ExchangeCredentials {
    /// API key
    pub api_key: String,
    /// API secret
    pub api_secret: String,
    /// Passphrase (required by some exchanges like OKX)
    pub passphrase: Option<String>,
}

impl ExchangeCredentials {
    /// Create new credentials
    pub fn new(api_key: String, api_secret: String, passphrase: Option<String>) -> Self {
        Self {
            api_key,
            api_secret,
            passphrase,
        }
    }

    /// Validate that credentials are present
    pub fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(arbitrage_core::ArbitrageError::AuthenticationFailed(
                "API key is empty".to_string(),
            ));
        }
        if self.api_secret.is_empty() {
            return Err(arbitrage_core::ArbitrageError::AuthenticationFailed(
                "API secret is empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Authentication headers for an exchange request
#[derive(Debug, Clone, Default)]
pub struct AuthHeaders {
    /// Header map to be added to requests
    pub headers: HashMap<String, String>,
}

impl AuthHeaders {
    /// Create empty auth headers
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }

    /// Convert to reqwest HeaderMap
    pub fn to_header_map(&self) -> Result<HeaderMap> {
        let mut header_map = HeaderMap::new();
        for (name, value) in &self.headers {
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|e| arbitrage_core::ArbitrageError::Validation(e.to_string()))?;
            let header_value =
                HeaderValue::from_str(value).map_err(|e| {
                    arbitrage_core::ArbitrageError::Validation(e.to_string())
                })?;
            header_map.insert(header_name, header_value);
        }
        Ok(header_map)
    }
}

/// Generate HMAC-SHA256 signature
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC-SHA256 key length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

/// Generate HMAC-SHA512 signature
pub fn hmac_sha512(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha512::new_from_slice(key).expect("HMAC-SHA512 key length");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

/// Generate Base64 encoded HMAC-SHA256 signature
pub fn hmac_sha256_base64(key: &[u8], message: &[u8]) -> String {
    let signature = hmac_sha256(key, message);
    BASE64.encode(signature)
}

/// Generate Base64 encoded HMAC-SHA512 signature
pub fn hmac_sha512_base64(key: &[u8], message: &[u8]) -> String {
    let signature = hmac_sha512(key, message);
    BASE64.encode(signature)
}

/// Generate hexadecimal HMAC signature
pub fn hmac_hex(key: &[u8], message: &[u8]) -> String {
    let signature = hmac_sha256(key, message);
    signature.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Get current timestamp in milliseconds
pub fn get_timestamp_ms() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("SystemTime before UNIX_EPOCH");
    format!("{}", now.as_millis())
}

/// Get current timestamp in seconds
pub fn get_timestamp_s() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("SystemTime before UNIX_EPOCH");
    format!("{}", now.as_secs())
}

// ============================================================================
// OKX Authentication
// ============================================================================

/// Generate OKX API signature
///
/// OKX uses HMAC-SHA256 with the following formula:
/// sign = Base64(HMAC-SHA256(timestamp + method + path + body, secret_key))
///
/// Required headers:
/// - OK-ACCESS-TIMESTAMP: Unix timestamp in milliseconds
/// - OK-ACCESS-SIGN: Base64 encoded signature
/// - OK-ACCESS-PASSPHRASE: API passphrase
/// - OK-ACCESS-KEY: API key
pub fn generate_okx_signature(
    timestamp: &str,
    method: &str,
    path: &str,
    body: &str,
    secret: &str,
) -> String {
    let message = format!("{}{}{}{}", timestamp, method, path, body);
    hmac_sha256_base64(secret.as_bytes(), message.as_bytes())
}

/// Generate OKX authentication headers
pub fn okx_auth_headers(
    method: &str,
    path: &str,
    body: &str,
    credentials: &ExchangeCredentials,
) -> Result<AuthHeaders> {
    credentials.validate()?;

    let timestamp = get_timestamp_ms();
    let sign = generate_okx_signature(&timestamp, method, path, body, &credentials.api_secret);

    let mut headers = AuthHeaders::new();
    headers
        .headers
        .insert("OK-ACCESS-TIMESTAMP".to_string(), timestamp);
    headers
        .headers
        .insert("OK-ACCESS-SIGN".to_string(), sign);
    headers
        .headers
        .insert("OK-ACCESS-KEY".to_string(), credentials.api_key.clone());

    if let Some(passphrase) = &credentials.passphrase {
        headers
            .headers
            .insert("OK-ACCESS-PASSPHRASE".to_string(), passphrase.clone());
    }

    headers.headers.insert(
        "Content-Type".to_string(),
        "application/json".to_string(),
    );

    Ok(headers)
}

// ============================================================================
// ByBit Authentication
// ============================================================================

/// Generate ByBit API signature
///
/// ByBit uses HMAC-SHA256 with the following formula:
/// sign = HMAC-SHA256(timestamp + api_key + param_str, secret_key)
///
/// Required headers:
/// - X-BAPI-API-KEY: API key
/// - X-BAPI-TIMESTAMP: Unix timestamp in milliseconds
/// - X-BAPI-SIGN: Hex encoded signature
pub fn generate_bybit_signature(
    timestamp: &str,
    api_key: &str,
    param_str: &str,
    secret: &str,
) -> String {
    let message = format!("{}{}{}", timestamp, api_key, param_str);
    hmac_hex(secret.as_bytes(), message.as_bytes())
}

/// Generate ByBit authentication headers
pub fn bybit_auth_headers(
    method: &str,
    _endpoint: &str,
    params: &HashMap<String, String>,
    body: &str,
    credentials: &ExchangeCredentials,
) -> Result<AuthHeaders> {
    credentials.validate()?;

    let timestamp = get_timestamp_ms();

    // Build query string or use body for signature
    let param_str = if method == "GET" {
        let mut query_params: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        query_params.sort();
        query_params.join("&")
    } else {
        body.to_string()
    };

    let sign =
        generate_bybit_signature(&timestamp, &credentials.api_key, &param_str, &credentials.api_secret);

    let mut headers = AuthHeaders::new();
    headers
        .headers
        .insert("X-BAPI-API-KEY".to_string(), credentials.api_key.clone());
    headers
        .headers
        .insert("X-BAPI-TIMESTAMP".to_string(), timestamp);
    headers
        .headers
        .insert("X-BAPI-SIGN".to_string(), sign);
    headers
        .headers
        .insert("Content-Type".to_string(), "application/json".to_string());

    Ok(headers)
}

// ============================================================================
// MEXC Authentication
// ============================================================================

/// Generate MEXC API signature
///
/// MEXC uses HMAC-SHA256 with the following formula:
/// sign = Base64(HMAC-SHA256(timestamp + method + uri + body, secret_key))
///
/// Required headers:
/// - AccessKey: API key
/// - Timestamp: Unix timestamp in milliseconds
/// - Signature: Base64 encoded signature
pub fn generate_mexc_signature(
    timestamp: &str,
    method: &str,
    uri: &str,
    body: &str,
    secret: &str,
) -> String {
    let message = format!("{}{}{}{}", timestamp, method, uri, body);
    hmac_sha256_base64(secret.as_bytes(), message.as_bytes())
}

/// Generate MEXC authentication headers
pub fn mexc_auth_headers(
    method: &str,
    uri: &str,
    body: &str,
    credentials: &ExchangeCredentials,
) -> Result<AuthHeaders> {
    credentials.validate()?;

    let timestamp = get_timestamp_ms();
    let sign = generate_mexc_signature(&timestamp, method, uri, body, &credentials.api_secret);

    let mut headers = AuthHeaders::new();
    headers
        .headers
        .insert("AccessKey".to_string(), credentials.api_key.clone());
    headers
        .headers
        .insert("Timestamp".to_string(), timestamp);
    headers
        .headers
        .insert("Signature".to_string(), sign);
    headers
        .headers
        .insert("Content-Type".to_string(), "application/json".to_string());

    Ok(headers)
}

// ============================================================================
// Gate.io Authentication
// ============================================================================

/// Generate Gate.io API signature
///
/// Gate.io uses HMAC-SHA512 with the following formula:
/// sign = Base64(HMAC-SHA512(timestamp + method + uri + query_string + body, secret_key))
///
/// Required headers:
/// - KEY_CODE: API key
/// - TIMESTAMP: Unix timestamp in seconds
/// - SIGN: Base64 encoded signature
pub fn generate_gateio_signature(
    timestamp: &str,
    method: &str,
    uri: &str,
    query_string: &str,
    body: &str,
    secret: &str,
) -> String {
    let message = format!("{}{}{}{}{}", timestamp, method, uri, query_string, body);
    let signature = hmac_sha512(secret.as_bytes(), message.as_bytes());
    BASE64.encode(signature)
}

/// Generate Gate.io authentication headers
pub fn gateio_auth_headers(
    method: &str,
    uri: &str,
    query_string: &str,
    body: &str,
    credentials: &ExchangeCredentials,
) -> Result<AuthHeaders> {
    credentials.validate()?;

    let timestamp = get_timestamp_s();
    let sign = generate_gateio_signature(
        &timestamp,
        method,
        uri,
        query_string,
        body,
        &credentials.api_secret,
    );

    let mut headers = AuthHeaders::new();
    headers
        .headers
        .insert("KEY_CODE".to_string(), credentials.api_key.clone());
    headers
        .headers
        .insert("TIMESTAMP".to_string(), timestamp);
    headers
        .headers
        .insert("SIGN".to_string(), sign);
    headers
        .headers
        .insert("Content-Type".to_string(), "application/json".to_string());

    Ok(headers)
}

// ============================================================================
// Kraken Authentication
// ============================================================================

/// Generate Kraken API signature
///
/// Kraken uses HMAC-SHA512 with the following formula:
/// sign = Base64(HMAC-SHA512(endpoint_path, sha256(nonce + post_data)))
///
/// Required headers:
/// - API-Key: API key
/// - API-Sign: Base64 encoded signature
/// - nonce: Increasing nonce value
pub fn generate_kraken_signature(
    _endpoint: &str,
    nonce: &str,
    post_data: &str,
    secret: &str,
) -> Result<String> {
    // Decode the Base64 secret
    let secret_decoded = BASE64.decode(secret).map_err(|_| {
        arbitrage_core::ArbitrageError::AuthenticationFailed(
            "Invalid Base64 API secret".to_string(),
        )
    })?;

    // Calculate SHA256 of nonce + post_data
    let nonce_post = format!("{}{}", nonce, post_data);
    let sha256_hash = Sha256::digest(nonce_post.as_bytes());

    // Calculate HMAC-SHA512
    let signature = hmac_sha512(&secret_decoded, &sha256_hash);

    Ok(BASE64.encode(signature))
}

/// Generate Kraken authentication headers
pub fn kraken_auth_headers(
    endpoint: &str,
    post_data: &str,
    credentials: &ExchangeCredentials,
) -> Result<AuthHeaders> {
    credentials.validate()?;

    let nonce = format!("{}", chrono::Utc::now().timestamp_millis());

    let sign = generate_kraken_signature(endpoint, &nonce, post_data, &credentials.api_secret)?;

    let mut headers = AuthHeaders::new();
    headers
        .headers
        .insert("API-Key".to_string(), credentials.api_key.clone());
    headers
        .headers
        .insert("API-Sign".to_string(), sign);
    headers
        .headers
        .insert("nonce".to_string(), nonce);
    headers
        .headers
        .insert("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string());

    Ok(headers)
}

// ============================================================================
// Bitstamp Authentication
// ============================================================================

/// Generate Bitstamp API signature
///
/// Bitstamp uses HMAC-SHA256 with the following formula:
/// signature = HMAC-SHA256(timestamp + customer_id + api_key, secret_key)
///
/// Required headers:
/// - X-Rauth: API key
/// - X-Signature: Hex encoded signature
pub fn generate_bitstamp_signature(
    timestamp: &str,
    customer_id: &str,
    api_key: &str,
    secret: &str,
) -> String {
    let message = format!("{}{}{}", timestamp, customer_id, api_key);
    hmac_hex(secret.as_bytes(), message.as_bytes())
}

/// Generate Bitstamp authentication headers
pub fn bitstamp_auth_headers(
    customer_id: &str,
    api_version: &str,
    credentials: &ExchangeCredentials,
) -> Result<AuthHeaders> {
    credentials.validate()?;

    let timestamp = get_timestamp_s();
    let signature = generate_bitstamp_signature(
        &timestamp,
        customer_id,
        &credentials.api_key,
        &credentials.api_secret,
    );

    let mut headers = AuthHeaders::new();
    headers
        .headers
        .insert("X-Rauth".to_string(), credentials.api_key.clone());
    headers
        .headers
        .insert("X-Signature".to_string(), signature);
    headers
        .headers
        .insert("X-Timestamp".to_string(), timestamp);
    headers
        .headers
        .insert("X-Version".to_string(), api_version.to_string());

    Ok(headers)
}

// ============================================================================
// Unified Authentication Interface
// ============================================================================

/// Generate authentication headers for a specific exchange
pub fn generate_auth_headers(
    exchange_id: ExchangeId,
    method: &str,
    path: &str,
    params: HashMap<String, String>,
    body: &str,
    credentials: &ExchangeCredentials,
) -> Result<AuthHeaders> {
    match exchange_id {
        ExchangeId::OKX => okx_auth_headers(method, path, body, credentials),
        ExchangeId::ByBit => bybit_auth_headers(method, path, &params, body, credentials),
        ExchangeId::MEXC => mexc_auth_headers(method, path, body, credentials),
        ExchangeId::GateIo => gateio_auth_headers(method, path, "", body, credentials),
        ExchangeId::Kraken => {
            // Kraken uses post_data for endpoint signature
            kraken_auth_headers(path, body, credentials)
        }
        ExchangeId::Bitstamp => {
            // Bitstamp uses customer_id in signature
            bitstamp_auth_headers("", "v2", credentials)
        }
        _ => Err(arbitrage_core::ArbitrageError::Validation(format!(
            "Unsupported exchange for authentication: {}",
            exchange_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256() {
        let key = b"test_secret";
        let message = b"test_message";
        let result = hmac_sha256(key, message);
        assert_eq!(result.len(), 32); // SHA256 produces 32 bytes
    }

    #[test]
    fn test_hmac_sha512() {
        let key = b"test_secret";
        let message = b"test_message";
        let result = hmac_sha512(key, message);
        assert_eq!(result.len(), 64); // SHA512 produces 64 bytes
    }

    #[test]
    fn test_hmac_sha256_base64() {
        let key = b"test_secret";
        let message = b"test_message";
        let result = hmac_sha256_base64(key, message);
        // Base64 encoding of 32 bytes should be ~44 chars
        assert!(result.len() >= 40);
    }

    #[test]
    fn test_hmac_hex() {
        let key = b"test_secret";
        let message = b"test_message";
        let result = hmac_hex(key, message);
        // Hex encoding of 32 bytes should be 64 chars
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_get_timestamp() {
        let ts_ms = get_timestamp_ms();
        let ts_s = get_timestamp_s();
        // Milliseconds timestamp should be ~13 digits
        assert!(ts_ms.len() >= 13);
        // Seconds timestamp should be ~10 digits
        assert!(ts_s.len() >= 10);
    }

    #[test]
    fn test_okx_signature() {
        let timestamp = "1672531200000";
        let method = "POST";
        let path = "/api/v5/trade/order";
        let body = r#"{"instId":"BTC-USDT","tdMode":"cash","side":"buy","ordType":"limit","sz":"1","px":"20000"}"#;
        let secret = "test_secret";

        let sign = generate_okx_signature(timestamp, method, path, body, secret);
        assert!(!sign.is_empty());
    }

    #[test]
    fn test_bybit_signature() {
        let timestamp = "1672531200000";
        let api_key = "test_api_key";
        let param_str = "category=spot&side=Buy&symbol=BTC/USDT&type=market";
        let secret = "test_secret";

        let sign = generate_bybit_signature(timestamp, api_key, param_str, secret);
        assert_eq!(sign.len(), 64); // Hex encoded SHA256 = 64 chars
    }

    #[test]
    fn test_mexc_signature() {
        let timestamp = "1672531200000";
        let method = "POST";
        let uri = "/api/v3/order";
        let body = r#"{"symbol":"BTC_USDT","side":"BUY","type":"LIMIT","quantity":"0.001","price":"20000"}"#;
        let secret = "test_secret";

        let sign = generate_mexc_signature(timestamp, method, uri, body, secret);
        assert!(!sign.is_empty());
    }

    #[test]
    fn test_gateio_signature() {
        let timestamp = "1672531200";
        let method = "POST";
        let uri = "/api/v4/spot/orders";
        let query_string = "";
        let body = r#"{"currency_pair":"BTC_USDT","side":"buy","type":"limit","amount":"0.001","price":"20000"}"#;
        let secret = "test_secret";

        let sign = generate_gateio_signature(timestamp, method, uri, query_string, body, secret);
        assert!(!sign.is_empty());
    }

    #[test]
    fn test_credentials_validation() {
        let valid = ExchangeCredentials::new(
            "test_key".to_string(),
            "test_secret".to_string(),
            Some("passphrase".to_string()),
        );
        assert!(valid.validate().is_ok());

        let empty_key = ExchangeCredentials::new(
            "".to_string(),
            "test_secret".to_string(),
            None,
        );
        assert!(empty_key.validate().is_err());

        let empty_secret = ExchangeCredentials::new(
            "test_key".to_string(),
            "".to_string(),
            None,
        );
        assert!(empty_secret.validate().is_err());
    }

    #[test]
    fn test_auth_headers_to_header_map() {
        let mut headers = AuthHeaders::new();
        headers.headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.headers.insert("X-Custom".to_string(), "value".to_string());

        let map = headers.to_header_map().unwrap();
        assert!(map.contains_key("content-type"));
        assert!(map.contains_key("x-custom"));
    }
}
