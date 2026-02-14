//! # Instruction Cache
//!
//! In-memory cache for storing prepared execution instructions.
//! Instructions are stored with a TTL to prevent stale executions.

use arbitrage_core::types::ExecutionInstruction;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Cached instruction with metadata
#[derive(Debug, Clone)]
pub struct CachedInstruction {
    /// The execution instruction
    pub instruction: ExecutionInstruction,
    /// When the instruction was created
    pub created_at: DateTime<Utc>,
    /// When the instruction expires
    pub expires_at: DateTime<Utc>,
}

/// Configuration for the instruction cache
#[derive(Debug, Clone)]
pub struct InstructionCacheConfig {
    /// Time-to-live for cached instructions in seconds
    pub ttl_seconds: u64,
    /// Maximum number of instructions to cache
    pub max_entries: usize,
}

impl Default for InstructionCacheConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 300, // 5 minutes
            max_entries: 1000,
        }
    }
}

/// In-memory cache for execution instructions
#[derive(Debug)]
pub struct InstructionCache {
    cache: Arc<RwLock<HashMap<Uuid, CachedInstruction>>>,
    config: InstructionCacheConfig,
}

impl InstructionCache {
    /// Create a new instruction cache
    pub fn new(config: InstructionCacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Store an instruction in the cache
    pub async fn store(&self, instruction: ExecutionInstruction) -> Uuid {
        let id = instruction.signal_id;
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(self.config.ttl_seconds as i64);

        let cached = CachedInstruction {
            instruction,
            created_at: now,
            expires_at,
        };

        let mut cache = self.cache.write().await;

        // Evict expired entries if at capacity
        if cache.len() >= self.config.max_entries {
            self.evict_expired(&mut cache);
        }

        cache.insert(id, cached);
        id
    }

    /// Retrieve an instruction from the cache
    pub async fn get(&self, id: &Uuid) -> Option<CachedInstruction> {
        let cache = self.cache.read().await;
        cache.get(id).and_then(|cached| {
            // Check if expired
            if cached.expires_at < Utc::now() {
                None
            } else {
                Some(cached.clone())
            }
        })
    }

    /// Remove an instruction from the cache
    pub async fn remove(&self, id: &Uuid) -> Option<CachedInstruction> {
        let mut cache = self.cache.write().await;
        cache.remove(id)
    }

    /// Check if an instruction exists and is not expired
    pub async fn contains(&self, id: &Uuid) -> bool {
        let cache = self.cache.read().await;
        cache
            .get(id)
            .map(|cached| cached.expires_at >= Utc::now())
            .unwrap_or(false)
    }

    /// Get the number of cached instructions (including expired)
    pub async fn len(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Clear all cached instructions
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Evict expired entries from the cache
    fn evict_expired(&self, cache: &mut HashMap<Uuid, CachedInstruction>) {
        let now = Utc::now();
        cache.retain(|_, cached| cached.expires_at >= now);
    }

    /// Clean up expired entries (call periodically)
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        self.evict_expired(&mut cache);
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let now = Utc::now();
        let total = cache.len();
        let expired = cache.values().filter(|c| c.expires_at < now).count();
        let active = total - expired;

        CacheStats {
            total_entries: total,
            active_entries: active,
            expired_entries: expired,
            max_entries: self.config.max_entries,
        }
    }
}

impl Default for InstructionCache {
    fn default() -> Self {
        Self::new(InstructionCacheConfig::default())
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub active_entries: usize,
    pub expired_entries: usize,
    pub max_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arbitrage_core::types::{ExchangeId, Order, OrderType, Side, Symbol};
    use rust_decimal::Decimal;

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
        assert_eq!(retrieved.unwrap().instruction.signal_id, id);
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
}
