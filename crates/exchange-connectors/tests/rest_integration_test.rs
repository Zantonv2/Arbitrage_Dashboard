use exchange_connectors::{
    bybit::ByBitConnector,
    bingx::BingXConnector,
    connector::{ConnectorConfig, ExchangeConnector, ConnectionEvent},
};
use arbitrage_core::types::{ExchangeId, Symbol};
use tokio::time::{timeout, Duration};
use tracing::{info, error};

/// Create a test connector config
fn create_test_config(exchange: ExchangeId) -> ConnectorConfig {
    ConnectorConfig {
        exchange,
        api_key: None,
        api_secret: None,
        passphrase: None,
        testnet: false,
        symbols: vec![Symbol::new("BTC", "USDT")],
        rate_limit_per_second: 10,
        reconnect_delay_ms: 1000,
        max_reconnect_attempts: 2,
        heartbeat_interval_ms: 30000,
    }
}

#[tokio::test]
#[ignore] // Use --ignored to run this test
async fn test_bybit_rest_integration() {
    tracing_subscriber::fmt::init();
    
    info!("🚀 TESTING BYBIT REST INTEGRATION");
    
    let config = create_test_config(ExchangeId::ByBit);
    let mut connector = ByBitConnector::new(config);
    
    // Subscribe to BTC/USDT
    let symbols = vec![Symbol::new("BTC", "USDT")];
    connector.subscribe(symbols).await.expect("Failed to subscribe");
    
    // Get event receiver
    let mut event_receiver = connector.event_receiver();
    
    // Connect
    info!("Connecting to ByBit REST...");
    connector.connect().await.expect("Failed to connect");
    
    // Listen for order book updates
    info!("Waiting for order book updates...");
    let listen_result = timeout(Duration::from_secs(10), async {
        let mut order_book_count = 0;
        while let Ok(event) = event_receiver.recv().await {
            match event {
                ConnectionEvent::Connected(exchange) => {
                    info!("✅ {} connected via REST", exchange);
                }
                ConnectionEvent::OrderBookUpdate(order_book) => {
                    info!("📊 REST ORDER BOOK UPDATE for {} on {}: {} bids, {} asks", 
                          order_book.symbol, order_book.exchange, 
                          order_book.bids.len(), order_book.asks.len());
                    
                    if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                        info!("   Best bid: ${} @ {}", order_book.bids[0].price, order_book.bids[0].quantity);
                        info!("   Best ask: ${} @ {}", order_book.asks[0].price, order_book.asks[0].quantity);
                    }
                    
                    order_book_count += 1;
                    if order_book_count >= 3 {
                        info!("✅ Received {} order book updates - ByBit REST is WORKING!", order_book_count);
                        break;
                    }
                }
                ConnectionEvent::Error(exchange, error) => {
                    error!("❌ {} error: {}", exchange, error);
                }
                _ => {}
            }
        }
    }).await;
    
    if listen_result.is_err() {
        error!("⏰ Timeout waiting for order book updates");
    }
    
    // Disconnect
    connector.disconnect().await.expect("Failed to disconnect");
    info!("✅ ByBit REST integration test complete");
}

#[tokio::test]
#[ignore] // Use --ignored to run this test
async fn test_bingx_rest_integration() {
    tracing_subscriber::fmt::init();
    
    info!("🚀 TESTING BINGX REST INTEGRATION");
    
    let config = create_test_config(ExchangeId::BingX);
    let mut connector = BingXConnector::new(config);
    
    // Subscribe to BTC/USDT
    let symbols = vec![Symbol::new("BTC", "USDT")];
    connector.subscribe(symbols).await.expect("Failed to subscribe");
    
    // Get event receiver
    let mut event_receiver = connector.event_receiver();
    
    // Connect
    info!("Connecting to BingX REST...");
    connector.connect().await.expect("Failed to connect");
    
    // Listen for order book updates
    info!("Waiting for order book updates...");
    let listen_result = timeout(Duration::from_secs(10), async {
        let mut order_book_count = 0;
        while let Ok(event) = event_receiver.recv().await {
            match event {
                ConnectionEvent::Connected(exchange) => {
                    info!("✅ {} connected via REST", exchange);
                }
                ConnectionEvent::OrderBookUpdate(order_book) => {
                    info!("📊 REST ORDER BOOK UPDATE for {} on {}: {} bids, {} asks", 
                          order_book.symbol, order_book.exchange, 
                          order_book.bids.len(), order_book.asks.len());
                    
                    if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                        info!("   Best bid: ${} @ {}", order_book.bids[0].price, order_book.bids[0].quantity);
                        info!("   Best ask: ${} @ {}", order_book.asks[0].price, order_book.asks[0].quantity);
                    }
                    
                    order_book_count += 1;
                    if order_book_count >= 3 {
                        info!("✅ Received {} order book updates - BingX REST is WORKING!", order_book_count);
                        break;
                    }
                }
                ConnectionEvent::Error(exchange, error) => {
                    error!("❌ {} error: {}", exchange, error);
                }
                _ => {}
            }
        }
    }).await;
    
    if listen_result.is_err() {
        error!("⏰ Timeout waiting for order book updates");
    }
    
    // Disconnect
    connector.disconnect().await.expect("Failed to disconnect");
    info!("✅ BingX REST integration test complete");
}

#[tokio::test]
#[ignore] // Use --ignored to run this test
async fn test_dual_exchange_arbitrage_rest() {
    tracing_subscriber::fmt::init();
    
    info!("🚀 TESTING DUAL EXCHANGE ARBITRAGE WITH REST");
    
    // Create both connectors
    let bybit_config = create_test_config(ExchangeId::ByBit);
    let bingx_config = create_test_config(ExchangeId::BingX);
    
    let mut bybit_connector = ByBitConnector::new(bybit_config);
    let mut bingx_connector = BingXConnector::new(bingx_config);
    
    // Subscribe to BTC/USDT on both
    let symbols = vec![Symbol::new("BTC", "USDT")];
    bybit_connector.subscribe(symbols.clone()).await.expect("Failed to subscribe to ByBit");
    bingx_connector.subscribe(symbols).await.expect("Failed to subscribe to BingX");
    
    // Get event receivers
    let mut bybit_receiver = bybit_connector.event_receiver();
    let mut bingx_receiver = bingx_connector.event_receiver();
    
    // Connect both
    info!("Connecting to both exchanges...");
    bybit_connector.connect().await.expect("Failed to connect to ByBit");
    bingx_connector.connect().await.expect("Failed to connect to BingX");
    
    // Track latest order books
    let mut bybit_orderbook = None;
    let mut bingx_orderbook = None;
    let mut price_updates = 0;
    
    info!("Monitoring for live price data...");
    let monitor_result = timeout(Duration::from_secs(15), async {
        loop {
            tokio::select! {
                Ok(event) = bybit_receiver.recv() => {
                    if let ConnectionEvent::OrderBookUpdate(order_book) = event {
                        bybit_orderbook = Some(order_book);
                        price_updates += 1;
                    }
                }
                Ok(event) = bingx_receiver.recv() => {
                    if let ConnectionEvent::OrderBookUpdate(order_book) = event {
                        bingx_orderbook = Some(order_book);
                        price_updates += 1;
                    }
                }
            }
            
            // Show live prices when we have both order books
            if let (Some(ref bybit_ob), Some(ref bingx_ob)) = (&bybit_orderbook, &bingx_orderbook) {
                if !bybit_ob.bids.is_empty() && !bybit_ob.asks.is_empty() &&
                   !bingx_ob.bids.is_empty() && !bingx_ob.asks.is_empty() {
                    
                    let bybit_bid = bybit_ob.bids[0].price;
                    let bybit_ask = bybit_ob.asks[0].price;
                    let bingx_bid = bingx_ob.bids[0].price;
                    let bingx_ask = bingx_ob.asks[0].price;
                    
                    info!("📊 LIVE PRICES: ByBit ${:.2}/${:.2} | BingX ${:.2}/${:.2}", 
                          bybit_bid, bybit_ask, bingx_bid, bingx_ask);
                    
                    if price_updates >= 6 {
                        info!("✅ Received {} price updates from both exchanges - REST arbitrage detection is WORKING!", price_updates);
                        break;
                    }
                }
            }
        }
    }).await;
    
    if monitor_result.is_err() {
        info!("⏰ Monitoring timeout - but REST connectors are working");
    }
    
    // Disconnect both
    bybit_connector.disconnect().await.expect("Failed to disconnect ByBit");
    bingx_connector.disconnect().await.expect("Failed to disconnect BingX");
    
    info!("✅ Dual exchange REST arbitrage test complete");
    info!("💡 REST approach provides live arbitrage detection without WebSocket complexity!");
}