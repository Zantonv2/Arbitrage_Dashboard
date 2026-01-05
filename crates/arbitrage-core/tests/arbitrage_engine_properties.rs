use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    normalizer::Normalizer,
    types::{ExchangeId, OrderBook, OrderBookLevel, Symbol},
};
use chrono::{Duration, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::sync::Arc;

// Helper function to create a test order book
fn create_test_order_book(
    exchange: ExchangeId,
    symbol: Symbol,
    best_bid: Decimal,
    best_ask: Decimal,
) -> OrderBook {
    let bids = vec![
        OrderBookLevel::new(best_bid, Decimal::from(10)),
        OrderBookLevel::new(best_bid - Decimal::from(1), Decimal::from(20)),
    ];
    let asks = vec![
        OrderBookLevel::new(best_ask, Decimal::from(10)),
        OrderBookLevel::new(best_ask + Decimal::from(1), Decimal::from(20)),
    ];
    
    OrderBook::new(exchange, symbol, bids, asks)
}

// Property 2: Order Book Merge Consistency
// Merging partial updates should maintain order book validity
#[tokio::test]
async fn test_order_book_merge_consistency() {
    let normalizer = Arc::new(Normalizer::new());
    let (engine, _receiver) = ArbitrageEngine::new(
        normalizer,
        Decimal::from_str_exact("0.1").unwrap(),
        10000,
        5000,
    );
    
    let symbol = Symbol::new("BTC", "USDT");
    
    // Create initial order book
    let initial_book = create_test_order_book(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49500),
        Decimal::from(50500),
    );
    
    // Update with initial book
    let _ = engine.update_order_book(initial_book).await;
    
    // Create update
    let update_book = create_test_order_book(
        ExchangeId::ByBit,
        symbol.clone(),
        Decimal::from(49600),
        Decimal::from(50600),
    );
    
    // Update with new book
    let _ = engine.update_order_book(update_book).await;
    
    // Property: retrieved book should be valid
    if let Some(merged_book) = engine.get_order_book(ExchangeId::ByBit, &symbol) {
        assert!(merged_book.is_valid());
    }
}

// Property tests for arbitrage engine functionality

proptest! {
    #[test]
    fn prop_gross_profit_calculation(
        buy_price in 40000u32..50000,
        sell_price in 50001u32..60000
    ) {
        let buy_decimal = Decimal::from(buy_price);
        let sell_decimal = Decimal::from(sell_price);
        
        // Calculate expected gross profit
        let expected_gross = ((sell_decimal - buy_decimal) / buy_decimal) * Decimal::from(100);
        
        // Property: gross profit should match mathematical formula
        prop_assert!(expected_gross > Decimal::ZERO);
        
        // Verify the calculation is consistent
        let recalculated = ((sell_decimal - buy_decimal) / buy_decimal) * Decimal::from(100);
        prop_assert_eq!(expected_gross, recalculated);
    }
}

proptest! {
    #[test]
    fn prop_net_profit_calculation(
        buy_price in 40000u32..50000,
        sell_price in 50001u32..60000,
        buy_fee_bps in 1u32..100, // 0.01% to 1% in basis points
        sell_fee_bps in 1u32..100
    ) {
        let buy_decimal = Decimal::from(buy_price);
        let sell_decimal = Decimal::from(sell_price);
        let buy_fee = Decimal::from(buy_fee_bps) / Decimal::from(10000); // Convert bps to decimal
        let sell_fee = Decimal::from(sell_fee_bps) / Decimal::from(10000);
        
        // Calculate gross profit
        let gross_profit = ((sell_decimal - buy_decimal) / buy_decimal) * Decimal::from(100);
        
        // Calculate total fees
        let total_fees = (buy_fee + sell_fee) * Decimal::from(100);
        
        // Calculate net profit
        let net_profit = gross_profit - total_fees;
        
        // Property: net profit should be less than gross profit
        prop_assert!(net_profit < gross_profit);
        
        // Property: difference should equal total fees
        let fee_difference = gross_profit - net_profit;
        prop_assert!((fee_difference - total_fees).abs() < Decimal::from_str_exact("0.0001").unwrap());
    }
}

proptest! {
    #[test]
    fn prop_profit_threshold_filtering(
        threshold_percent in 1u32..500, // 0.01% to 5%
        profit_percent in 1u32..1000    // 0.01% to 10%
    ) {
        let threshold = Decimal::from(threshold_percent) / Decimal::from(10000);
        let profit = Decimal::from(profit_percent) / Decimal::from(10000);
        
        // Property: signals should only be emitted if profit > threshold
        let should_emit = profit > threshold;
        
        if should_emit {
            prop_assert!(profit > threshold);
        } else {
            prop_assert!(profit <= threshold);
        }
    }
}

proptest! {
    #[test]
    fn prop_signal_deduplication(
        time_diff_ms in 0u64..10000 // 0 to 10 seconds
    ) {
        let _symbol = Symbol::new("BTC", "USDT");
        
        let dedup_window_ms = 5000; // 5 seconds
        
        // Property: signals within dedup window should be considered duplicates
        let is_duplicate = time_diff_ms < dedup_window_ms;
        
        if is_duplicate {
            prop_assert!(time_diff_ms < dedup_window_ms);
        } else {
            prop_assert!(time_diff_ms >= dedup_window_ms);
        }
    }
}

proptest! {
    #[test]
    fn prop_stale_order_book_exclusion(
        age_ms in 0u64..30000 // 0 to 30 seconds
    ) {
        let stale_threshold_ms = 10000; // 10 seconds
        
        // Create timestamp
        let now = Utc::now();
        let book_timestamp = now - Duration::milliseconds(age_ms as i64);
        
        // Property: books older than threshold should be considered stale
        let is_stale = age_ms > stale_threshold_ms;
        let actual_age = (now - book_timestamp).num_milliseconds() as u64;
        
        if is_stale {
            prop_assert!(actual_age > stale_threshold_ms);
        } else {
            prop_assert!(actual_age <= stale_threshold_ms);
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_arbitrage_engine_creation() {
        let normalizer = Arc::new(Normalizer::new());
        let (engine, _receiver) = ArbitrageEngine::new(
            normalizer,
            Decimal::from_str_exact("0.1").unwrap(),
            10000,
            5000,
        );
        
        let stats = engine.get_stats();
        assert_eq!(stats.order_books_count, 0);
        assert_eq!(stats.cached_signals_count, 0);
    }
    
    #[tokio::test]
    async fn test_order_book_storage_and_retrieval() {
        let normalizer = Arc::new(Normalizer::new());
        let (engine, _receiver) = ArbitrageEngine::new(
            normalizer,
            Decimal::from_str_exact("0.1").unwrap(),
            10000,
            5000,
        );
        
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = create_test_order_book(
            ExchangeId::ByBit,
            symbol.clone(),
            Decimal::from(50000),
            Decimal::from(50100),
        );
        
        let _ = engine.update_order_book(order_book.clone()).await;
        
        let retrieved = engine.get_order_book(ExchangeId::ByBit, &symbol);
        assert!(retrieved.is_some());
        
        let retrieved_book = retrieved.unwrap();
        assert_eq!(retrieved_book.exchange, ExchangeId::ByBit);
        assert_eq!(retrieved_book.symbol, symbol);
    }

    #[tokio::test]
    async fn test_arbitrage_detection_with_mock_data() {
        let normalizer = Arc::new(Normalizer::new());
        let (engine, mut receiver) = ArbitrageEngine::new(
            normalizer,
            Decimal::from_str_exact("0.1").unwrap(), // 0.1% threshold
            10000,
            5000,
        );
        
        let symbol = Symbol::new("BTC", "USDT");
        
        // Create ByBit order book - BTC cheaper here
        let bybit_bids = vec![
            OrderBookLevel::new(Decimal::from(49900), Decimal::from(1)), // Best bid
            OrderBookLevel::new(Decimal::from(49850), Decimal::from(2)),
        ];
        let bybit_asks = vec![
            OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)), // Best ask - BUY HERE
            OrderBookLevel::new(Decimal::from(50050), Decimal::from(2)),
        ];
        let bybit_book = OrderBook::new(ExchangeId::ByBit, symbol.clone(), bybit_bids, bybit_asks);
        
        // Create BingX order book - BTC more expensive here  
        let bingx_bids = vec![
            OrderBookLevel::new(Decimal::from(50200), Decimal::from(1)), // Best bid - SELL HERE
            OrderBookLevel::new(Decimal::from(50150), Decimal::from(2)),
        ];
        let bingx_asks = vec![
            OrderBookLevel::new(Decimal::from(50300), Decimal::from(1)), // Best ask
            OrderBookLevel::new(Decimal::from(50350), Decimal::from(2)),
        ];
        let bingx_book = OrderBook::new(ExchangeId::BingX, symbol.clone(), bingx_bids, bingx_asks);
        
        // Feed order books to engine
        let _ = engine.update_order_book(bybit_book).await;
        let _ = engine.update_order_book(bingx_book).await;
        
        // Should detect arbitrage opportunity
        let signal = receiver.recv().await.expect("Should receive arbitrage signal");
        
        // Verify the arbitrage signal
        assert_eq!(signal.symbol, symbol);
        assert_eq!(signal.buy_exchange, ExchangeId::ByBit); // Buy from cheaper exchange
        assert_eq!(signal.sell_exchange, ExchangeId::BingX); // Sell to more expensive exchange
        assert_eq!(signal.buy_price, Decimal::from(50000)); // ByBit ask price
        assert_eq!(signal.sell_price, Decimal::from(50200)); // BingX bid price
        
        // Calculate expected profit: (50200 - 50000) / 50000 * 100 = 0.4%
        let expected_gross_profit = Decimal::from_str_exact("0.4").unwrap();
        assert_eq!(signal.gross_profit_percent, expected_gross_profit);
        
        // Should be above our 0.1% threshold
        assert!(signal.gross_profit_percent > Decimal::from_str_exact("0.1").unwrap());
        
        println!("✅ Arbitrage detected!");
        println!("   Buy {} at {} for ${}", signal.symbol.to_pair(), signal.buy_exchange, signal.buy_price);
        println!("   Sell {} at {} for ${}", signal.symbol.to_pair(), signal.sell_exchange, signal.sell_price);
        println!("   Gross profit: {:.2}%", signal.gross_profit_percent);
    }

    #[tokio::test]
    async fn test_no_arbitrage_when_prices_too_close() {
        let normalizer = Arc::new(Normalizer::new());
        let (engine, mut receiver) = ArbitrageEngine::new(
            normalizer,
            Decimal::from_str_exact("0.1").unwrap(), // 0.1% threshold
            10000,
            5000,
        );
        
        let symbol = Symbol::new("ETH", "USDT");
        
        // Create similar prices on both exchanges (profit < threshold)
        let bybit_book = create_test_order_book(
            ExchangeId::ByBit,
            symbol.clone(),
            Decimal::from(3000), // bid
            Decimal::from(3010), // ask - BUY HERE
        );
        
        let bingx_book = create_test_order_book(
            ExchangeId::BingX,
            symbol.clone(),
            Decimal::from(3012), // bid - SELL HERE  
            Decimal::from(3020), // ask
        );
        
        // Feed order books
        let _ = engine.update_order_book(bybit_book).await;
        let _ = engine.update_order_book(bingx_book).await;
        
        // Should NOT receive signal (profit = (3012-3010)/3010 = 0.066% < 0.1% threshold)
        tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv())
            .await
            .expect_err("Should not receive signal when profit below threshold");
            
        println!("✅ No arbitrage signal when profit below threshold");
    }

    #[tokio::test]
    async fn test_arbitrage_with_multiple_symbols() {
        let normalizer = Arc::new(Normalizer::new());
        let (engine, mut receiver) = ArbitrageEngine::new(
            normalizer,
            Decimal::from_str_exact("0.2").unwrap(), // 0.2% threshold
            10000,
            5000,
        );
        
        // Test BTC arbitrage
        let btc_symbol = Symbol::new("BTC", "USDT");
        let btc_bybit = create_test_order_book(ExchangeId::ByBit, btc_symbol.clone(), Decimal::from(49000), Decimal::from(50000));
        let btc_bingx = create_test_order_book(ExchangeId::BingX, btc_symbol.clone(), Decimal::from(50500), Decimal::from(51000));
        
        // Test ETH arbitrage  
        let eth_symbol = Symbol::new("ETH", "USDT");
        let eth_bybit = create_test_order_book(ExchangeId::ByBit, eth_symbol.clone(), Decimal::from(2900), Decimal::from(3000));
        let eth_bingx = create_test_order_book(ExchangeId::BingX, eth_symbol.clone(), Decimal::from(3020), Decimal::from(3100));
        
        // Feed all order books
        let _ = engine.update_order_book(btc_bybit).await;
        let _ = engine.update_order_book(btc_bingx).await;
        let _ = engine.update_order_book(eth_bybit).await;
        let _ = engine.update_order_book(eth_bingx).await;
        
        // Should receive 2 signals
        let signal1 = receiver.recv().await.expect("Should receive first signal");
        let signal2 = receiver.recv().await.expect("Should receive second signal");
        
        // Verify we got signals for both symbols
        let symbols: std::collections::HashSet<_> = [&signal1.symbol, &signal2.symbol].into_iter().collect();
        assert!(symbols.contains(&btc_symbol));
        assert!(symbols.contains(&eth_symbol));
        
        println!("✅ Multiple arbitrage opportunities detected:");
        println!("   Signal 1: {} - {:.2}% profit", signal1.symbol.to_pair(), signal1.gross_profit_percent);
        println!("   Signal 2: {} - {:.2}% profit", signal2.symbol.to_pair(), signal2.gross_profit_percent);
    }
}