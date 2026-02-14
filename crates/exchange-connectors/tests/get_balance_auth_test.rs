// Tests to verify get_balance() includes authentication headers
// This is a critical fix - previously get_balance() was missing auth and would fail on live exchanges

use exchange_connectors::auth::{
    okx_auth_headers, bybit_auth_headers, mexc_auth_headers, 
    gateio_auth_headers, kraken_auth_headers, bitstamp_auth_headers, 
    ExchangeCredentials,
};
use std::collections::HashMap;

// Use valid Base64-encoded test credentials to pass validation
fn create_test_credentials() -> ExchangeCredentials {
    // Base64 encoded "test_api_secret" = "dGVzdF9hcGlfc2VjcmV0"
    ExchangeCredentials::new(
        "test_api_key".to_string(),
        "dGVzdF9hcGlfc2VjcmV0".to_string(), // Base64 encoded secret
        None,
    )
}

#[test]
fn test_okx_get_balance_auth_headers_include_required_fields() {
    let credentials = create_test_credentials();
    let auth_headers = okx_auth_headers("GET", "/api/v5/account/balance", "", &credentials)
        .expect("Failed to generate OKX auth headers");
    
    let header_map = auth_headers.to_header_map().expect("Failed to convert to header map");
    
    // Verify required OKX headers are present
    assert!(header_map.contains_key("OK-ACCESS-KEY"), "Missing OK-ACCESS-KEY header");
    assert!(header_map.contains_key("OK-ACCESS-SIGN"), "Missing OK-ACCESS-SIGN header");
    assert!(header_map.contains_key("OK-ACCESS-TIMESTAMP"), "Missing OK-ACCESS-TIMESTAMP header");
}

#[test]
fn test_bybit_get_balance_auth_headers_include_required_fields() {
    let credentials = create_test_credentials();
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("accountType".to_string(), "SPOT".to_string());
    
    let auth_headers = bybit_auth_headers("GET", "/v5/account/wallet-balance", &params, "", &credentials)
        .expect("Failed to generate ByBit auth headers");
    
    let header_map = auth_headers.to_header_map().expect("Failed to convert to header map");
    
    // Verify required ByBit headers are present
    assert!(header_map.contains_key("X-BAPI-API-KEY"), "Missing X-BAPI-API-KEY header");
    assert!(header_map.contains_key("X-BAPI-SIGN"), "Missing X-BAPI-SIGN header");
    assert!(header_map.contains_key("X-BAPI-TIMESTAMP"), "Missing X-BAPI-TIMESTAMP header");
}

#[test]
fn test_mexc_get_balance_auth_headers_include_required_fields() {
    let credentials = create_test_credentials();
    let auth_headers = mexc_auth_headers("GET", "/api/v3/account", "", &credentials)
        .expect("Failed to generate MEXC auth headers");
    
    let header_map = auth_headers.to_header_map().expect("Failed to convert to header map");
    
    // Verify required MEXC headers are present (AccessKey not X-MEXC-APIKEY)
    assert!(header_map.contains_key("AccessKey"), "Missing AccessKey header");
    assert!(header_map.contains_key("Timestamp"), "Missing Timestamp header");
    assert!(header_map.contains_key("Signature"), "Missing Signature header");
}

#[test]
fn test_gateio_get_balance_auth_headers_include_required_fields() {
    let credentials = create_test_credentials();
    let auth_headers = gateio_auth_headers("GET", "/api/v4/spot/accounts", "", "", &credentials)
        .expect("Failed to generate Gate.io auth headers");
    
    let header_map = auth_headers.to_header_map().expect("Failed to convert to header map");
    
    // Verify required Gate.io headers are present
    assert!(header_map.contains_key("KEY_CODE"), "Missing KEY_CODE header");
    assert!(header_map.contains_key("SIGN"), "Missing SIGN header");
    assert!(header_map.contains_key("TIMESTAMP"), "Missing TIMESTAMP header");
}

#[test]
fn test_kraken_get_balance_auth_headers_include_required_fields() {
    let credentials = create_test_credentials();
    let post_data = "nonce=1234567890";
    let auth_headers = kraken_auth_headers("/0/private/Balance", post_data, &credentials)
        .expect("Failed to generate Kraken auth headers");
    
    let header_map = auth_headers.to_header_map().expect("Failed to convert to header map");
    
    // Verify required Kraken headers are present
    assert!(header_map.contains_key("API-Key"), "Missing API-Key header");
    assert!(header_map.contains_key("API-Sign"), "Missing API-Sign header");
}

#[test]
fn test_bitstamp_get_balance_auth_headers_include_required_fields() {
    let credentials = create_test_credentials();
    let auth_headers = bitstamp_auth_headers("", "v2", &credentials)
        .expect("Failed to generate Bitstamp auth headers");
    
    let header_map = auth_headers.to_header_map().expect("Failed to convert to header map");
    
    // Verify required Bitstamp headers are present
    assert!(header_map.contains_key("X-Rauth"), "Missing X-Rauth header (API-Key)");
    assert!(header_map.contains_key("X-Signature"), "Missing X-Signature header");
}
