/// Professional arbitrage strategies for institutional-grade trading
/// 
/// Phase 1: Core Engine Completion (CURRENT FOCUS)
/// - Complete Size Calculator, Execution Preparer, Confidence Scorer, Storage
/// - Add comprehensive property tests
/// 
/// Phase 2: Strategy Implementation (AFTER Phase 1)
/// - Implement all 10 strategy modules according to priority
/// - Integrate with completed core engine

// Core strategy framework
pub mod base;

// Strategy implementations will be added in Phase 2
// TODO: Implement after core engine completion:
// pub mod cex_arbitrage;           // Tier 1 - Highest ROI
// pub mod funding_rate_arbitrage;  // Tier 1 - Passive income
// pub mod stablecoin_arbitrage;    // Tier 1 - Predictable
// pub mod spot_perp_arbitrage;     // Tier 2 - Medium complexity
// pub mod cross_exchange_arbitrage;// Tier 2 - Balance management
// pub mod new_listing_arbitrage;   // Tier 2 - Event-driven
// pub mod latency_arbitrage;       // Tier 3 - Ultra-low latency
// pub mod spread_capture;          // Tier 3 - Market making
// pub mod convergence_arbitrage;   // Tier 3 - Statistical modeling
// pub mod hedged_funding;          // Tier 3 - Complex position management

// Strategy registry - TODO: implement after strategies are ready
// pub mod registry;

// Re-export core types
pub use base::*;