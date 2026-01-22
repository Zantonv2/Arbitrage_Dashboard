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

pub mod config;
pub mod execution_context;
pub mod filter_context;
pub mod macros;
pub mod market_types;
pub mod traits;

// Individual strategy implementations
pub mod strategy_impl;

// Re-export core types
pub use base::*;
pub use config::*;
pub use execution_context::*;
pub use filter_context::*;
pub use macros::*;
pub use market_types::*;
pub use registry::StrategyRegistry;
pub use strategies_specifics::*;
pub use traits::*;

// Re-export strategy modules for direct access
pub use strategy_impl::cex_arbitrage;
pub use strategy_impl::convergence_arbitrage;
pub use strategy_impl::cross_exchange_arbitrage;
pub use strategy_impl::funding_rate_arbitrage;
pub use strategy_impl::hedged_funding;
pub use strategy_impl::latency_arbitrage;
pub use strategy_impl::new_listing_arbitrage;
pub use strategy_impl::spot_perp_arbitrage;
pub use strategy_impl::spread_capture;
pub use strategy_impl::stablecoin_arbitrage;

// Re-export all strategies
pub use strategy_impl::{
    CexArbitrageStrategy, ConvergenceArbitrageStrategy, CrossExchangeArbitrageStrategy,
    FundingRateArbitrageStrategy, HedgedFundingStrategy, LatencyArbitrageStrategy,
    NewListingArbitrageStrategy, SpotPerpArbitrageStrategy, SpreadCaptureStrategy,
    StablecoinArbitrageStrategy,
};
