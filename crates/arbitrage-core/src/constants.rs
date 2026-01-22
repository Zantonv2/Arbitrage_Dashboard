/// Arbitrage trading constants
///
/// Contains numerical constants used throughout the arbitrage system
/// to avoid magic numbers in the codebase.
pub const BASIS_POINTS_DIVISOR: i32 = 10000;

/// Broadcast channel capacity for tokio sync channels
///
/// Used for event broadcasting between components.
pub const BROADCAST_CHANNEL_CAPACITY: usize = 1000;

/// Default minimum notional value in USD for trade execution
pub const DEFAULT_MIN_NOTIONAL_USD: i32 = 10;

/// Default position allocation as fraction of max exposure for convergence strategies
///
/// Value of 4 means 25% (1/4) of max exposure per position
pub const DEFAULT_CONVERGENCE_POSITION_DIVISOR: i32 = 4;

/// Profit multiplier for convergence arbitrage Z-score to basis points
///
/// Converts Z-score deviation to estimated profit in basis points.
/// Higher Z-scores indicate more extreme price divergence.
pub const DEFAULT_CONVERGENCE_PROFIT_MULTIPLIER: i32 = 50;

/// Maximum profit cap for convergence arbitrage in basis points
///
/// Prevents overestimation of profits from extreme deviations.
pub const DEFAULT_CONVERGENCE_PROFIT_CAP_BPS: i32 = 500;

/// Multiplier for ratio deviation to basis points in mean reversion
///
/// Converts price ratio deviation to estimated profit in basis points.
pub const DEFAULT_RATIO_DEVIATION_MULTIPLIER: i32 = 1000;

/// Slippage tier 1: Conservative slippage estimate in basis points
///
/// Used for hedged funding strategy with lower cost assumptions.
pub const SLIPPIER_TIER_1_BPS: i32 = 10;

/// Slippage tier 2: Moderate slippage estimate in basis points
///
/// Used for hedged funding strategy with moderate cost assumptions.
pub const SLIPPIER_TIER_2_BPS: i32 = 15;
