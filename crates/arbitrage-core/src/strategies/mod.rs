/// Professional arbitrage strategies for institutional-grade trading
///
/// Phase 1: Core Engine Completion (COMPLETE)
/// - Size Calculator, Execution Preparer, Confidence Scorer, Storage ✅
/// - Comprehensive property tests ✅
///
/// Phase 2: Strategy System Implementation (IN PROGRESS)
/// - Strategy trait definition ✅
/// - Strategy registry system ✅
/// - Strategy-specific configurations ✅
/// - CEX ↔ CEX Price Arbitrage ✅ (Tier 1 - Highest ROI)
/// - Implement remaining 9 strategy modules (NEXT)
// Core strategy framework
pub mod base;
pub mod registry;
pub mod strategies_specifics;

// Individual strategy implementations
pub mod strategy_impl;

// Re-export core types
pub use base::*;
pub use registry::StrategyRegistry;
pub use strategies_specifics::*;

// Re-export all strategies
pub use strategy_impl::{
    CexArbitrageStrategy, ConvergenceArbitrageStrategy, CrossExchangeArbitrageStrategy,
    FundingRateArbitrageStrategy, HedgedFundingStrategy, LatencyArbitrageStrategy,
    NewListingArbitrageStrategy, SpotPerpArbitrageStrategy, SpreadCaptureStrategy,
    StablecoinArbitrageStrategy,
};
