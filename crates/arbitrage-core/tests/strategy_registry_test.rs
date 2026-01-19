use arbitrage_core::strategies::{
    FilterContext, MarketBundle, RawSignal, Strategy, StrategyConfig, StrategyRegistry,
};
use arbitrage_core::{ArbitrageError, Result};
use std::sync::Arc;

/// Mock strategy for testing
struct MockStrategy {
    id: &'static str,
    name: &'static str,
    config: StrategyConfig,
}

impl MockStrategy {
    fn new(id: &'static str, name: &'static str) -> Self {
        Self {
            id,
            name,
            config: StrategyConfig::default(),
        }
    }
}

impl Strategy for MockStrategy {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn detect(&self, _market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        Ok(Vec::new())
    }

    fn filter(&self, _signal: &RawSignal, _context: &FilterContext) -> Result<bool> {
        Ok(true)
    }

    fn config(&self) -> &StrategyConfig {
        &self.config
    }

    fn update_config(&mut self, config: StrategyConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }
}

#[test]
fn test_registry_register_strategy() {
    let mut registry = StrategyRegistry::new();
    let strategy = Arc::new(MockStrategy::new("test_1", "Test Strategy 1"));

    assert!(registry.register(strategy).is_ok());
    assert_eq!(registry.count(), 1);
    assert!(registry.contains("test_1"));
}

#[test]
fn test_registry_duplicate_registration() {
    let mut registry = StrategyRegistry::new();
    let strategy = Arc::new(MockStrategy::new("test_1", "Test Strategy 1"));

    assert!(registry.register(strategy.clone()).is_ok());

    // Should fail on duplicate registration
    let result = registry.register(strategy);
    assert!(result.is_err());

    if let Err(ArbitrageError::Configuration(msg)) = result {
        assert!(msg.contains("already registered"));
    } else {
        panic!("Expected Configuration error");
    }
}

#[test]
fn test_registry_get_strategy() {
    let mut registry = StrategyRegistry::new();
    let strategy = Arc::new(MockStrategy::new("test_1", "Test Strategy 1"));

    let _ = registry.register(strategy);

    let retrieved = registry.get("test_1");
    assert!(retrieved.is_some());

    let nonexistent = registry.get("nonexistent");
    assert!(nonexistent.is_none());
}

#[test]
fn test_registry_get_all() {
    let mut registry = StrategyRegistry::new();
    let s1 = Arc::new(MockStrategy::new("test_1", "Test 1"));
    let s2 = Arc::new(MockStrategy::new("test_2", "Test 2"));

    let _ = registry.register(s1);
    let _ = registry.register(s2);

    assert_eq!(registry.get_all().len(), 2);
}

#[test]
fn test_registry_get_enabled() {
    let mut registry = StrategyRegistry::new();

    // Create enabled strategy
    let mut enabled_strategy = MockStrategy::new("enabled", "Enabled Strategy");
    enabled_strategy.config.enabled = true;

    // Create disabled strategy
    let mut disabled_strategy = MockStrategy::new("disabled", "Disabled Strategy");
    disabled_strategy.config.enabled = false;

    let _ = registry.register(Arc::new(enabled_strategy));
    let _ = registry.register(Arc::new(disabled_strategy));

    let enabled = registry.get_enabled();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id(), "enabled");
}

#[test]
fn test_registry_list_ids() {
    let mut registry = StrategyRegistry::new();
    let s1 = Arc::new(MockStrategy::new("test_1", "Test 1"));
    let s2 = Arc::new(MockStrategy::new("test_2", "Test 2"));

    let _ = registry.register(s1);
    let _ = registry.register(s2);

    let ids = registry.list_ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"test_1".to_string()));
    assert!(ids.contains(&"test_2".to_string()));
}

#[test]
fn test_registry_list_names() {
    let mut registry = StrategyRegistry::new();
    let strategy = Arc::new(MockStrategy::new("test_1", "Test Strategy 1"));

    let _ = registry.register(strategy);

    let names = registry.list_names();
    assert_eq!(names.len(), 1);
    assert!(names[0].contains("Test Strategy 1"));
    assert!(names[0].contains("test_1"));
}

#[test]
fn test_registry_unregister() {
    let mut registry = StrategyRegistry::new();
    let strategy = Arc::new(MockStrategy::new("test_1", "Test Strategy 1"));

    let _ = registry.register(strategy);
    assert_eq!(registry.count(), 1);

    assert!(registry.unregister("test_1").is_ok());
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_registry_unregister_nonexistent() {
    let mut registry = StrategyRegistry::new();
    let result = registry.unregister("nonexistent");

    assert!(result.is_err());
    if let Err(ArbitrageError::Configuration(msg)) = result {
        assert!(msg.contains("not found"));
    } else {
        panic!("Expected Configuration error");
    }
}

#[test]
fn test_registry_clear() {
    let mut registry = StrategyRegistry::new();
    let s1 = Arc::new(MockStrategy::new("test_1", "Test 1"));
    let s2 = Arc::new(MockStrategy::new("test_2", "Test 2"));

    let _ = registry.register(s1);
    let _ = registry.register(s2);
    assert_eq!(registry.count(), 2);

    registry.clear();
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_registry_count_enabled() {
    let mut registry = StrategyRegistry::new();

    // Create enabled strategy
    let mut enabled_strategy = MockStrategy::new("enabled", "Enabled Strategy");
    enabled_strategy.config.enabled = true;

    // Create disabled strategy
    let mut disabled_strategy = MockStrategy::new("disabled", "Disabled Strategy");
    disabled_strategy.config.enabled = false;

    let _ = registry.register(Arc::new(enabled_strategy));
    let _ = registry.register(Arc::new(disabled_strategy));

    assert_eq!(registry.count(), 2);
    assert_eq!(registry.count_enabled(), 1);
}

#[test]
fn test_registry_default() {
    let registry = StrategyRegistry::default();
    assert_eq!(registry.count(), 0);
}
