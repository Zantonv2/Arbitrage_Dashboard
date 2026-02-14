use arbitrage_core::types::Symbol;
use exchange_connectors::connector::ExchangeConnector;
use exchange_connectors::connections::mexc::MEXCConnector;
use arbitrage_core::Result;
use serde_json::json;
use std::time::Duration;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_mexc_fetch_order_book_timeout() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v3/depth"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "bids": [["50000.00", "1.5"]],
                    "asks": [["50001.00", "1.0"]]
                }))
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&mock_server)
        .await;

    let mut connector = MEXCConnector::new();
    connector.base.config.rest_url = mock_server.uri();

    let symbol = Symbol::new("BTC", "USDT");
    let result = connector.fetch_order_book(&symbol).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    match error {
        arbitrage_core::ArbitrageError::Timeout(msg) => {
            assert!(msg.contains("MEXC"));
            assert!(msg.contains("fetch_order_book"));
            assert!(msg.contains("BTCUSDT"));
            assert!(msg.contains("timed out"));
        }
        _ => panic!("Expected Timeout error, got: {:?}", error),
    }
}

#[tokio::test]
async fn test_mexc_fetch_symbols_timeout() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v3/exchangeInfo"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "symbols": [{
                        "symbol": "BTCUSDT",
                        "status": "TRADING",
                        "isSpotTradingAllowed": true
                    }]
                }))
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&mock_server)
        .await;

    let mut connector = MEXCConnector::new();
    connector.base.config.rest_url = mock_server.uri();

    let result = connector.fetch_symbols().await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    match error {
        arbitrage_core::ArbitrageError::Timeout(msg) => {
            assert!(msg.contains("MEXC"));
            assert!(msg.contains("fetch_symbols"));
            assert!(msg.contains("timed out"));
        }
        _ => panic!("Expected Timeout error, got: {:?}", error),
    }
}

#[tokio::test]
async fn test_mexc_fetch_tickers_timeout() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v3/ticker/bookTicker"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([
                    {
                        "symbol": "BTCUSDT",
                        "bidPrice": "50000.50",
                        "askPrice": "50001.50",
                        "bidQty": "1.5",
                        "askQty": "1.0"
                    }
                ]))
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&mock_server)
        .await;

    let mut connector = MEXCConnector::new();
    connector.base.config.rest_url = mock_server.uri();

    let symbols = vec![Symbol::new("BTC", "USDT")];
    let result = connector.fetch_tickers(&symbols).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    match error {
        arbitrage_core::ArbitrageError::Timeout(msg) => {
            assert!(msg.contains("MEXC"));
            assert!(msg.contains("fetch_tickers"));
            assert!(msg.contains("BTC/USDT"));
            assert!(msg.contains("timed out"));
        }
        _ => panic!("Expected Timeout error, got: {:?}", error),
    }
}

#[tokio::test]
async fn test_mexc_get_balance_timeout() {
    let mock_server = MockServer::start().await;

    // Now we need to match the auth headers that MEXC sends
    Mock::given(method("GET"))
        .and(path("/api/v3/account"))
        .and(header("accesskey", "test_api_key"))
        .and(header_exists("timestamp"))
        .and(header_exists("signature"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "balances": [
                        {"asset": "USDT", "free": "1000.00", "locked": "0.00"}
                    ]
                }))
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&mock_server)
        .await;

    // Create connector and set credentials manually
    let mut connector = MEXCConnector::new();
    connector.base.config.rest_url = mock_server.uri();
    connector.base.config.api_key = Some("test_api_key".to_string());
    connector.base.config.api_secret = Some("dGVzdF9hcGlfc2VjcmV0".to_string());

    let result = connector.get_balance().await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    match error {
        arbitrage_core::ArbitrageError::Timeout(msg) => {
            assert!(msg.contains("MEXC"));
            assert!(msg.contains("get_balance"));
            assert!(msg.contains("timed out"));
        }
        _ => panic!("Expected Timeout error, got: {:?}", error),
    }
}

#[tokio::test]
async fn test_mexc_error_context_includes_operation_and_symbol() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v3/depth"))
        .respond_with(
            ResponseTemplate::new(500).set_body_raw("Internal Server Error", "text/plain"),
        )
        .mount(&mock_server)
        .await;

    let mut connector = MEXCConnector::new();
    connector.base.config.rest_url = mock_server.uri();

    let symbol = Symbol::new("ETH", "USDT");
    let result = connector.fetch_order_book(&symbol).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    match error {
        arbitrage_core::ArbitrageError::Network(msg) => {
            assert!(msg.contains("MEXC"));
            assert!(msg.contains("fetch_order_book"));
            assert!(msg.contains("ETHUSDT"));
            assert!(msg.contains("error="));
        }
        _ => panic!("Expected Network error, got: {:?}", error),
    }
}

#[tokio::test]
async fn test_mexc_timeout_error_includes_operation_and_symbol() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v3/depth"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "bids": [["50000.00", "1.5"]],
                    "asks": [["50001.00", "1.0"]]
                }))
                .set_delay(Duration::from_secs(10)),
        )
        .mount(&mock_server)
        .await;

    let mut connector = MEXCConnector::new();
    connector.base.config.rest_url = mock_server.uri();

    let symbol = Symbol::new("SOL", "USDT");
    let result = connector.fetch_order_book(&symbol).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    match error {
        arbitrage_core::ArbitrageError::Timeout(msg) => {
            assert!(msg.contains("operation=fetch_order_book"));
            assert!(msg.contains("exchange=MEXC"));
            assert!(msg.contains("symbol=SOLUSDT"));
            assert!(msg.contains("5s"));
        }
        _ => panic!("Expected Timeout error, got: {:?}", error),
    }
}
