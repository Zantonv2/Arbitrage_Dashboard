---
inclusion: fileMatch
fileMatchPattern: "strategies/**"
---

# Strategy Framework Implementation Guide

## Strategy Trait Definition

All arbitrage strategies must implement the unified Strategy trait:

```rust
use crate::{ArbitrageError, Result, Signal, OrderBook, ExecutionInstruction};
use rust_decimal::Decimal;
use std::collections::HashMap;

pub trait Strategy: Send + Sync {
    /// Strategy identifier for logging and configuration
    fn id(&self) -> &'static str;
    
    /// Human-readable strategy name
    fn name(&self) -> &'static str;
    
    /// Detect arbitrage opportunities from market data
    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>>;
    
    /// Filter signals based on strategy-specific criteria
    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool>;
    
    /// Plan execution for validated signals
    fn plan(&self, signal: &Signal, context: &ExecutionContext) -> Result<ExecutionInstruction>;
    
    /// Get strategy configuration parameters
    fn config(&self) -> &StrategyConfig;
    
    /// Update strategy configuration
    fn update_config(&mut self, config: StrategyConfig) -> Result<()>;
}
```

## Core Data Structures

### MarketBundle
```rust
pub struct MarketBundle {
    pub order_books: HashMap<(ExchangeId, Symbol), OrderBook>,
    pub funding_rates: HashMap<(ExchangeId, Symbol), FundingRate>,
    pub tickers: HashMap<(ExchangeId, Symbol), Ticker>,
    pub timestamp: DateTime<Utc>,
}
```

### RawSignal
```rust
pub struct RawSignal {
    pub strategy_id: &'static str,
    pub symbol: Symbol,
    pub legs: Vec<TradeLeg>,
    pub expected_profit_bps: i32,
    pub basis_bps: Option<i32>,
    pub confidence_factors: ConfidenceFactors,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

### TradeLeg
```rust
pub struct TradeLeg {
    pub exchange: ExchangeId,
    pub symbol: Symbol,
    pub side: Side,
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_type: OrderType,
}
```

## Implementation Requirements

### 1. Error Handling
- **NEVER use .unwrap() or .expect()**
- All functions return `Result<T, ArbitrageError>`
- Use `?` operator for error propagation
- Provide meaningful error context with `.map_err()`

### 2. Financial Safety
- **ALL calculations use `rust_decimal::Decimal`**
- Validate input ranges before calculations
- Check for overflow in arithmetic operations
- Use proper rounding for precision handling

### 3. Performance Requirements
- Minimize allocations in hot paths
- Use `&str` instead of `String` where possible
- Prefer iterators over collecting to vectors
- Cache expensive calculations

### 4. Testing Requirements
- Every strategy must have property-based tests
- Test edge cases (empty order books, extreme prices)
- Mock external dependencies
- Validate financial calculations with known examples

## Strategy Implementation Template

```rust
use crate::strategies::{Strategy, MarketBundle, RawSignal, FilterContext, ExecutionContext};
use crate::{ArbitrageError, Result, Signal, ExecutionInstruction, StrategyConfig};

pub struct YourStrategy {
    config: StrategyConfig,
}

impl YourStrategy {
    pub fn new(config: StrategyConfig) -> Self {
        Self { config }
    }
}

impl Strategy for YourStrategy {
    fn id(&self) -> &'static str {
        "your_strategy"
    }
    
    fn name(&self) -> &'static str {
        "Your Strategy Name"
    }
    
    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();
        
        // Implementation logic here
        // Remember: NO .unwrap() - use proper error handling
        
        Ok(signals)
    }
    
    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Validation logic here
        Ok(true)
    }
    
    fn plan(&self, signal: &Signal, context: &ExecutionContext) -> Result<ExecutionInstruction> {
        // Execution planning logic here
        todo!("Implement execution planning")
    }
    
    fn config(&self) -> &StrategyConfig {
        &self.config
    }
    
    fn update_config(&mut self, config: StrategyConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    #[test]
    fn test_strategy_basic_functionality() {
        // Basic unit tests
    }
    
    proptest! {
        #[test]
        fn test_strategy_properties(
            // Property-based test parameters
        ) {
            // Property-based tests here
        }
    }
}
```

## Strategy-Specific Guidelines

### CEX ↔ CEX Price Arbitrage
- Focus on bid/ask spread analysis
- Account for trading fees on both exchanges
- Validate symbol availability on both exchanges
- Consider execution latency and slippage

### Funding Rate Arbitrage
- Monitor funding rate schedules
- Calculate time-weighted returns
- Account for position holding costs
- Implement funding rate prediction

### Spot ↔ Perpetual Arbitrage
- Track basis (futures - spot price)
- Monitor convergence patterns
- Account for margin requirements
- Handle rollover mechanics

## Common Pitfalls to Avoid

1. **Using f64 for financial calculations** - Always use Decimal
2. **Ignoring trading fees** - Include all costs in profit calculations
3. **Not validating order book depth** - Check liquidity before signaling
4. **Hardcoding exchange-specific logic** - Use normalized data structures
5. **Missing error handling** - Every operation can fail, handle gracefully
6. **Not testing edge cases** - Empty books, stale data, network issues

## Integration Points

### With Arbitrage Engine
- Strategies are registered in the engine
- Engine calls detect() on market data updates
- Engine manages signal lifecycle and deduplication

### With Confidence Scorer
- Strategies provide confidence factors
- Scorer combines factors into final confidence score
- Score influences signal priority and sizing

### With Size Calculator
- Strategies specify maximum position sizes
- Calculator determines optimal trade sizes
- Considers order book depth and slippage

### With Execution Preparer
- Strategies generate execution plans
- Preparer validates and optimizes orders
- Handles exchange-specific order formatting