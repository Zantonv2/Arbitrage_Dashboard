//! # Storage Module Tests
//!
//! Comprehensive tests for SQLite storage functionality including signals, executions,
//! and performance validation.

use arbitrage_core::{
    storage::{StorageService, StorageConfig, SignalStatus, ExecutionStatus, SignalQuery},
    types::{Signal, Symbol, ExchangeId, ExecutionInstruction, Order, Side, OrderType},
};
use rust_decimal::Decimal;
use uuid::Uuid;

/// Helper function to create test storage service
async fn create_test_storage() -> StorageService {
    let config = StorageConfig {
        database_path: ":memory:".to_string(), // In-memory SQLite for tests
        max_signal_history: 1000,
        max_execution_history: 500,
        enable_compression: false,
    };
    
    StorageService::new_async(config).await.expect("Failed to create test storage")
}

/// Helper function to create test signal
fn create_test_signal() -> Signal {
    let symbol = Symbol::new("BTC", "USDT");
    let buy_price = Decimal::new(50000, 0);
    let sell_price = Decimal::new(50100, 0);
    
    let mut signal = Signal::new(symbol, ExchangeId::OKX, ExchangeId::ByBit, buy_price, sell_price);
    signal.gross_profit_percent = Decimal::new(200, 4); // 2%
    signal.net_profit_percent = Decimal::new(150, 4); // 1.5%
    signal.confidence = Decimal::new(85, 2); // 0.85
    signal.recommended_size = Decimal::new(1000, 0);
    signal.max_size = Decimal::new(5000, 0);
    signal.expected_slippage = Decimal::new(10, 4); // 0.1%
    
    signal
}

/// Helper function to create test execution instruction
fn create_test_execution(signal_id: Uuid) -> ExecutionInstruction {
    let symbol = Symbol::new("BTC", "USDT");
    
    let buy_order = Order::new(
        ExchangeId::OKX,
        symbol.clone(),
        Side::Buy,
        OrderType::Market,
        Decimal::new(1, 1), // 0.1 BTC
        Some(Decimal::new(50000, 0)),
    );
    
    let sell_order = Order::new(
        ExchangeId::ByBit,
        symbol,
        Side::Sell,
        OrderType::Market,
        Decimal::new(1, 1), // 0.1 BTC
        Some(Decimal::new(50100, 0)),
    );
    
    let mut instruction = ExecutionInstruction::new(signal_id, buy_order, sell_order);
    instruction.expected_profit = Decimal::new(150, 4); // 1.5%
    instruction
}

#[tokio::test]
async fn test_storage_initialization() {
    let storage = create_test_storage().await;
    
    // Test that storage is properly initialized
    let stats = storage.get_statistics().await.expect("Failed to get statistics");
    assert_eq!(stats.total_signals, 0);
    assert_eq!(stats.total_executions, 0);
}

#[tokio::test]
async fn test_signal_storage_and_retrieval() {
    let storage = create_test_storage().await;
    let signal = create_test_signal();
    let confidence_score = Decimal::new(85, 2);
    
    // Store signal
    storage.store_signal_async(signal.clone(), confidence_score, SignalStatus::Detected)
        .await
        .expect("Failed to store signal");
    
    // Query signals
    let query = SignalQuery::default();
    let stored_signals = storage.query_signals_async(&query)
        .await
        .expect("Failed to query signals");
    
    assert_eq!(stored_signals.len(), 1);
    let stored_signal = &stored_signals[0];
    assert_eq!(stored_signal.signal.id, signal.id);
    assert_eq!(stored_signal.signal.symbol, signal.symbol);
    assert_eq!(stored_signal.confidence_score, confidence_score);
    assert_eq!(stored_signal.status, SignalStatus::Detected);
}

#[tokio::test]
async fn test_execution_storage_and_retrieval() {
    let storage = create_test_storage().await;
    let signal = create_test_signal();
    let execution = create_test_execution(signal.id);
    
    // Store signal first
    storage.store_signal_async(signal.clone(), Decimal::new(85, 2), SignalStatus::Detected)
        .await
        .expect("Failed to store signal");
    
    // Store execution
    storage.store_execution(execution.clone(), ExecutionStatus::Pending)
        .await
        .expect("Failed to store execution");
    
    // Retrieve execution
    let stored_execution = storage.get_execution(signal.id)
        .await
        .expect("Failed to get execution")
        .expect("Execution not found");
    
    assert_eq!(stored_execution.instruction.signal_id, execution.signal_id);
    assert_eq!(stored_execution.status, ExecutionStatus::Pending);
}

#[tokio::test]
async fn test_signal_status_updates() {
    let storage = create_test_storage().await;
    let signal = create_test_signal();
    
    // Store signal with initial status
    storage.store_signal_async(signal.clone(), Decimal::new(85, 2), SignalStatus::Detected)
        .await
        .expect("Failed to store signal");
    
    // Update status
    storage.update_signal_status(signal.id, SignalStatus::Executed)
        .await
        .expect("Failed to update signal status");
    
    // Verify status update
    let query = SignalQuery {
        status_filter: Some(SignalStatus::Executed),
        ..Default::default()
    };
    let signals = storage.query_signals_async(&query)
        .await
        .expect("Failed to query signals");
    
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0].status, SignalStatus::Executed);
}

#[tokio::test]
async fn test_execution_status_updates() {
    let storage = create_test_storage().await;
    let signal = create_test_signal();
    let execution = create_test_execution(signal.id);
    
    // Store signal and execution
    storage.store_signal_async(signal.clone(), Decimal::new(85, 2), SignalStatus::Detected)
        .await
        .expect("Failed to store signal");
    
    storage.store_execution(execution.clone(), ExecutionStatus::Pending)
        .await
        .expect("Failed to store execution");
    
    // Update execution status with results
    let actual_profit = Decimal::new(125, 4); // 1.25%
    let execution_time = 2500u64;
    
    storage.update_execution_status(
        signal.id,
        ExecutionStatus::Completed,
        Some(actual_profit),
        Some(execution_time),
    )
    .await
    .expect("Failed to update execution status");
    
    // Verify update
    let stored_execution = storage.get_execution(signal.id)
        .await
        .expect("Failed to get execution")
        .expect("Execution not found");
    
    assert_eq!(stored_execution.status, ExecutionStatus::Completed);
    assert_eq!(stored_execution.actual_profit, Some(actual_profit));
    assert_eq!(stored_execution.execution_time_ms, Some(execution_time));
}

#[tokio::test]
async fn test_signal_filtering() {
    let storage = create_test_storage().await;
    
    // Create multiple signals with different properties
    let mut signal1 = create_test_signal();
    signal1.symbol = Symbol::new("BTC", "USDT");
    signal1.buy_exchange = ExchangeId::OKX;
    
    let mut signal2 = create_test_signal();
    signal2.symbol = Symbol::new("ETH", "USDT");
    signal2.buy_exchange = ExchangeId::ByBit;
    
    let mut signal3 = create_test_signal();
    signal3.symbol = Symbol::new("BTC", "USDT");
    signal3.buy_exchange = ExchangeId::MEXC;
    
    // Store signals with different statuses and confidence scores
    storage.store_signal_async(signal1.clone(), Decimal::new(90, 2), SignalStatus::Detected)
        .await
        .expect("Failed to store signal1");
    
    storage.store_signal_async(signal2.clone(), Decimal::new(75, 2), SignalStatus::Filtered)
        .await
        .expect("Failed to store signal2");
    
    storage.store_signal_async(signal3.clone(), Decimal::new(95, 2), SignalStatus::Executed)
        .await
        .expect("Failed to store signal3");
    
    // Test status filtering
    let query = SignalQuery {
        status_filter: Some(SignalStatus::Detected),
        ..Default::default()
    };
    let detected_signals = storage.query_signals_async(&query).await.expect("Failed to query");
    assert_eq!(detected_signals.len(), 1);
    assert_eq!(detected_signals[0].signal.id, signal1.id);
    
    // Test symbol filtering
    let query = SignalQuery {
        symbol_filter: Some("BTC".to_string()),
        ..Default::default()
    };
    let btc_signals = storage.query_signals_async(&query).await.expect("Failed to query");
    assert_eq!(btc_signals.len(), 2);
    
    // Test exchange filtering
    let query = SignalQuery {
        exchange_filter: Some(ExchangeId::OKX),
        ..Default::default()
    };
    let okx_signals = storage.query_signals_async(&query).await.expect("Failed to query");
    assert_eq!(okx_signals.len(), 1);
    assert_eq!(okx_signals[0].signal.id, signal1.id);
    
    // Test confidence filtering
    let query = SignalQuery {
        min_confidence: Some(Decimal::new(80, 2)),
        ..Default::default()
    };
    let high_confidence_signals = storage.query_signals_async(&query).await.expect("Failed to query");
    assert_eq!(high_confidence_signals.len(), 2); // signal1 (90%) and signal3 (95%)
    
    // Test limit
    let query = SignalQuery {
        limit: Some(1),
        ..Default::default()
    };
    let limited_signals = storage.query_signals_async(&query).await.expect("Failed to query");
    assert_eq!(limited_signals.len(), 1);
}

#[tokio::test]
async fn test_storage_statistics() {
    let storage = create_test_storage().await;
    
    // Store multiple signals with different statuses
    for i in 0..5 {
        let mut signal = create_test_signal();
        signal.id = Uuid::new_v4();
        
        let status = match i {
            0..=2 => SignalStatus::Detected,
            3 => SignalStatus::Executed,
            _ => SignalStatus::Filtered,
        };
        
        storage.store_signal_async(signal.clone(), Decimal::new(85, 2), status)
            .await
            .expect("Failed to store signal");
        
        if i < 2 {
            let execution = create_test_execution(signal.id);
            storage.store_execution(execution, ExecutionStatus::Pending)
                .await
                .expect("Failed to store execution");
        }
    }
    
    // Get statistics
    let stats = storage.get_statistics().await.expect("Failed to get statistics");
    
    assert_eq!(stats.total_signals, 5);
    assert_eq!(stats.total_executions, 2);
    
    // Check status counts
    assert_eq!(stats.signal_status_counts.get(&SignalStatus::Detected), Some(&3));
    assert_eq!(stats.signal_status_counts.get(&SignalStatus::Executed), Some(&1));
    assert_eq!(stats.signal_status_counts.get(&SignalStatus::Filtered), Some(&1));
    
    assert_eq!(stats.execution_status_counts.get(&ExecutionStatus::Pending), Some(&2));
}

#[tokio::test]
async fn test_storage_cleanup() {
    let config = StorageConfig {
        database_path: ":memory:".to_string(),
        max_signal_history: 3, // Small limit for testing
        max_execution_history: 2,
        enable_compression: false,
    };
    
    let storage = StorageService::new_async(config).await.expect("Failed to create storage");
    
    // Store more signals than the limit
    for _i in 0..5 {
        let mut signal = create_test_signal();
        signal.id = Uuid::new_v4();
        
        storage.store_signal_async(signal.clone(), Decimal::new(85, 2), SignalStatus::Detected)
            .await
            .expect("Failed to store signal");
        
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Check that cleanup occurred
    let stats = storage.get_statistics().await.expect("Failed to get statistics");
    assert_eq!(stats.total_signals, 3); // Should be limited to max_signal_history
}

#[tokio::test]
async fn test_concurrent_storage_operations() {
    let storage = std::sync::Arc::new(create_test_storage().await);
    
    // Spawn multiple concurrent tasks
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            let mut signal = create_test_signal();
            signal.id = Uuid::new_v4();
            signal.symbol = Symbol::new("BTC", &format!("USDT{}", i));
            
            storage_clone.store_signal_async(
                signal,
                Decimal::new(85, 2),
                SignalStatus::Detected,
            ).await
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task panicked").expect("Failed to store signal");
    }
    
    // Verify all signals were stored
    let stats = storage.get_statistics().await.expect("Failed to get statistics");
    assert_eq!(stats.total_signals, 10);
}

#[tokio::test]
async fn test_time_range_filtering() {
    let storage = create_test_storage().await;
    
    let now = chrono::Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    let two_hours_ago = now - chrono::Duration::hours(2);
    
    // Store signals (they will have current timestamp)
    for _i in 0..3 {
        let mut signal = create_test_signal();
        signal.id = Uuid::new_v4();
        
        storage.store_signal_async(signal, Decimal::new(85, 2), SignalStatus::Detected)
            .await
            .expect("Failed to store signal");
    }
    
    // Query with time range (should find all signals since they're recent)
    let query = SignalQuery {
        time_range: Some((one_hour_ago, now + chrono::Duration::minutes(1))),
        ..Default::default()
    };
    
    let signals = storage.query_signals_async(&query).await.expect("Failed to query");
    assert_eq!(signals.len(), 3);
    
    // Query with time range that excludes all signals
    let query = SignalQuery {
        time_range: Some((two_hours_ago, one_hour_ago)),
        ..Default::default()
    };
    
    let signals = storage.query_signals_async(&query).await.expect("Failed to query");
    assert_eq!(signals.len(), 0);
}

#[tokio::test]
async fn test_error_handling() {
    // Test with invalid database path (should fail gracefully)
    let config = StorageConfig {
        database_path: "/invalid/path/database.db".to_string(),
        max_signal_history: 1000,
        max_execution_history: 500,
        enable_compression: false,
    };
    
    let result = StorageService::new_async(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_decimal_precision() {
    let storage = create_test_storage().await;
    
    // Create signal with high precision decimals
    let mut signal = create_test_signal();
    signal.buy_price = Decimal::new(5000012345, 5); // 50000.12345
    signal.sell_price = Decimal::new(5010067890, 5); // 50100.67890
    signal.gross_profit_percent = Decimal::new(123456789, 9); // 0.123456789
    
    let confidence_score = Decimal::new(876543210, 9); // 0.876543210
    
    // Store and retrieve
    storage.store_signal_async(signal.clone(), confidence_score, SignalStatus::Detected)
        .await
        .expect("Failed to store signal");
    
    let query = SignalQuery::default();
    let stored_signals = storage.query_signals_async(&query)
        .await
        .expect("Failed to query signals");
    
    assert_eq!(stored_signals.len(), 1);
    let stored_signal = &stored_signals[0];
    
    // Verify precision is maintained
    assert_eq!(stored_signal.signal.buy_price, signal.buy_price);
    assert_eq!(stored_signal.signal.sell_price, signal.sell_price);
    assert_eq!(stored_signal.signal.gross_profit_percent, signal.gross_profit_percent);
    assert_eq!(stored_signal.confidence_score, confidence_score);
}