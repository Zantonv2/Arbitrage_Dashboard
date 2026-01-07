use arbitrage_core::{
    storage::{StorageService, StorageConfig, SignalStatus, ExecutionStatus, SignalQuery},
    types::{ExchangeId, Signal, Symbol, ExecutionInstruction, Order, OrderType, Side},
};
use proptest::prelude::*;
use rust_decimal::Decimal;
use uuid::Uuid;

// Helper function to create test signal
fn create_test_signal(
    symbol: Symbol,
    buy_exchange: ExchangeId,
    sell_exchange: ExchangeId,
    buy_price: Decimal,
    sell_price: Decimal,
) -> Signal {
    Signal::new(symbol, buy_exchange, sell_exchange, buy_price, sell_price)
}

// Helper function to create test execution instruction
fn create_test_execution(signal_id: Uuid, symbol: Symbol, quantity: Decimal) -> ExecutionInstruction {
    let buy_order = Order::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Side::Buy,
        OrderType::Limit,
        quantity,
        Some(Decimal::from(50000)),
    );
    
    let sell_order = Order::new(
        ExchangeId::OKX,
        symbol,
        Side::Sell,
        OrderType::Limit,
        quantity,
        Some(Decimal::from(50100)),
    );
    
    ExecutionInstruction::new(signal_id, buy_order, sell_order)
}

// Property 26: Storage Consistency
// Stored signals should be retrievable with consistent data
proptest! {
    #[test]
    fn prop_storage_consistency(
        price_base in 1000u32..100000,
        price_spread in 1u32..1000,
        confidence in 0u32..100
    ) {
        let storage = StorageService::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let buy_price = Decimal::from(price_base);
        let sell_price = buy_price + Decimal::from(price_spread);
        let confidence_score = Decimal::from(confidence);
        
        let signal = create_test_signal(
            symbol.clone(),
            ExchangeId::ByBit,
            ExchangeId::OKX,
            buy_price,
            sell_price
        );
        
        let signal_id = signal.id;
        
        // Store the signal
        let store_result = storage.store_signal(signal.clone(), confidence_score, SignalStatus::Detected);
        prop_assert!(store_result.is_ok());
        
        // Query it back
        let query = SignalQuery::default();
        let query_result = storage.query_signals(&query);
        prop_assert!(query_result.is_ok());
        
        if let Ok(stored_signals) = query_result {
            // Property: stored signal should be retrievable
            let found_signal = stored_signals.iter().find(|s| s.signal.id == signal_id);
            prop_assert!(found_signal.is_some());
            
            if let Some(stored) = found_signal {
                // Property: stored data should match original
                prop_assert_eq!(stored.signal.symbol.clone(), symbol);
                prop_assert_eq!(stored.signal.buy_price, buy_price);
                prop_assert_eq!(stored.signal.sell_price, sell_price);
                prop_assert_eq!(stored.confidence_score, confidence_score);
                prop_assert_eq!(stored.status.clone(), SignalStatus::Detected);
            }
        }
    }
}

// Property 27: Query Filtering Accuracy
// Query filters should return only matching signals
proptest! {
    #[test]
    fn prop_query_filtering_accuracy(
        num_signals in 1usize..20,
        target_exchange_idx in 0usize..3,
        min_confidence in 0u32..50
    ) {
        let storage = StorageService::default();
        let exchanges = [ExchangeId::ByBit, ExchangeId::OKX, ExchangeId::MEXC];
        let target_exchange = exchanges[target_exchange_idx % exchanges.len()];
        let min_confidence_decimal = Decimal::from(min_confidence);
        
        // Store multiple signals with different properties
        for i in 0..num_signals {
            let symbol = if i % 2 == 0 { 
                Symbol::new("BTC", "USDT") 
            } else { 
                Symbol::new("ETH", "USDT") 
            };
            
            let exchange = exchanges[i % exchanges.len()];
            let confidence = Decimal::from((i * 10) % 100); // Varying confidence scores
            
            let signal = create_test_signal(
                symbol,
                exchange,
                ExchangeId::OKX,
                Decimal::from(50000 + i * 100),
                Decimal::from(50100 + i * 100)
            );
            
            let _ = storage.store_signal(signal, confidence, SignalStatus::Detected);
        }
        
        // Query with exchange filter
        let mut query = SignalQuery::default();
        query.exchange_filter = Some(target_exchange);
        query.min_confidence = Some(min_confidence_decimal);
        
        let result = storage.query_signals(&query);
        prop_assert!(result.is_ok());
        
        if let Ok(filtered_signals) = result {
            // Property: all returned signals should match the filter criteria
            for stored_signal in &filtered_signals {
                let matches_exchange = stored_signal.signal.buy_exchange == target_exchange || 
                                     stored_signal.signal.sell_exchange == target_exchange;
                prop_assert!(matches_exchange);
                prop_assert!(stored_signal.confidence_score >= min_confidence_decimal);
            }
        }
    }
}

// Property 28: Status Update Consistency
// Status updates should be reflected in queries
proptest! {
    #[test]
    fn prop_status_update_consistency(
        initial_status_idx in 0usize..5,
        updated_status_idx in 0usize..5
    ) {
        let statuses = [
            SignalStatus::Detected,
            SignalStatus::Filtered,
            SignalStatus::Executed,
            SignalStatus::Expired,
            SignalStatus::Failed,
        ];
        
        let initial_status = statuses[initial_status_idx % statuses.len()].clone();
        let updated_status = statuses[updated_status_idx % statuses.len()].clone();
        
        let storage = StorageService::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let signal = create_test_signal(
            symbol,
            ExchangeId::ByBit,
            ExchangeId::OKX,
            Decimal::from(50000),
            Decimal::from(50100)
        );
        
        let signal_id = signal.id;
        
        // Store with initial status
        let store_result = storage.store_signal(signal, Decimal::from(80), initial_status.clone());
        prop_assert!(store_result.is_ok());
        
        // Update status
        let update_result = storage.update_signal_status(signal_id, updated_status.clone());
        prop_assert!(update_result.is_ok());
        
        // Query and verify update
        let mut query = SignalQuery::default();
        query.status_filter = Some(updated_status.clone());
        
        let query_result = storage.query_signals(&query);
        prop_assert!(query_result.is_ok());
        
        if let Ok(signals) = query_result {
            let found = signals.iter().find(|s| s.signal.id == signal_id);
            
            if updated_status != initial_status {
                // Property: signal should be found with updated status
                prop_assert!(found.is_some());
                if let Some(stored) = found {
                    prop_assert_eq!(stored.status.clone(), updated_status);
                }
            }
        }
    }
}

// Property 29: Execution Storage Consistency
// Stored executions should be retrievable and consistent
proptest! {
    #[test]
    fn prop_execution_storage_consistency(
        quantity in 1u32..1000,
        profit in -1000i32..1000 // Can be negative
    ) {
        let storage = StorageService::default();
        let symbol = Symbol::new("ETH", "USDT");
        let signal_id = Uuid::new_v4();
        
        let quantity_decimal = Decimal::from(quantity) / Decimal::from(1000);
        let profit_decimal = if profit >= 0 { 
            Some(Decimal::from(profit)) 
        } else { 
            Some(Decimal::from(-profit) * Decimal::new(-1, 0)) 
        };
        
        let execution = create_test_execution(signal_id, symbol, quantity_decimal);
        
        // Store execution
        let store_result = storage.store_execution(execution.clone(), ExecutionStatus::Pending);
        prop_assert!(store_result.is_ok());
        
        // Update with results
        let update_result = storage.update_execution_status(
            signal_id,
            ExecutionStatus::Completed,
            profit_decimal,
            Some(150) // 150ms execution time
        );
        prop_assert!(update_result.is_ok());
        
        // Retrieve execution
        let get_result = storage.get_execution(signal_id);
        prop_assert!(get_result.is_ok());
        
        if let Ok(Some(stored_execution)) = get_result {
            // Property: stored execution should match original
            prop_assert_eq!(stored_execution.instruction.signal_id, signal_id);
            prop_assert_eq!(stored_execution.instruction.buy_order.quantity, quantity_decimal);
            prop_assert_eq!(stored_execution.status, ExecutionStatus::Completed);
            prop_assert_eq!(stored_execution.actual_profit, profit_decimal);
            prop_assert_eq!(stored_execution.execution_time_ms, Some(150));
        }
    }
}

// Property 30: Storage Limits Enforcement
// Storage should respect configured limits
proptest! {
    #[test]
    fn prop_storage_limits_enforcement(
        num_signals in 10usize..50,
        max_history in 5usize..25
    ) {
        // Ensure we exceed the limit
        prop_assume!(num_signals > max_history);
        
        let mut config = StorageConfig::default();
        config.max_signal_history = max_history;
        
        let storage = StorageService::new(config).unwrap();
        let symbol = Symbol::new("BTC", "USDT");
        
        // Store more signals than the limit
        for i in 0..num_signals {
            let signal = create_test_signal(
                symbol.clone(),
                ExchangeId::ByBit,
                ExchangeId::OKX,
                Decimal::from(50000 + i * 10),
                Decimal::from(50100 + i * 10)
            );
            
            let _ = storage.store_signal(signal, Decimal::from(80), SignalStatus::Detected);
        }
        
        // Query all signals
        let query = SignalQuery { limit: None, ..Default::default() };
        let result = storage.query_signals(&query);
        prop_assert!(result.is_ok());
        
        if let Ok(signals) = result {
            // Property: should not exceed max history limit
            prop_assert!(signals.len() <= max_history);
            
            // Property: if we have max_history signals, they should be the most recent
            if signals.len() == max_history {
                // Verify signals are sorted by timestamp (newest first)
                for window in signals.windows(2) {
                    prop_assert!(window[0].timestamp >= window[1].timestamp);
                }
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_storage_service_creation() {
        let storage = StorageService::default();
        let stats = storage.get_statistics().unwrap();
        
        assert_eq!(stats.total_signals, 0);
        assert_eq!(stats.total_executions, 0);
    }
    
    #[test]
    fn test_basic_signal_storage() {
        let storage = StorageService::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        let signal = create_test_signal(
            symbol,
            ExchangeId::ByBit,
            ExchangeId::OKX,
            Decimal::from(50000),
            Decimal::from(50100)
        );
        
        let result = storage.store_signal(signal.clone(), Decimal::from(85), SignalStatus::Detected);
        assert!(result.is_ok());
        
        let stats = storage.get_statistics().unwrap();
        assert_eq!(stats.total_signals, 1);
    }
    
    #[test]
    fn test_signal_query_with_limit() {
        let storage = StorageService::default();
        let symbol = Symbol::new("ETH", "USDT");
        
        // Store 5 signals
        for i in 0..5 {
            let signal = create_test_signal(
                symbol.clone(),
                ExchangeId::ByBit,
                ExchangeId::OKX,
                Decimal::from(3000 + i * 10),
                Decimal::from(3100 + i * 10)
            );
            
            let _ = storage.store_signal(signal, Decimal::from(80), SignalStatus::Detected);
        }
        
        // Query with limit of 3
        let query = SignalQuery { limit: Some(3), ..Default::default() };
        let result = storage.query_signals(&query).unwrap();
        
        assert_eq!(result.len(), 3);
    }
    
    #[test]
    fn test_execution_storage_and_retrieval() {
        let storage = StorageService::default();
        let symbol = Symbol::new("BTC", "USDT");
        let signal_id = Uuid::new_v4();
        
        let execution = create_test_execution(signal_id, symbol, Decimal::from(1));
        
        let store_result = storage.store_execution(execution, ExecutionStatus::Pending);
        assert!(store_result.is_ok());
        
        let get_result = storage.get_execution(signal_id);
        assert!(get_result.is_ok());
        assert!(get_result.unwrap().is_some());
    }
    
    #[test]
    fn test_storage_statistics() {
        let storage = StorageService::default();
        let symbol = Symbol::new("BTC", "USDT");
        
        // Store signals with different statuses
        for (i, status) in [SignalStatus::Detected, SignalStatus::Executed, SignalStatus::Failed].iter().enumerate() {
            let signal = create_test_signal(
                symbol.clone(),
                ExchangeId::ByBit,
                ExchangeId::OKX,
                Decimal::from(50000 + i * 100),
                Decimal::from(50100 + i * 100)
            );
            
            let _ = storage.store_signal(signal, Decimal::from(80), status.clone());
        }
        
        let stats = storage.get_statistics().unwrap();
        assert_eq!(stats.total_signals, 3);
        assert_eq!(stats.signal_status_counts.len(), 3);
    }
}