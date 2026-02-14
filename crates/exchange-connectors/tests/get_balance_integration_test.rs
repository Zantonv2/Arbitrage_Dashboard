// Integration tests for get_balance() - verifies actual HTTP requests include auth headers
// Uses a mock HTTP server to intercept and verify requests

use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use exchange_connectors::connector::{ConnectorConfig, ExchangeConnector};
use exchange_connectors::connections::okx::OKXConnector;
use arbitrage_core::types::ExchangeId;

#[tokio::test]
async fn test_okx_get_balance_sends_authentication() {
    // Create a mock server that captures the request
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let rest_url = format!("http://{}", addr);
    
    // Spawn server in background
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let n = socket.read(&mut buf).await.unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).to_lowercase(); // lowercase for case-insensitive check
        
        // Verify authentication headers are present (case-insensitive)
        // Note: HTTP headers are case-insensitive, reqwest sends them as lowercase
        assert!(request.contains("ok-access-key:"), "Missing OK-ACCESS-KEY header");
        assert!(request.contains("ok-access-sign:"), "Missing OK-ACCESS-SIGN header"); 
        assert!(request.contains("ok-access-timestamp:"), "Missing OK-ACCESS-TIMESTAMP header");
        
        // Return a valid OKX response
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 47\r\n\r\n{\"code\":\"0\",\"msg\":\"\",\"data\":[{\"adjEq\":\"\",\"details\":[]}]}";
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    
    // Create connector with mock URL
    let config = ConnectorConfig {
        exchange_id: ExchangeId::OKX,
        ws_url: "wss://ws.okx.com:8443".to_string(),
        rest_url,
        api_key: Some("test_api_key".to_string()),
        api_secret: Some("dGVzdF9hcGlfc2VjcmV0".to_string()), // Base64 encoded
        passphrase: Some("test_passphrase".to_string()),
        is_testnet: false,
        credentials_encrypted: false,
        rate_limit_per_second: 20,
        rate_limit_burst: 40,
        reconnect_interval_ms: 5000,
        max_reconnect_attempts: 10,
        heartbeat_interval_ms: 30000,
        order_book_depth: 20,
        enable_trades: true,
        enable_tickers: true,
        enable_funding_rates: false,
    };
    let connector = OKXConnector::new_with_config(config);
    
    // Call get_balance - this should hit our mock server
    // We ignore the result because the mock server will panic if auth headers are missing
    let _ = connector.get_balance().await;
    
    // Wait for server to verify
    server.await.unwrap();
}

#[tokio::test] 
async fn test_okx_get_balance_fails_without_credentials() {
    // Create connector with EMPTY credentials (None)
    // NOTE: This should fail BEFORE making any HTTP request
    // because validate_credentials() returns error when api_key is None
    let config = ConnectorConfig {
        exchange_id: ExchangeId::OKX,
        ws_url: "wss://ws.okx.com:8443".to_string(),
        rest_url: "http://localhost:9999".to_string(), // Won't be used
        api_key: None,       // No API key
        api_secret: None,    // No API secret
        passphrase: None,
        is_testnet: false,
        credentials_encrypted: false,
        rate_limit_per_second: 20,
        rate_limit_burst: 40,
        reconnect_interval_ms: 5000,
        max_reconnect_attempts: 10,
        heartbeat_interval_ms: 30000,
        order_book_depth: 20,
        enable_trades: true,
        enable_tickers: true,
        enable_funding_rates: false,
    };
    let connector = OKXConnector::new_with_config(config);
    
    // Call get_balance - should fail due to missing credentials
    // The mock server at localhost:9999 should NEVER receive this request
    // because validate_credentials() fails first
    let result = connector.get_balance().await;
    
    // Verify that we get an authentication error
    assert!(result.is_err(), "Expected error when credentials are missing");
    let err = result.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("AuthenticationFailed") || err_str.contains("API key"), 
        "Expected authentication error, got: {}", err_str);
}
