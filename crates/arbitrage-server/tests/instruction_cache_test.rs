//! Tests for instruction cache functionality

use arbitrage_server::instruction_cache::{InstructionCache, InstructionCacheConfig};
use arbitrage_core::types::{ExchangeId, ExecutionInstruction, Order, OrderType, Side, Symbol};
use rust_decimal::Decimal;
use uuid::Uuid;

fn create_test_instruction() -> ExecutionInstruction {
    let buy_order = Order::new(
        ExchangeId::OKX,
        Symbol::new("BTC", "USDT"),
        Side::Buy,
        OrderType::Market,
        Decimal::new(1, 3),
        Some(Decimal::new(50000, 0)),
    );
    let sell_order = Order::new(
        ExchangeId::ByBit,
        Symbol::new("BTC", "USDT"),
        Side::Sell,
        OrderType::Market,
        Decimal::new(1, 3),
        Some(Decimal::new(50100, 0)),
    );
    ExecutionInstruction::new(Uuid::new_v4(), buy_order, sell_order)
}

#[tokio::test]
async fn test_store_and_retrieve() {
    let cache = InstructionCache::new(InstructionCacheConfig {
        ttl_seconds: 60,
        max_entries: 100,
    });

    let instruction = create_test_instruction();
    let id = cache.store(instruction.clone()).await;

    let retrieved = cache.get(&id).await;
    assert!(retrieved.is_some());
    let cached = retrieved.unwrap();
    assert_eq!(cached.instruction.signal_id, instruction.signal_id);
}

#[tokio::test]
async fn test_remove() {
    let cache = InstructionCache::default();
    let instruction = create_test_instruction();
    let id = cache.store(instruction).await;

    assert!(cache.contains(&id).await);
    cache.remove(&id).await;
    assert!(!cache.contains(&id).await);
}

#[tokio::test]
async fn test_expiry() {
    let cache = InstructionCache::new(InstructionCacheConfig {
        ttl_seconds: 0, // Immediate expiry
        max_entries: 100,
    });

    let instruction = create_test_instruction();
    let id = cache.store(instruction).await;

    // Give time for expiry
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let retrieved = cache.get(&id).await;
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_stats() {
    let cache = InstructionCache::default();

    let instruction1 = create_test_instruction();
    let instruction2 = create_test_instruction();
    cache.store(instruction1).await;
    cache.store(instruction2).await;

    let stats = cache.stats().await;
    assert_eq!(stats.total_entries, 2);
    assert_eq!(stats.active_entries, 2);
}

#[tokio::test]
async fn test_max_entries_eviction() {
    let cache = InstructionCache::new(InstructionCacheConfig {
        ttl_seconds: 60,
        max_entries: 3,
    });

    // Store 3 instructions
    let id1 = cache.store(create_test_instruction()).await;
    let id2 = cache.store(create_test_instruction()).await;
    let id3 = cache.store(create_test_instruction()).await;

    // All should be present
    assert!(cache.contains(&id1).await);
    assert!(cache.contains(&id2).await);
    assert!(cache.contains(&id3).await);

    // Store one more - should trigger eviction of expired entries
    let id4 = cache.store(create_test_instruction()).await;
    
    // New one should be present
    assert!(cache.contains(&id4).await);
}

#[tokio::test]
async fn test_clear() {
    let cache = InstructionCache::default();

    cache.store(create_test_instruction()).await;
    cache.store(create_test_instruction()).await;

    assert_eq!(cache.len().await, 2);
    
    cache.clear().await;
    
    assert_eq!(cache.len().await, 0);
}
