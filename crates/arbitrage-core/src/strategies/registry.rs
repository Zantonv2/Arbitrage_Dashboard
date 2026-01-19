use super::Strategy;
use crate::{ArbitrageError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Registry for managing all available strategies
pub struct StrategyRegistry {
    strategies: HashMap<String, Arc<dyn Strategy>>,
}

impl StrategyRegistry {
    /// Create a new empty strategy registry
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
        }
    }

    /// Register a strategy
    pub fn register(&mut self, strategy: Arc<dyn Strategy>) -> Result<()> {
        let id = strategy.id().to_string();

        if self.strategies.contains_key(&id) {
            return Err(ArbitrageError::Configuration(format!(
                "Strategy '{}' is already registered",
                id
            )));
        }

        debug!("Registering strategy: {} ({})", strategy.name(), id);
        self.strategies.insert(id, strategy);
        Ok(())
    }

    /// Get a strategy by ID
    pub fn get(&self, id: &str) -> Option<Arc<dyn Strategy>> {
        self.strategies.get(id).cloned()
    }

    /// Get all registered strategies
    pub fn get_all(&self) -> Vec<Arc<dyn Strategy>> {
        self.strategies.values().cloned().collect()
    }

    /// Get all enabled strategies
    pub fn get_enabled(&self) -> Vec<Arc<dyn Strategy>> {
        self.strategies
            .values()
            .filter(|s| s.config().enabled)
            .cloned()
            .collect()
    }

    /// List all strategy IDs
    pub fn list_ids(&self) -> Vec<String> {
        self.strategies.keys().cloned().collect()
    }

    /// List all strategy names
    pub fn list_names(&self) -> Vec<String> {
        self.strategies
            .values()
            .map(|s| format!("{} ({})", s.name(), s.id()))
            .collect()
    }

    /// Get count of registered strategies
    pub fn count(&self) -> usize {
        self.strategies.len()
    }

    /// Get count of enabled strategies
    pub fn count_enabled(&self) -> usize {
        self.strategies
            .values()
            .filter(|s| s.config().enabled)
            .count()
    }

    /// Unregister a strategy
    pub fn unregister(&mut self, id: &str) -> Result<()> {
        if self.strategies.remove(id).is_some() {
            debug!("Unregistered strategy: {}", id);
            Ok(())
        } else {
            Err(ArbitrageError::Configuration(format!(
                "Strategy '{}' not found",
                id
            )))
        }
    }

    /// Check if a strategy is registered
    pub fn contains(&self, id: &str) -> bool {
        self.strategies.contains_key(id)
    }

    /// Clear all strategies
    pub fn clear(&mut self) {
        self.strategies.clear();
        debug!("Cleared all strategies from registry");
    }
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        Self::new()
    }
}
