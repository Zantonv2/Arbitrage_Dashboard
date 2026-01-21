use arbitrage_core::types::{ConnectionStatus, ExchangeId, Symbol};
use exchange_connectors::{
    connections::{
        bitstamp::BitstampConnector, bybit::BybitConnector, gateio::GateioConnector,
        kraken::KrakenConnector, mexc::MEXCConnector, okx::OKXConnector,
    },
    connector::{ConnectorConfig, ExchangeConnector},
    events::ConnectionEvent,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use warp;

/// Test configuration for each exchange
struct TestConfig {
    exchange_id: ExchangeId,
    test_symbols: Vec<Symbol>,
    supports_funding_rates: bool,
    expected_min_symbols: usize,
}

impl TestConfig {
    fn new(
        exchange_id: ExchangeId,
        test_symbols: Vec<Symbol>,
        supports_funding_rates: bool,
        expected_min_symbols: usize,
    ) -> Self {
        Self {
            exchange_id,
            test_symbols,
            supports_funding_rates,
            expected_min_symbols,
        }
    }
}

/// Test results for tracking success/failure
#[derive(Debug, Default)]
struct TestResults {
    total_tests: u32,
    passed_tests: u32,
    failed_tests: u32,
    test_details: Vec<String>,
}

impl TestResults {
    fn add_test(&mut self, test_name: &str, success: bool, details: Option<String>) {
        self.total_tests += 1;
        if success {
            self.passed_tests += 1;
            self.test_details.push(format!("✅ {}", test_name));
        } else {
            self.failed_tests += 1;
            let detail = details.unwrap_or_else(|| "No details".to_string());
            self.test_details
                .push(format!("❌ {} - {}", test_name, detail));
        }
    }

    fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            0.0
        } else {
            (self.passed_tests as f64 / self.total_tests as f64) * 100.0
        }
    }
}

/// Create test symbols for different exchanges
fn create_test_symbols() -> Vec<Symbol> {
    vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDT"),
        Symbol::new("BNB", "USDT"),
    ]
}

/// Get test configurations for all exchanges
fn get_test_configs() -> Vec<TestConfig> {
    let test_symbols = create_test_symbols();

    vec![
        TestConfig::new(ExchangeId::OKX, test_symbols.clone(), true, 100),
        TestConfig::new(ExchangeId::ByBit, test_symbols.clone(), true, 50),
        TestConfig::new(ExchangeId::MEXC, test_symbols.clone(), true, 200),
        TestConfig::new(ExchangeId::GateIo, test_symbols.clone(), false, 100),
        TestConfig::new(ExchangeId::Bitstamp, test_symbols.clone(), false, 20),
        TestConfig::new(ExchangeId::Kraken, test_symbols.clone(), false, 30),
    ]
}

/// Create connector instance based on exchange ID
fn create_connector(exchange_id: ExchangeId) -> Box<dyn ExchangeConnector + Send + Sync + 'static> {
    match exchange_id {
        ExchangeId::OKX => Box::new(OKXConnector::new()),
        ExchangeId::ByBit => Box::new(BybitConnector::new()),
        ExchangeId::MEXC => Box::new(MEXCConnector::new()),
        ExchangeId::GateIo => Box::new(GateioConnector::new()),
        ExchangeId::Bitstamp => Box::new(BitstampConnector::new()),
        ExchangeId::Kraken => Box::new(KrakenConnector::new()),
        _ => panic!("Unsupported exchange: {:?}", exchange_id),
    }
}

/// Test REST API endpoints for a connector
async fn test_rest_api(connector: &dyn ExchangeConnector, config: &TestConfig) -> TestResults {
    let mut results = TestResults::default();
    let exchange_name = format!("{:?}", config.exchange_id);

    // Test 1: Fetch symbols
    println!("  Testing fetch_symbols for {}...", exchange_name);
    match timeout(Duration::from_secs(30), connector.fetch_symbols()).await {
        Ok(Ok(symbols)) => {
            let success = symbols.len() >= config.expected_min_symbols;
            results.add_test(
                &format!("{} fetch_symbols", exchange_name),
                success,
                Some(format!(
                    "Found {} symbols (expected >= {})",
                    symbols.len(),
                    config.expected_min_symbols
                )),
            );
        }
        Ok(Err(e)) => {
            results.add_test(
                &format!("{} fetch_symbols", exchange_name),
                false,
                Some(format!("Error: {}", e)),
            );
        }
        Err(_) => {
            results.add_test(
                &format!("{} fetch_symbols", exchange_name),
                false,
                Some("Timeout".to_string()),
            );
        }
    }

    // Test 2: Fetch order books
    for symbol in &config.test_symbols {
        println!(
            "  Testing fetch_order_book for {} {}...",
            exchange_name, symbol
        );
        match timeout(Duration::from_secs(15), connector.fetch_order_book(symbol)).await {
            Ok(Ok(order_book)) => {
                let has_bids = !order_book.bids.is_empty();
                let has_asks = !order_book.asks.is_empty();
                let success = has_bids && has_asks && order_book.exchange == config.exchange_id;
                results.add_test(
                    &format!("{} fetch_order_book({})", exchange_name, symbol),
                    success,
                    Some(format!(
                        "Bids: {}, Asks: {}",
                        order_book.bids.len(),
                        order_book.asks.len()
                    )),
                );
            }
            Ok(Err(e)) => {
                results.add_test(
                    &format!("{} fetch_order_book({})", exchange_name, symbol),
                    false,
                    Some(format!("Error: {}", e)),
                );
            }
            Err(_) => {
                results.add_test(
                    &format!("{} fetch_order_book({})", exchange_name, symbol),
                    false,
                    Some("Timeout".to_string()),
                );
            }
        }
    }

    // Test 3: Fetch tickers
    println!("  Testing fetch_tickers for {}...", exchange_name);
    match timeout(
        Duration::from_secs(20),
        connector.fetch_tickers(&config.test_symbols),
    )
    .await
    {
        Ok(Ok(tickers)) => {
            let success = !tickers.is_empty();
            results.add_test(
                &format!("{} fetch_tickers", exchange_name),
                success,
                Some(format!("Found {} tickers", tickers.len())),
            );
        }
        Ok(Err(e)) => {
            results.add_test(
                &format!("{} fetch_tickers", exchange_name),
                false,
                Some(format!("Error: {}", e)),
            );
        }
        Err(_) => {
            results.add_test(
                &format!("{} fetch_tickers", exchange_name),
                false,
                Some("Timeout".to_string()),
            );
        }
    }

    // Test 4: Fetch funding rates (if supported)
    if config.supports_funding_rates {
        println!("  Testing fetch_funding_rates for {}...", exchange_name);
        match timeout(
            Duration::from_secs(20),
            connector.fetch_funding_rates(&config.test_symbols),
        )
        .await
        {
            Ok(Ok(funding_rates)) => {
                results.add_test(
                    &format!("{} fetch_funding_rates", exchange_name),
                    true,
                    Some(format!("Found {} funding rates", funding_rates.len())),
                );
            }
            Ok(Err(e)) => {
                results.add_test(
                    &format!("{} fetch_funding_rates", exchange_name),
                    false,
                    Some(format!("Error: {}", e)),
                );
            }
            Err(_) => {
                results.add_test(
                    &format!("{} fetch_funding_rates", exchange_name),
                    false,
                    Some("Timeout".to_string()),
                );
            }
        }
    }

    // Test 5: Health check
    println!("  Testing health_check for {}...", exchange_name);
    match timeout(Duration::from_secs(10), connector.health_check()).await {
        Ok(Ok(health)) => {
            results.add_test(
                &format!("{} health_check", exchange_name),
                true,
                Some(format!("REST status: {:?}", health.rest_api_status)),
            );
        }
        Ok(Err(e)) => {
            results.add_test(
                &format!("{} health_check", exchange_name),
                false,
                Some(format!("Error: {}", e)),
            );
        }
        Err(_) => {
            results.add_test(
                &format!("{} health_check", exchange_name),
                false,
                Some("Timeout".to_string()),
            );
        }
    }

    results
}

/// Test WebSocket functionality for a connector
async fn test_websocket(connector: &mut dyn ExchangeConnector, config: &TestConfig) -> TestResults {
    let mut results = TestResults::default();
    let exchange_name = format!("{:?}", config.exchange_id);

    // Test 1: Initial connection status
    let initial_status = connector.status();
    results.add_test(
        &format!("{} initial_status", exchange_name),
        initial_status == ConnectionStatus::Disconnected,
        Some(format!("Status: {:?}", initial_status)),
    );

    // Test 2: WebSocket connection
    println!("  Testing WebSocket connect for {}...", exchange_name);
    match timeout(Duration::from_secs(30), connector.connect()).await {
        Ok(Ok(_)) => {
            results.add_test(
                &format!("{} websocket_connect", exchange_name),
                true,
                Some("Connected successfully".to_string()),
            );

            // Wait a moment for connection to stabilize
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // Test 3: Connection status after connect
            let connected_status = connector.status();
            results.add_test(
                &format!("{} connected_status", exchange_name),
                connected_status == ConnectionStatus::Connected,
                Some(format!("Status: {:?}", connected_status)),
            );

            // Test 4: Event receiver
            let mut event_receiver = connector.event_receiver();
            results.add_test(
                &format!("{} event_receiver", exchange_name),
                true,
                Some("Event receiver created".to_string()),
            );

            // Test 5: Subscribe to order books
            println!("  Testing order book subscription for {}...", exchange_name);
            match timeout(
                Duration::from_secs(15),
                connector.subscribe_order_books(&config.test_symbols),
            )
            .await
            {
                Ok(Ok(_)) => {
                    results.add_test(
                        &format!("{} subscribe_order_books", exchange_name),
                        true,
                        Some(format!(
                            "Subscribed to {} symbols",
                            config.test_symbols.len()
                        )),
                    );

                    // Test 6: Wait for market data events
                    println!("  Waiting for market data from {}...", exchange_name);
                    let mut received_events = 0;
                    let start_time = std::time::Instant::now();

                    // Subscribe to symbols first to ensure we get data
                    if let Err(e) = connector.subscribe_order_books(&config.test_symbols).await {
                        println!("    Warning: Failed to subscribe to order books: {}", e);
                    }

                    while received_events < 3 && start_time.elapsed() < Duration::from_secs(45) {
                        match timeout(Duration::from_secs(5), event_receiver.recv()).await {
                            Ok(Ok(event)) => match event {
                                ConnectionEvent::MarketData(_) => {
                                    received_events += 1;
                                    println!(
                                        "    Received market data event {} from {}",
                                        received_events, exchange_name
                                    );
                                }
                                ConnectionEvent::StatusChange { new_status, .. } => {
                                    println!(
                                        "    Status change to {:?} for {}",
                                        new_status, exchange_name
                                    );
                                }
                                ConnectionEvent::SubscriptionConfirmed { .. } => {
                                    println!("    Subscription confirmed for {}", exchange_name);
                                }
                                ConnectionEvent::Error { error, .. } => {
                                    println!("    Error from {}: {}", exchange_name, error);
                                }
                                _ => {}
                            },
                            Ok(Err(_)) => {
                                // Channel closed or lagged
                                break;
                            }
                            Err(_) => {
                                // Timeout waiting for event
                                continue;
                            }
                        }
                    }

                    results.add_test(
                        &format!("{} receive_market_data", exchange_name),
                        received_events > 0,
                        Some(format!("Received {} market data events", received_events)),
                    );

                    // Test 7: Subscribe to tickers
                    println!("  Testing ticker subscription for {}...", exchange_name);
                    match timeout(
                        Duration::from_secs(10),
                        connector.subscribe_tickers(&config.test_symbols),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            results.add_test(
                                &format!("{} subscribe_tickers", exchange_name),
                                true,
                                Some("Ticker subscription successful".to_string()),
                            );
                        }
                        Ok(Err(e)) => {
                            results.add_test(
                                &format!("{} subscribe_tickers", exchange_name),
                                false,
                                Some(format!("Error: {}", e)),
                            );
                        }
                        Err(_) => {
                            results.add_test(
                                &format!("{} subscribe_tickers", exchange_name),
                                false,
                                Some("Timeout".to_string()),
                            );
                        }
                    }

                    // Test 8: Unsubscribe from symbols
                    println!("  Testing unsubscribe for {}...", exchange_name);
                    match timeout(
                        Duration::from_secs(10),
                        connector.unsubscribe_symbols(&config.test_symbols),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            results.add_test(
                                &format!("{} unsubscribe_symbols", exchange_name),
                                true,
                                Some("Unsubscribe successful".to_string()),
                            );
                        }
                        Ok(Err(e)) => {
                            results.add_test(
                                &format!("{} unsubscribe_symbols", exchange_name),
                                false,
                                Some(format!("Error: {}", e)),
                            );
                        }
                        Err(_) => {
                            results.add_test(
                                &format!("{} unsubscribe_symbols", exchange_name),
                                false,
                                Some("Timeout".to_string()),
                            );
                        }
                    }
                }
                Ok(Err(e)) => {
                    results.add_test(
                        &format!("{} subscribe_order_books", exchange_name),
                        false,
                        Some(format!("Error: {}", e)),
                    );
                }
                Err(_) => {
                    results.add_test(
                        &format!("{} subscribe_order_books", exchange_name),
                        false,
                        Some("Timeout".to_string()),
                    );
                }
            }

            // Test 9: Force reconnect
            println!("  Testing force reconnect for {}...", exchange_name);
            match timeout(Duration::from_secs(20), connector.force_reconnect()).await {
                Ok(Ok(_)) => {
                    results.add_test(
                        &format!("{} force_reconnect", exchange_name),
                        true,
                        Some("Reconnect successful".to_string()),
                    );
                }
                Ok(Err(e)) => {
                    results.add_test(
                        &format!("{} force_reconnect", exchange_name),
                        false,
                        Some(format!("Error: {}", e)),
                    );
                }
                Err(_) => {
                    results.add_test(
                        &format!("{} force_reconnect", exchange_name),
                        false,
                        Some("Timeout".to_string()),
                    );
                }
            }

            // Test 10: Disconnect
            println!("  Testing disconnect for {}...", exchange_name);
            match timeout(Duration::from_secs(10), connector.disconnect()).await {
                Ok(Ok(_)) => {
                    let disconnected_status = connector.status();
                    results.add_test(
                        &format!("{} disconnect", exchange_name),
                        disconnected_status == ConnectionStatus::Disconnected,
                        Some(format!("Final status: {:?}", disconnected_status)),
                    );
                }
                Ok(Err(e)) => {
                    results.add_test(
                        &format!("{} disconnect", exchange_name),
                        false,
                        Some(format!("Error: {}", e)),
                    );
                }
                Err(_) => {
                    results.add_test(
                        &format!("{} disconnect", exchange_name),
                        false,
                        Some("Timeout".to_string()),
                    );
                }
            }
        }
        Ok(Err(e)) => {
            results.add_test(
                &format!("{} websocket_connect", exchange_name),
                false,
                Some(format!("Error: {}", e)),
            );
        }
        Err(_) => {
            results.add_test(
                &format!("{} websocket_connect", exchange_name),
                false,
                Some("Timeout".to_string()),
            );
        }
    }

    results
}

/// Test connector statistics and monitoring
async fn test_connector_stats(
    connector: &dyn ExchangeConnector,
    config: &TestConfig,
) -> TestResults {
    let mut results = TestResults::default();
    let exchange_name = format!("{:?}", config.exchange_id);

    // Test 1: Get stats
    let stats = connector.get_stats();
    results.add_test(
        &format!("{} get_stats", exchange_name),
        stats.exchange == config.exchange_id,
        Some(format!(
            "Exchange: {:?}, Messages: {}",
            stats.exchange, stats.messages_received
        )),
    );

    // Test 2: Health check
    match timeout(Duration::from_secs(10), connector.health_check()).await {
        Ok(Ok(health)) => {
            results.add_test(
                &format!("{} health_status", exchange_name),
                true,
                Some(format!(
                    "Connected: {}, Errors: {}",
                    health.is_connected, health.error_count
                )),
            );
        }
        Ok(Err(e)) => {
            results.add_test(
                &format!("{} health_status", exchange_name),
                false,
                Some(format!("Error: {}", e)),
            );
        }
        Err(_) => {
            results.add_test(
                &format!("{} health_status", exchange_name),
                false,
                Some("Timeout".to_string()),
            );
        }
    }

    results
}

/// Run comprehensive tests for a single exchange
async fn test_exchange(config: TestConfig) -> TestResults {
    let mut combined_results = TestResults::default();
    let exchange_name = format!("{:?}", config.exchange_id);

    println!("\n🔄 Testing {} Exchange", exchange_name);
    println!("{}", "=".repeat(50));

    let mut connector = create_connector(config.exchange_id);

    // Test REST API
    println!("\n📡 Testing REST API for {}...", exchange_name);
    let rest_results = test_rest_api(connector.as_ref(), &config).await;

    // Test WebSocket
    println!("\n🔌 Testing WebSocket for {}...", exchange_name);
    let ws_results = test_websocket(connector.as_mut(), &config).await;

    // Test Stats and Monitoring
    println!("\n📊 Testing Stats for {}...", exchange_name);
    let stats_results = test_connector_stats(connector.as_ref(), &config).await;

    // Combine all results
    combined_results.total_tests =
        rest_results.total_tests + ws_results.total_tests + stats_results.total_tests;
    combined_results.passed_tests =
        rest_results.passed_tests + ws_results.passed_tests + stats_results.passed_tests;
    combined_results.failed_tests =
        rest_results.failed_tests + ws_results.failed_tests + stats_results.failed_tests;

    combined_results
        .test_details
        .extend(rest_results.test_details);
    combined_results
        .test_details
        .extend(ws_results.test_details);
    combined_results
        .test_details
        .extend(stats_results.test_details);

    println!(
        "\n📋 {} Results: {}/{} tests passed ({:.1}%)",
        exchange_name,
        combined_results.passed_tests,
        combined_results.total_tests,
        combined_results.success_rate()
    );

    combined_results
}

#[tokio::test]
async fn test_all_exchange_connectors() {
    println!("🚀 Starting Comprehensive Exchange Connector Integration Tests");
    println!("{}", "=".repeat(80));

    let configs = get_test_configs();
    let mut all_results = HashMap::new();
    let mut total_tests = 0;
    let mut total_passed = 0;

    // Test each exchange
    for config in configs {
        let exchange_name = format!("{:?}", config.exchange_id);
        let results = test_exchange(config).await;

        total_tests += results.total_tests;
        total_passed += results.passed_tests;

        all_results.insert(exchange_name, results);
    }

    // Print comprehensive summary
    println!("\n\n📊 COMPREHENSIVE TEST SUMMARY");
    println!("{}", "=".repeat(80));

    for (exchange, results) in &all_results {
        println!("\n🏢 {} Exchange:", exchange);
        println!(
            "   Tests: {}/{} passed ({:.1}%)",
            results.passed_tests,
            results.total_tests,
            results.success_rate()
        );

        // Show failed tests
        let failed_tests: Vec<&String> = results
            .test_details
            .iter()
            .filter(|detail| detail.starts_with("❌"))
            .collect();

        if !failed_tests.is_empty() {
            println!("   Failed tests:");
            for failed in failed_tests {
                println!("     {}", failed);
            }
        }
    }

    let overall_success_rate = if total_tests == 0 {
        0.0
    } else {
        (total_passed as f64 / total_tests as f64) * 100.0
    };

    println!("\n\n🎯 OVERALL RESULTS");
    println!("{}", "=".repeat(50));
    println!("Total Tests: {}", total_tests);
    println!("Passed: {}", total_passed);
    println!("Failed: {}", total_tests - total_passed);
    println!("Success Rate: {:.1}%", overall_success_rate);

    if overall_success_rate >= 80.0 {
        println!("🎉 EXCELLENT! All connectors are working well!");
    } else if overall_success_rate >= 60.0 {
        println!("⚠️  GOOD! Most functionality is working, some issues to address.");
    } else {
        println!("🚨 NEEDS ATTENTION! Several issues need to be fixed.");
    }

    println!("\n\n📋 DETAILED TEST BREAKDOWN");
    println!("{}", "=".repeat(80));

    for (exchange, results) in &all_results {
        println!("\n{} - Detailed Results:", exchange);
        for detail in &results.test_details {
            println!("  {}", detail);
        }
    }

    // The test passes if we achieve at least 70% success rate overall
    assert!(
        overall_success_rate >= 70.0,
        "Overall success rate {:.1}% is below the required 70% threshold. {} out of {} tests failed.",
        overall_success_rate,
        total_tests - total_passed,
        total_tests
    );
}

#[tokio::test]
async fn test_individual_okx_connector() {
    let config = TestConfig::new(ExchangeId::OKX, create_test_symbols(), true, 100);
    let results = test_exchange(config).await;

    assert!(
        results.success_rate() >= 70.0,
        "OKX connector success rate {:.1}% is below 70%",
        results.success_rate()
    );
}

#[tokio::test]
async fn test_individual_bybit_connector() {
    let config = TestConfig::new(ExchangeId::ByBit, create_test_symbols(), true, 50);
    let results = test_exchange(config).await;

    assert!(
        results.success_rate() >= 70.0,
        "ByBit connector success rate {:.1}% is below 70%",
        results.success_rate()
    );
}

#[tokio::test]
async fn test_individual_mexc_connector() {
    let config = TestConfig::new(ExchangeId::MEXC, create_test_symbols(), true, 200);
    let results = test_exchange(config).await;

    assert!(
        results.success_rate() >= 70.0,
        "MEXC connector success rate {:.1}% is below 70%",
        results.success_rate()
    );
}

#[tokio::test]
async fn test_individual_gateio_connector() {
    let config = TestConfig::new(ExchangeId::GateIo, create_test_symbols(), false, 100);
    let results = test_exchange(config).await;

    assert!(
        results.success_rate() >= 70.0,
        "Gate.io connector success rate {:.1}% is below 70%",
        results.success_rate()
    );
}

#[tokio::test]
async fn test_individual_bitstamp_connector() {
    let config = TestConfig::new(ExchangeId::Bitstamp, create_test_symbols(), false, 20);
    let results = test_exchange(config).await;

    assert!(
        results.success_rate() >= 70.0,
        "Bitstamp connector success rate {:.1}% is below 70%",
        results.success_rate()
    );
}

#[tokio::test]
async fn test_individual_kraken_connector() {
    let config = TestConfig::new(ExchangeId::Kraken, create_test_symbols(), false, 30);
    let results = test_exchange(config).await;

    assert!(
        results.success_rate() >= 70.0,
        "Kraken connector success rate {:.1}% is below 70%",
        results.success_rate()
    );
}

#[cfg(test)]
mod disconnected_exchange_tests {
    use super::*;
    use exchange_connectors::connector::ExchangeConnector;
    use reqwest::Client;
    use std::net::{SocketAddr, TcpListener};
    use tokio::net::TcpListener as AsyncTcpListener;
    use tokio::time::timeout;
    use warp::Filter;

    async fn start_mock_server() -> String {
        let route = warp::any().map(|| warp::reply::html("OK"));

        let addr = AsyncTcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = addr.local_addr().port();

        tokio::spawn(warp::serve(route).run(addr));

        format!("http://127.0.0.1:{}", port)
    }

    #[tokio::test]
    async fn test_fetch_orderbook_from_disconnected_exchange() {
        let url = start_mock_server().await;

        let mut config = ConnectorConfig::default();
        config.exchange_id = ExchangeId::OKX;
        config.rest_url = url;
        config.order_book_depth = 10;

        let connector = OKXConnector {
            base: ConnectorBase::new(config),
            client: Client::new(),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
            parsing_failures: Arc::new(AtomicU64::new(0)),
        };

        let symbol = Symbol::new("BTC", "USDT");
        let result = timeout(Duration::from_secs(5), connector.fetch_order_book(&symbol)).await;

        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Should fail when exchange is disconnected"
        );
    }

    #[tokio::test]
    async fn test_health_check_when_disconnected() {
        let url = start_mock_server().await;

        let mut config = ConnectorConfig::default();
        config.exchange_id = ExchangeId::OKX;
        config.rest_url = url;

        let connector = OKXConnector {
            base: ConnectorBase::new(config),
            client: Client::new(),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
            parsing_failures: Arc::new(AtomicU64::new(0)),
        };

        let health = connector.health_check().await;

        assert!(
            health.is_ok(),
            "Health check should not panic when disconnected"
        );
    }
}

#[cfg(test)]
mod malformed_response_tests {
    use super::*;
    use exchange_connectors::connector::ExchangeConnector;
    use reqwest::Client;
    use tokio::time::timeout;
    use warp::Filter;

    async fn start_malformed_server() -> String {
        let malformed_routes = warp::path("api").and(warp::any().map(|| {
            warp::reply::with_status(
                r#"{"invalid": "json", "missing": ["required", "fields"], "prices": "not_numbers"}"#,
                warp::http::StatusCode::OK,
            )
        }));

        let addr = AsyncTcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = addr.local_addr().port();

        tokio::spawn(warp::serve(malformed_routes).run(addr));

        format!("http://127.0.0.1:{}", port)
    }

    async fn start_empty_server() -> String {
        let empty_routes = warp::path("api")
            .and(warp::any().map(|| warp::reply::with_status(r#"{}"#, warp::http::StatusCode::OK)));

        let addr = AsyncTcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = addr.local_addr().port();

        tokio::spawn(warp::serve(empty_routes).run(addr));

        format!("http://127.0.0.1:{}", port)
    }

    async fn start_truncated_json_server() -> String {
        let truncated_routes = warp::path("api").and(warp::any().map(|| {
            warp::reply::with_status(
                r#"{"data": [{"bid": "50000", "ask": "50001"#,
                warp::http::StatusCode::OK,
            )
        }));

        let addr = AsyncTcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = addr.local_addr().port();

        tokio::spawn(warp::serve(truncated_routes).run(addr));

        format!("http://127.0.0.1:{}", port)
    }

    #[tokio::test]
    async fn test_parse_malformed_orderbook_response() {
        let url = start_malformed_server().await;

        let mut config = ConnectorConfig::default();
        config.exchange_id = ExchangeId::OKX;
        config.rest_url = url;
        config.order_book_depth = 10;

        let connector = OKXConnector {
            base: ConnectorBase::new(config),
            client: Client::new(),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
            parsing_failures: Arc::new(AtomicU64::new(0)),
        };

        let symbol = Symbol::new("BTC", "USDT");
        let result = timeout(Duration::from_secs(5), connector.fetch_order_book(&symbol)).await;

        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Should fail with malformed JSON response"
        );

        let failure_count = connector.get_parsing_failure_count();
        assert!(failure_count > 0, "Should track parsing failures");
    }

    #[tokio::test]
    async fn test_parse_empty_response() {
        let url = start_empty_server().await;

        let mut config = ConnectorConfig::default();
        config.exchange_id = ExchangeId::OKX;
        config.rest_url = url;
        config.order_book_depth = 10;

        let connector = OKXConnector {
            base: ConnectorBase::new(config),
            client: Client::new(),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
            parsing_failures: Arc::new(AtomicU64::new(0)),
        };

        let symbol = Symbol::new("BTC", "USDT");
        let result = timeout(Duration::from_secs(5), connector.fetch_order_book(&symbol)).await;

        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Should fail with empty JSON response"
        );
    }

    #[tokio::test]
    async fn test_parse_truncated_json_response() {
        let url = start_truncated_json_server().await;

        let mut config = ConnectorConfig::default();
        config.exchange_id = ExchangeId::OKX;
        config.rest_url = url;
        config.order_book_depth = 10;

        let connector = OKXConnector {
            base: ConnectorBase::new(config),
            client: Client::new(),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
            parsing_failures: Arc::new(AtomicU64::new(0)),
        };

        let symbol = Symbol::new("BTC", "USDT");
        let result = timeout(Duration::from_secs(5), connector.fetch_order_book(&symbol)).await;

        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Should fail with truncated JSON"
        );

        let failure_count = connector.get_parsing_failure_count();
        assert!(
            failure_count > 0,
            "Should track parsing failures for truncated JSON"
        );
    }

    #[tokio::test]
    async fn test_retry_logic_on_parsing_failure() {
        let attempt_counter = Arc::new(Mutex::new(0u32));
        let counter_clone = attempt_counter.clone();

        let routes = warp::path("api").and(warp::any().map(move || {
            let mut count = counter_clone.lock().unwrap();
            *count += 1;

            if *count < 3 {
                warp::reply::with_status("{invalid json", warp::http::StatusCode::OK)
            } else {
                warp::reply::with_status(
                    r#"{"data": [{"bid": "50000", "ask": "50001"}]}"#,
                    warp::http::StatusCode::OK,
                )
            }
        }));

        let addr = AsyncTcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = addr.local_addr().port();

        tokio::spawn(warp::serve(routes).run(addr));

        let mut config = ConnectorConfig::default();
        config.exchange_id = ExchangeId::OKX;
        config.rest_url = format!("http://127.0.0.1:{}", port);
        config.order_book_depth = 10;

        let connector = OKXConnector {
            base: ConnectorBase::new(config),
            client: Client::new(),
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
            parsing_failures: Arc::new(AtomicU64::new(0)),
        };

        let symbol = Symbol::new("BTC", "USDT");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = timeout(Duration::from_secs(10), connector.fetch_order_book(&symbol)).await;

        let attempts = *attempt_counter.lock().unwrap();
        assert!(
            attempts >= 3,
            "Should retry at least 3 times on parsing failures"
        );
    }
}
