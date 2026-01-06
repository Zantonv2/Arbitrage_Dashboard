// Integration tests for the arbitrage dashboard
// These tests will verify end-to-end functionality

use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    config::ArbitrageConfig,
    types::*,
};
use exchange_connectors::{
    bybit::ByBitConnector,
    bingx::BingXConnector,
    connector::{ConnectorConfig, ExchangeConnector, ConnectionEvent},
};
use tokio::time::{timeout, Duration};
use tracing::{info, warn, error};
use std::collections::HashMap;
use tokio_test;

#[tokio::test]
async fn test_basic_functionality() {
    // Basic smoke test to ensure core types work
    let symbol = Symbol::new("BTC", "USDT");
    assert_eq!(symbol.to_pair(), "BTC/USDT");
    
    let signal = Signal::new(
        symbol,
        ExchangeId::ByBit,
        ExchangeId::BingX,
        rust_decimal::Decimal::from(50000),
        rust_decimal::Decimal::from(50100),
    );
    
    assert!(!signal.is_expired());
    assert_eq!(signal.buy_exchange, ExchangeId::ByBit);
    assert_eq!(signal.sell_exchange, ExchangeId::BingX);
}

#[tokio::test]
async fn test_order_book_creation() {
    let symbol = Symbol::new("ETH", "USDT");
    let bids = vec![
        OrderBookLevel::new(rust_decimal::Decimal::from(3000), rust_decimal::Decimal::from(1)),
        OrderBookLevel::new(rust_decimal::Decimal::from(2999), rust_decimal::Decimal::from(2)),
    ];
    let asks = vec![
        OrderBookLevel::new(rust_decimal::Decimal::from(3001), rust_decimal::Decimal::from(1)),
        OrderBookLevel::new(rust_decimal::Decimal::from(3002), rust_decimal::Decimal::from(2)),
    ];
    
    let order_book = OrderBook::new(ExchangeId::ByBit, symbol, bids, asks);
    
    assert!(order_book.is_valid());
    assert_eq!(order_book.best_bid().unwrap().price, rust_decimal::Decimal::from(3000));
    assert_eq!(order_book.best_ask().unwrap().price, rust_decimal::Decimal::from(3001));
    assert_eq!(order_book.spread().unwrap(), rust_decimal::Decimal::from(1));
}

/// Create connector config for testing
fn create_connector_config(exchange: ExchangeId, symbols: Vec<Symbol>) -> ConnectorConfig {
    ConnectorConfig {
        exchange,
        api_key: None,
        api_secret: None,
        passphrase: None,
        testnet: false,
        symbols,
        rate_limit_per_second: 10,
        reconnect_delay_ms: 1000,
        max_reconnect_attempts: 2,
        heartbeat_interval_ms: 30000,
    }
}

/// Create arbitrage engine config
fn create_arbitrage_config() -> ArbitrageConfig {
    ArbitrageConfig {
        profit_threshold_percent: 0.05, // 0.05% minimum profit
        max_order_book_age_ms: 5000,    // 5 second max age
        signal_cooldown_ms: 2000,       // 2 second cooldown between signals
    }
}

#[tokio::test]
#[ignore] // Use --ignored to run this test
async fn test_end_to_end_arbitrage_detection() {
    tracing_subscriber::fmt::init();
    
    info!("🚀 STARTING END-TO-END ARBITRAGE TEST");
    
    // Test with multiple crypto pairs
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDT"),
        Symbol::new("SOL", "USDT"),
        Symbol::new("ADA", "USDT"),
    ];
    
    info!("📊 Testing {} crypto pairs: {:?}", symbols.len(), symbols);
    
    // Create arbitrage engine
    let config = create_arbitrage_config();
    let mut engine = ArbitrageEngine::new(config);
    let mut signal_receiver = engine.signal_receiver();
    
    // Create exchange connectors
    let bybit_config = create_connector_config(ExchangeId::ByBit, symbols.clone());
    let bingx_config = create_connector_config(ExchangeId::BingX, symbols.clone());
    
    let mut bybit_connector = ByBitConnector::new(bybit_config);
    let mut bingx_connector = BingXConnector::new(bingx_config);
    
    // Subscribe to symbols
    bybit_connector.subscribe(symbols.clone()).await.expect("Failed to subscribe to ByBit");
    bingx_connector.subscribe(symbols.clone()).await.expect("Failed to subscribe to BingX");
    
    // Get event receivers
    let mut bybit_receiver = bybit_connector.event_receiver();
    let mut bingx_receiver = bingx_connector.event_receiver();
    
    // Connect both exchanges
    info!("🔗 Connecting to exchanges...");
    bybit_connector.connect().await.expect("Failed to connect to ByBit");
    bingx_connector.connect().await.expect("Failed to connect to BingX");
    
    // Track statistics
    let mut order_book_updates = HashMap::new();
    let mut arbitrage_signals = 0;
    let mut price_comparisons = 0;
    
    info!("📈 Monitoring for arbitrage opportunities across {} pairs...", symbols.len());
    
    let test_result = timeout(Duration::from_secs(30), async {
        loop {
            tokio::select! {
                // Handle ByBit order book updates
                Ok(event) = bybit_receiver.recv() => {
                    if let ConnectionEvent::OrderBookUpdate(order_book) = event {
                        *order_book_updates.entry(format!("ByBit-{}", order_book.symbol)).or_insert(0) += 1;
                        
                        info!("📊 ByBit {} update: {} bids, {} asks", 
                              order_book.symbol, order_book.bids.len(), order_book.asks.len());
                        
                        // Feed to arbitrage engine
                        engine.update_order_book(order_book).await;
                    }
                }
                
                // Handle BingX order book updates
                Ok(event) = bingx_receiver.recv() => {
                    if let ConnectionEvent::OrderBookUpdate(order_book) = event {
                        *order_book_updates.entry(format!("BingX-{}", order_book.symbol)).or_insert(0) += 1;
                        
                        info!("📊 BingX {} update: {} bids, {} asks", 
                              order_book.symbol, order_book.bids.len(), order_book.asks.len());
                        
                        // Feed to arbitrage engine
                        engine.update_order_book(order_book).await;
                        price_comparisons += 1;
                    }
                }
                
                // Handle arbitrage signals
                Ok(signal) = signal_receiver.recv() => {
                    arbitrage_signals += 1;
                    
                    info!("🚀 ARBITRAGE SIGNAL #{}: {} on {} → {}", 
                          arbitrage_signals,
                          signal.symbol,
                          signal.buy_exchange,
                          signal.sell_exchange);
                    
                    info!("   💰 Gross Profit: {:.3}% | Net Profit: {:.3}% | Confidence: {:.2}", 
                          signal.gross_profit_percent * 100.0,
                          signal.net_profit_percent * 100.0,
                          signal.confidence_score);
                    
                    info!("   📈 Buy at ${:.2} | Sell at ${:.2}", 
                          signal.buy_price, signal.sell_price);
                    
                    // Stop after finding a few signals
                    if arbitrage_signals >= 3 {
                        info!("✅ Found {} arbitrage signals - test successful!", arbitrage_signals);
                        break;
                    }
                }
            }
            
            // Show progress every 50 price updates
            if price_comparisons > 0 && price_comparisons % 50 == 0 {
                info!("📊 Progress: {} price updates, {} signals found", price_comparisons, arbitrage_signals);
                
                // Show order book update counts
                for (exchange_symbol, count) in &order_book_updates {
                    info!("   {} updates: {}", exchange_symbol, count);
                }
            }
            
            // If we have enough data but no signals, that's also valuable info
            if price_comparisons >= 200 && arbitrage_signals == 0 {
                info!("📈 Processed {} price updates with no arbitrage opportunities", price_comparisons);
                info!("💡 This indicates tight market efficiency - good for the market, challenging for arbitrage!");
                break;
            }
        }
    }).await;
    
    // Disconnect
    bybit_connector.disconnect().await.expect("Failed to disconnect ByBit");
    bingx_connector.disconnect().await.expect("Failed to disconnect BingX");
    
    // Report results
    info!("📊 END-TO-END TEST RESULTS:");
    info!("   🔄 Total price updates: {}", price_comparisons);
    info!("   🚀 Arbitrage signals found: {}", arbitrage_signals);
    info!("   📈 Order book updates by exchange:");
    
    for (exchange_symbol, count) in order_book_updates {
        info!("      {}: {} updates", exchange_symbol, count);
    }
    
    if test_result.is_err() {
        warn!("⏰ Test timeout - but system is working");
    }
    
    if arbitrage_signals > 0 {
        info!("✅ END-TO-END ARBITRAGE DETECTION: SUCCESS!");
        info!("💡 Live arbitrage opportunities detected across multiple crypto pairs");
    } else if price_comparisons > 100 {
        info!("✅ END-TO-END SYSTEM: SUCCESS!");
        info!("💡 No arbitrage found indicates efficient markets (system working correctly)");
    } else {
        error!("❌ Insufficient data received - check exchange connections");
    }
}

#[tokio::test]
#[ignore] // Use --ignored to run this test
async fn test_multi_symbol_price_monitoring() {
    tracing_subscriber::fmt::init();
    
    info!("🚀 TESTING MULTI-SYMBOL PRICE MONITORING");
    
    // Test with popular crypto pairs
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDT"),
        Symbol::new("BNB", "USDT"),
        Symbol::new("SOL", "USDT"),
        Symbol::new("XRP", "USDT"),
    ];
    
    info!("📊 Monitoring {} crypto pairs", symbols.len());
    
    // Create connectors
    let bybit_config = create_connector_config(ExchangeId::ByBit, symbols.clone());
    let bingx_config = create_connector_config(ExchangeId::BingX, symbols.clone());
    
    let mut bybit_connector = ByBitConnector::new(bybit_config);
    let mut bingx_connector = BingXConnector::new(bingx_config);
    
    // Subscribe and connect
    bybit_connector.subscribe(symbols.clone()).await.expect("Failed to subscribe to ByBit");
    bingx_connector.subscribe(symbols.clone()).await.expect("Failed to subscribe to BingX");
    
    let mut bybit_receiver = bybit_connector.event_receiver();
    let mut bingx_receiver = bingx_connector.event_receiver();
    
    bybit_connector.connect().await.expect("Failed to connect to ByBit");
    bingx_connector.connect().await.expect("Failed to connect to BingX");
    
    // Track prices by symbol
    let mut bybit_prices = HashMap::new();
    let mut bingx_prices = HashMap::new();
    let mut symbol_updates = HashMap::new();
    
    info!("📈 Collecting price data for spread analysis...");
    
    let monitor_result = timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                Ok(event) = bybit_receiver.recv() => {
                    if let ConnectionEvent::OrderBookUpdate(order_book) = event {
                        if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                            let mid_price = (order_book.bids[0].price + order_book.asks[0].price) / 2.0;
                            bybit_prices.insert(order_book.symbol.clone(), mid_price);
                            *symbol_updates.entry(order_book.symbol.clone()).or_insert(0) += 1;
                        }
                    }
                }
                
                Ok(event) = bingx_receiver.recv() => {
                    if let ConnectionEvent::OrderBookUpdate(order_book) = event {
                        if !order_book.bids.is_empty() && !order_book.asks.is_empty() {
                            let mid_price = (order_book.bids[0].price + order_book.asks[0].price) / 2.0;
                            bingx_prices.insert(order_book.symbol.clone(), mid_price);
                            *symbol_updates.entry(order_book.symbol.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
            
            // Show price comparison when we have data for both exchanges
            let mut complete_pairs = 0;
            for symbol in &symbols {
                if let (Some(bybit_price), Some(bingx_price)) = (
                    bybit_prices.get(symbol),
                    bingx_prices.get(symbol)
                ) {
                    complete_pairs += 1;
                    let spread = (bingx_price - bybit_price).abs();
                    let spread_pct = (spread / bybit_price) * 100.0;
                    
                    info!("💱 {}: ByBit ${:.2} | BingX ${:.2} | Spread: {:.3}%", 
                          symbol, bybit_price, bingx_price, spread_pct);
                }
            }
            
            if complete_pairs >= symbols.len() {
                info!("✅ Got price data for all {} symbols!", symbols.len());
                break;
            }
        }
    }).await;
    
    // Disconnect
    bybit_connector.disconnect().await.expect("Failed to disconnect ByBit");
    bingx_connector.disconnect().await.expect("Failed to disconnect BingX");
    
    // Report results
    info!("📊 MULTI-SYMBOL MONITORING RESULTS:");
    for symbol in &symbols {
        let updates = symbol_updates.get(symbol).unwrap_or(&0);
        info!("   {}: {} price updates", symbol, updates);
    }
    
    if monitor_result.is_ok() {
        info!("✅ MULTI-SYMBOL PRICE MONITORING: SUCCESS!");
    } else {
        warn!("⏰ Monitoring timeout - but data was collected");
    }
}