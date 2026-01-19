/// Individual strategy implementations
///
/// Each strategy is a self-contained module that implements the Strategy trait
/// and focuses on a specific type of arbitrage opportunity.
// Tier 1 Strategies (Highest ROI)
pub mod cex_arbitrage;
pub mod funding_rate_arbitrage;
pub mod stablecoin_arbitrage;

// Tier 2 Strategies (Medium Complexity)
pub mod cross_exchange_arbitrage;
pub mod new_listing_arbitrage;
pub mod spot_perp_arbitrage;

// Tier 3 Strategies (Advanced Features)
pub mod convergence_arbitrage;
pub mod hedged_funding;
pub mod latency_arbitrage;
pub mod spread_capture;

// Re-exports
pub use cex_arbitrage::CexArbitrageStrategy;
pub use convergence_arbitrage::ConvergenceArbitrageStrategy;
pub use cross_exchange_arbitrage::CrossExchangeArbitrageStrategy;
pub use funding_rate_arbitrage::FundingRateArbitrageStrategy;
pub use hedged_funding::HedgedFundingStrategy;
pub use latency_arbitrage::LatencyArbitrageStrategy;
pub use new_listing_arbitrage::NewListingArbitrageStrategy;
pub use spot_perp_arbitrage::SpotPerpArbitrageStrategy;
pub use spread_capture::SpreadCaptureStrategy;
pub use stablecoin_arbitrage::StablecoinArbitrageStrategy;
