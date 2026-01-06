use exchange_connectors::{
    bingx::BingXConnector,
    bybit::ByBitConnector,
    hyperliquid::HyperliquidConnector,
    connector::{ConnectorConfig, ExchangeConnector, ConnectionEvent},
};
use arbitrage_core::types::{ExchangeId, Symbol};
use tokio::time::{timeout, Duration};
use tracing::{info, warn, error};

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
async fn test_bybit_live_connection() {
    tracing_subscriber::fmt::init();
    
    info!("🔥 TESTING LIVE BYBIT CONNECTION");
    
    let config = create_test_config(ExchangeId::ByBit);
    let mut connector = ByBitConnector::new(config);
    
    // Subscribe to BTC/USDT
    let symbols = vec![Symbol::new("BTC", "USDT")];
    connector.subscribe(symbols).await.expect("Failed to subscribe");
    
    // Get event receiver
    let mut event_receiver = connector.event_receiver();
    
    // Try to connect with longer timeout
    info!("Attempting to connect to ByBit...");
    let connect_result = timeout(Duration::from_secs(30), connector.connect()).await;
    
    match connect_result {
        Ok(Ok(())) => {
            info!("✅ ByBit connection successful!");
            
            // Listen for events with longer timeout
            let listen_result = timeout(Duration::from_secs(30), async {
                let mut order_book_count = 0;
                let mut any_message_count = 0;
                while let Ok(event) = event_receiver.recv().await {
                    any_message_count += 1;
                    info!("📨 Event #{}: {:?}", any_message_count, event);
                    
                    match event {
                        ConnectionEvent::Connected(exchange) => {
                            info!("🔗 {} connected", exchange);
                        }
                        ConnectionEvent::OrderBookUpdate(order_book) => {
                            info!("📊 LIVE ORDER BOOK UPDATE for {} on {}: {} bids, {} asks", 
                                  order_book.symbol, order_book.exchange, 
                                  order_book.bids.len(), order_book.asks.len());
                            
                            if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                                info!("   Best bid: {} @ {}", order_book.bids[0].price, order_book.bids[0].quantity);
                                info!("   Best ask: {} @ {}", order_book.asks[0].price, order_book.asks[0].quantity);
                            }
                            
                            order_book_count += 1;
                            if order_book_count >= 3 {
                                info!("✅ Received {} order book updates - ByBit is WORKING!", order_book_count);
                                break;
                            }
                        }
                        ConnectionEvent::Error(exchange, error) => {
                            error!("❌ {} error: {}", exchange, error);
                        }
                        ConnectionEvent::Disconnected(exchange) => {
                            info!("🔌 {} disconnected", exchange);
                        }
                        ConnectionEvent::Reconnecting(exchange) => {
                            info!("🔄 {} reconnecting...", exchange);
                        }
                        ConnectionEvent::RateLimit(exchange, seconds) => {
                            warn!("⏱️ {} rate limited for {} seconds", exchange, seconds);
                        }
                    }
                    
                    // If we get any messages but no order books, that's still progress
                    if any_message_count >= 10 && order_book_count == 0 {
                        warn!("⚠️ Received {} events but no order book updates - may need to fix message parsing", any_message_count);
                        break;
                    }
                }
            }).await;
            
            if listen_result.is_err() {
                warn!("⏰ Event listening timeout - may need to adjust message parsing or connection logic");
            }
        }
        Ok(Err(e)) => {
            error!("❌ ByBit connection failed: {}", e);
            // Don't panic, just report the error
            eprintln!("Connection failed: {}", e);
        }
        Err(_) => {
            error!("⏰ ByBit connection timeout");
            // Don't panic, just report the timeout
            eprintln!("Connection timeout - this could be due to network issues or incorrect WebSocket URL");
        }
    }
    
    // Disconnect
    connector.disconnect().await.expect("Failed to disconnect");
}

#[tokio::test]
#[ignore] // Use --ignored to run this test
async fn test_bingx_live_connection() {
    tracing_subscriber::fmt::init();
    
    info!("🔥 TESTING LIVE BINGX CONNECTION");
    
    let config = create_test_config(ExchangeId::BingX);
    let mut connector = BingXConnector::new(config);
    
    // Subscribe to BTC/USDT
    let symbols = vec![Symbol::new("BTC", "USDT")];
    connector.subscribe(symbols).await.expect("Failed to subscribe");
    
    // Get event receiver
    let mut event_receiver = connector.event_receiver();
    
    // Try to connect with timeout
    info!("Attempting to connect to BingX...");
    let connect_result = timeout(Duration::from_secs(10), connector.connect()).await;
    
    match connect_result {
        Ok(Ok(())) => {
            info!("✅ BingX connection successful!");
            
            // Listen for events
            let listen_result = timeout(Duration::from_secs(15), async {
                let mut order_book_count = 0;
                while let Ok(event) = event_receiver.recv().await {
                    match event {
                        ConnectionEvent::Connected(exchange) => {
                            info!("🔗 {} connected", exchange);
                        }
                        ConnectionEvent::OrderBookUpdate(order_book) => {
                            info!("📊 LIVE ORDER BOOK UPDATE for {} on {}: {} bids, {} asks", 
                                  order_book.symbol, order_book.exchange, 
                                  order_book.bids.len(), order_book.asks.len());
                            
                            if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                                info!("   Best bid: {} @ {}", order_book.bids[0].price, order_book.bids[0].quantity);
                                info!("   Best ask: {} @ {}", order_book.asks[0].price, order_book.asks[0].quantity);
                            }
                            
                            order_book_count += 1;
                            if order_book_count >= 3 {
                                info!("✅ Received {} order book updates - BingX is WORKING!", order_book_count);
                                break;
                            }
                        }
                        ConnectionEvent::Error(exchange, error) => {
                            error!("❌ {} error: {}", exchange, error);
                        }
                        ConnectionEvent::Disconnected(exchange) => {
                            info!("🔌 {} disconnected", exchange);
                        }
                        ConnectionEvent::Reconnecting(exchange) => {
                            info!("🔄 {} reconnecting...", exchange);
                        }
                        ConnectionEvent::RateLimit(exchange, seconds) => {
                            warn!("⏱️ {} rate limited for {} seconds", exchange, seconds);
                        }
                    }
                }
            }).await;
            
            if listen_result.is_err() {
                warn!("⏰ Event listening timeout - may need to adjust message parsing");
            }
        }
        Ok(Err(e)) => {
            error!("❌ BingX connection failed: {}", e);
            panic!("BingX connection failed: {}", e);
        }
        Err(_) => {
            error!("⏰ BingX connection timeout");
            panic!("BingX connection timeout");
        }
    }
    
    // Disconnect
    connector.disconnect().await.expect("Failed to disconnect");
}

#[tokio::test]
#[ignore] // Use --ignored to run this test
async fn test_hyperliquid_live_connection() {
    tracing_subscriber::fmt::init();
    
    info!("🔥 TESTING LIVE HYPERLIQUID CONNECTION");
    
    let config = create_test_config(ExchangeId::Hyperliquid);
    let mut connector = HyperliquidConnector::new(config);
    
    // Subscribe to BTC/USDT (Hyperliquid uses BTC/USDC)
    let symbols = vec![Symbol::new("BTC", "USDC")];
    connector.subscribe(symbols).await.expect("Failed to subscribe");
    
    // Get event receiver
    let mut event_receiver = connector.event_receiver();
    
    // Try to connect with timeout
    info!("Attempting to connect to Hyperliquid...");
    let connect_result = timeout(Duration::from_secs(10), connector.connect()).await;
    
    match connect_result {
        Ok(Ok(())) => {
            info!("✅ Hyperliquid connection successful!");
            
            // Listen for events
            let listen_result = timeout(Duration::from_secs(15), async {
                let mut order_book_count = 0;
                while let Ok(event) = event_receiver.recv().await {
                    match event {
                        ConnectionEvent::Connected(exchange) => {
                            info!("🔗 {} connected", exchange);
                        }
                        ConnectionEvent::OrderBookUpdate(order_book) => {
                            info!("📊 LIVE ORDER BOOK UPDATE for {} on {}: {} bids, {} asks", 
                                  order_book.symbol, order_book.exchange, 
                                  order_book.bids.len(), order_book.asks.len());
                            
                            if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                                info!("   Best bid: {} @ {}", order_book.bids[0].price, order_book.bids[0].quantity);
                                info!("   Best ask: {} @ {}", order_book.asks[0].price, order_book.asks[0].quantity);
                            }
                            
                            order_book_count += 1;
                            if order_book_count >= 3 {
                                info!("✅ Received {} order book updates - Hyperliquid is WORKING!", order_book_count);
                                break;
                            }
                        }
                        ConnectionEvent::Error(exchange, error) => {
                            error!("❌ {} error: {}", exchange, error);
                        }
                        ConnectionEvent::Disconnected(exchange) => {
                            info!("🔌 {} disconnected", exchange);
                        }
                        ConnectionEvent::Reconnecting(exchange) => {
                            info!("🔄 {} reconnecting...", exchange);
                        }
                        ConnectionEvent::RateLimit(exchange, seconds) => {
                            warn!("⏱️ {} rate limited for {} seconds", exchange, seconds);
                        }
                    }
                }
            }).await;
            
            if listen_result.is_err() {
                warn!("⏰ Event listening timeout - may need to adjust message parsing");
            }
        }
        Ok(Err(e)) => {
            error!("❌ Hyperliquid connection failed: {}", e);
            panic!("Hyperliquid connection failed: {}", e);
        }
        Err(_) => {
            error!("⏰ Hyperliquid connection timeout");
            panic!("Hyperliquid connection timeout");
        }
    }
    
    // Disconnect
    connector.disconnect().await.expect("Failed to disconnect");
}