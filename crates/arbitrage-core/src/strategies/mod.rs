/// Professional arbitrage strategies for institutional-grade trading
/// 
/// This module implements 10 proven arbitrage strategies that work in real markets:
/// 1. CEX ↔ CEX Price Arbitrage
/// 2. Spot ↔ Perpetual (Futures) Arbitrage  
/// 3. Funding Rate Arbitrage (Roll Yield)
/// 4. Funding + Spot Hedged Strategy
/// 5. Pre-Positioned Capital Cross-Exchange Arbitrage
/// 6. Limit Order Spread Capture
/// 7. Temporal (Latency) Arbitrage
/// 8. Stablecoin Peg Arbitrage
/// 9. Convergence Arbitrage
/// 10. New Listing Cross-Exchange Arbitrage

pub mod cex_arbitrage;
pub mod spot_futures_arbitrage;
pub mod funding_rate_arbitrage;
pub mod hedged_funding;
pub mod cross_exchange;
pub mod spread_capture;
pub mod latency_arbitrage;
pub mod stablecoin_peg;
pub mod convergence;
pub mod new_listing;

// Re-export all strategy types
pub use cex_arbitrage::*;
pub use spot_futures_arbitrage::*;
pub use funding_rate_arbitrage::*;
pub use hedged_funding::*;
pub use cross_exchange::*;
pub use spread_capture::*;
pub use latency_arbitrage::*;
pub use stablecoin_peg::*;
pub use convergence::*;
pub use new_listing::*;