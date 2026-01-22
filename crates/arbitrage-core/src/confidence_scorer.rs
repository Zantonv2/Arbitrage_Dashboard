//! Confidence scoring and fee-adjusted profit calculations.
//!
//! This module provides the core scoring engine for evaluating arbitrage signals.
//! It handles:
//! - Fee-adjusted profit calculations to prevent unprofitable signals
//! - Data freshness validation to prevent phantom signals
//! - Multi-factor confidence scoring with configurable weights
//!
//! # Confidence Factors
//!
//! The confidence score is composed of multiple factors:
//! - **Depth Score**: Liquidity available for the trade (30% weight)
//! - **Volatility Score**: Price stability during execution (20% weight)
//! - **Reliability Score**: Exchange uptime and data quality (20% weight)
//! - **Spread Stability Score**: Consistency of the arbitrage spread (15% weight)
//! - **Freshness Score**: Age of market data (15% weight)
//!
//! # Example
//!
//! ```rust
//! use arbitrage_core::confidence_scorer::{ConfidenceScorer, ConfidenceConfig, NetSpreadResult};
//!
//! let config = ConfidenceConfig::default();
//! let scorer = ConfidenceScorer::new(config);
//!
//! // Calculate net spread after fees
//! let result = scorer.calculate_net_spread_bps(
//!     rust_decimal::Decimal::from(50000),
//!     rust_decimal::Decimal::from(50200),
//!     arbitrage_core::types::ExchangeId::Binance,
//!     arbitrage_core::types::ExchangeId::ByBit,
//! );
//!
//! assert!(result.is_profitable());
//! ```

use crate::types::{ExchangeId, OrderBook, VwapResult};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Result type for net spread calculation after fees.
///
/// Represents whether a spread is profitable after accounting for exchange fees.
#[derive(Debug, Clone)]
pub enum NetSpreadResult {
    /// Spread is profitable with the value in basis points
    Profit(i32),
    /// Spread is not profitable after fees
    Unprofitable,
}

impl NetSpreadResult {
    /// Checks if the result is unprofitable.
    ///
    /// # Returns
    ///
    /// `true` if the spread is unprofitable, `false` otherwise.
    pub fn is_unprofitable(&self) -> bool {
        matches!(self, NetSpreadResult::Unprofitable)
    }

    /// Checks if the result is profitable.
    ///
    /// # Returns
    ///
    /// `true` if the spread is profitable, `false` otherwise.
    pub fn is_profitable(&self) -> bool {
        matches!(self, NetSpreadResult::Profit(_))
    }

    /// Extracts the profit value if profitable.
    ///
    /// # Returns
    ///
    /// `Some(i32)` containing the profit in basis points if profitable,
    /// `None` if unprofitable.
    pub fn profit_value(self) -> Option<i32> {
        match self {
            NetSpreadResult::Profit(value) => Some(value),
            NetSpreadResult::Unprofitable => None,
        }
    }
}

/// Individual factors contributing to the confidence score.
///
/// Each factor is a value between 0 and 100, representing the quality
/// of that aspect of the signal.
#[derive(Debug, Clone)]
pub struct ConfidenceFactors {
    /// Score based on available liquidity (0-100)
    pub depth_score: Decimal,
    /// Score based on price volatility (0-100)
    pub volatility_score: Decimal,
    /// Score based on exchange reliability (0-100)
    pub reliability_score: Decimal,
    /// Score based on spread stability (0-100)
    pub spread_stability_score: Decimal,
    /// Score based on data freshness (0-100)
    pub freshness_score: Decimal,
}

/// Configuration for confidence scoring weights.
///
/// Allows customization of how different factors contribute to the overall
/// confidence score. All weights should be positive and typically sum to 1.0.
#[derive(Debug, Clone)]
pub struct ConfidenceConfig {
    /// Weight for depth score (default: 0.3)
    pub depth_weight: Decimal,
    /// Weight for volatility score (default: 0.2)
    pub volatility_weight: Decimal,
    /// Weight for reliability score (default: 0.2)
    pub reliability_weight: Decimal,
    /// Weight for spread stability score (default: 0.15)
    pub spread_stability_weight: Decimal,
    /// Weight for freshness score (default: 0.15)
    pub freshness_weight: Decimal,
    /// Minimum confidence threshold for signal acceptance (default: 0.3)
    pub min_confidence_threshold: Decimal,
    /// Maximum acceptable data age in milliseconds (default: 500ms)
    pub max_latency_ms: u64,
    /// Target VWAP amount in USD for depth normalization (default: $10,000)
    pub target_vwap_usd: Decimal,
    /// Maximum depth score cap (default: 100)
    pub depth_cap: Decimal,
    /// Maximum volatility score cap (default: 100)
    pub volatility_cap: Decimal,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            depth_weight: Decimal::new(3, 1),
            volatility_weight: Decimal::new(2, 1),
            reliability_weight: Decimal::new(2, 1),
            spread_stability_weight: Decimal::new(15, 2),
            freshness_weight: Decimal::new(15, 2),
            min_confidence_threshold: Decimal::new(3, 1),
            max_latency_ms: 500,
            target_vwap_usd: Decimal::from(10000),
            depth_cap: Decimal::from(100),
            volatility_cap: Decimal::from(100),
        }
    }
}

/// Exchange reliability metrics for scoring.
///
/// Tracks historical performance metrics for each exchange.
#[derive(Debug, Clone)]
pub struct ExchangeReliability {
    /// Uptime percentage (0-100)
    pub uptime_percent: Decimal,
    /// Error rate (0-1)
    pub error_rate: Decimal,
    /// Average latency in milliseconds
    pub avg_latency_ms: u64,
    /// Last metrics update timestamp
    pub last_updated: DateTime<Utc>,
}

/// Fee schedule for profit calculations.
///
/// Contains exchange-specific maker and taker fee rates.
#[derive(Debug, Clone)]
pub struct FeeSchedule {
    /// The exchange this schedule applies to
    pub exchange: ExchangeId,
    /// Maker fee rate (for providing liquidity)
    pub maker_fee: Decimal,
    /// Taker fee rate (for taking liquidity)
    pub taker_fee: Decimal,
    /// Optional fee tier identifier
    pub tier: Option<String>,
}

impl FeeSchedule {
    /// Creates a new FeeSchedule.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `maker_fee` - Maker fee rate
    /// * `taker_fee` - Taker fee rate
    pub fn new(exchange: ExchangeId, maker_fee: Decimal, taker_fee: Decimal) -> Self {
        Self {
            exchange,
            maker_fee,
            taker_fee,
            tier: None,
        }
    }

    /// Gets the applicable fee rate.
    ///
    /// # Arguments
    ///
    /// * `is_maker` - True for maker fee, false for taker fee
    ///
    /// # Returns
    ///
    /// The appropriate fee rate.
    pub fn get_fee_rate(&self, is_maker: bool) -> Decimal {
        if is_maker {
            self.maker_fee
        } else {
            self.taker_fee
        }
    }
}

/// Confidence scorer with fee-aware profit calculation.
///
/// This is a critical component that prevents unprofitable signals by:
/// 1. Calculating net spread after exchange fees
/// 2. Validating data freshness to prevent phantom signals
/// 3. Computing multi-factor confidence scores
///
/// # Example
///
/// ```rust
/// use arbitrage_core::confidence_scorer::{ConfidenceScorer, ConfidenceConfig, NetSpreadResult};
/// use arbitrage_core::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol, VwapResult};
/// use rust_decimal::Decimal;
///
/// let config = ConfidenceConfig::default();
/// let scorer = ConfidenceScorer::new(config);
///
/// // Calculate fee-adjusted spread
/// let result = scorer.calculate_net_spread_bps(
///     Decimal::from(50000),  // buy price
///     Decimal::from(50200),  // sell price
///     ExchangeId::Binance,
///     ExchangeId::ByBit,
/// );
///
/// match result {
///     NetSpreadResult::Profit(bps) => println!("Profit: {} bps", bps),
///     NetSpreadResult::Unprofitable => println!("Not profitable after fees"),
/// }
/// ```
pub struct ConfidenceScorer {
    /// Scoring configuration
    config: ConfidenceConfig,
    /// Exchange reliability metrics
    exchange_reliability: HashMap<ExchangeId, ExchangeReliability>,
    /// Exchange fee schedules
    fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    /// Cached total weight for confidence calculation
    cached_total_weight: Decimal,
}

impl ConfidenceScorer {
    /// Creates a new ConfidenceScorer with default fee schedules.
    ///
    /// # Arguments
    ///
    /// * `config` - Scoring configuration
    pub fn new(config: ConfidenceConfig) -> Self {
        let total_weight = config.depth_weight
            + config.volatility_weight
            + config.reliability_weight
            + config.spread_stability_weight
            + config.freshness_weight;

        Self {
            config,
            exchange_reliability: HashMap::new(),
            fee_schedules: Self::default_fee_schedules(),
            cached_total_weight: total_weight,
        }
    }

    /// Creates default fee schedules for major exchanges.
    ///
    /// These are approximate rates based on standard tier schedules.
    /// Actual rates may vary based on user tier and volume.
    fn default_fee_schedules() -> HashMap<ExchangeId, FeeSchedule> {
        let mut schedules = HashMap::new();

        schedules.insert(
            ExchangeId::OKX,
            FeeSchedule::new(
                ExchangeId::OKX,
                Decimal::new(8, 4), // 0.0008 = 0.08% maker
                Decimal::new(1, 3), // 0.001 = 0.1% taker
            ),
        );
        schedules.insert(
            ExchangeId::ByBit,
            FeeSchedule::new(
                ExchangeId::ByBit,
                Decimal::new(1, 3), // 0.001 = 0.1% maker
                Decimal::new(1, 3), // 0.001 = 0.1% taker
            ),
        );
        schedules.insert(
            ExchangeId::MEXC,
            FeeSchedule::new(
                ExchangeId::MEXC,
                Decimal::new(2, 3), // 0.002 = 0.2% maker
                Decimal::new(2, 3), // 0.002 = 0.2% taker
            ),
        );
        schedules.insert(
            ExchangeId::GateIo,
            FeeSchedule::new(
                ExchangeId::GateIo,
                Decimal::new(2, 3), // 0.002 = 0.2% maker
                Decimal::new(2, 3), // 0.002 = 0.2% taker
            ),
        );

        schedules
    }

    /// Calculates net spread after fees in basis points.
    ///
    /// This is the critical function that prevents unprofitable signals.
    /// It accounts for:
    /// - Buy-side taker fees (added to effective buy price)
    /// - Sell-side taker fees (subtracted from effective sell price)
    ///
    /// # Arguments
    ///
    /// * `buy_price` - Price on the buy exchange
    /// * `sell_price` - Price on the sell exchange
    /// * `buy_exchange` - The exchange to buy from
    /// * `sell_exchange` - The exchange to sell to
    ///
    /// # Returns
    ///
    /// `NetSpreadResult::Profit(bps)` if profitable, `NetSpreadResult::Unprofitable` otherwise.
    ///
    /// # Preconditions
    ///
    /// - `buy_price` must be greater than zero
    /// - Fee schedules must be loaded for both exchanges (or defaults are used)
    ///
    /// # Postconditions
    ///
    /// - Returns a positive profit in basis points if the spread exceeds combined fees
    /// - Returns `Unprofitable` if fees exceed the gross spread
    pub fn calculate_net_spread_bps(
        &self,
        buy_price: Decimal,
        sell_price: Decimal,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
    ) -> NetSpreadResult {
        if buy_price.is_zero() {
            return NetSpreadResult::Unprofitable;
        }

        let default_fee = Decimal::new(1, 3);
        let buy_fee_rate = self
            .fee_schedules
            .get(&buy_exchange)
            .map(|f| f.get_fee_rate(false))
            .unwrap_or(default_fee);
        let sell_fee_rate = self
            .fee_schedules
            .get(&sell_exchange)
            .map(|f| f.get_fee_rate(false))
            .unwrap_or(default_fee);

        let effective_buy = buy_price * (Decimal::ONE + buy_fee_rate);
        let effective_sell = sell_price * (Decimal::ONE - sell_fee_rate);

        if effective_sell <= effective_buy {
            return NetSpreadResult::Unprofitable;
        }

        if effective_buy <= Decimal::ZERO {
            return NetSpreadResult::Unprofitable;
        }

        let net_spread_ratio = match (effective_sell - effective_buy).checked_div(effective_buy) {
            Some(ratio) => ratio,
            None => return NetSpreadResult::Unprofitable,
        };

        let spread_bps = match net_spread_ratio.checked_mul(Decimal::from(10000)) {
            Some(bps) => bps,
            None => return NetSpreadResult::Unprofitable,
        };

        match spread_bps.to_i32() {
            Some(bps) => NetSpreadResult::Profit(bps),
            None => NetSpreadResult::Unprofitable,
        }
    }

    /// Checks if market data is fresh enough for signal generation.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp of the market data
    ///
    /// # Returns
    ///
    /// `true` if the data age is within `max_latency_ms`, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use chrono::Utc;
    /// use arbitrage_core::confidence_scorer::ConfidenceScorer;
    ///
    /// let scorer = ConfidenceScorer::default();
    ///
    /// // Fresh data
    /// assert!(scorer.is_data_fresh(Utc::now()));
    ///
    /// // Old data
    /// let old_time = Utc::now() - chrono::Duration::milliseconds(1000);
    /// assert!(!scorer.is_data_fresh(old_time));
    /// ```
    pub fn is_data_fresh(&self, timestamp: DateTime<Utc>) -> bool {
        let age_ms = (Utc::now() - timestamp).num_milliseconds();
        age_ms >= 0 && (age_ms as u64) <= self.config.max_latency_ms
    }

    /// Calculates confidence factors based on market conditions.
    ///
    /// Analyzes order books and VWAP results to compute individual factor scores.
    ///
    /// # Arguments
    ///
    /// * `buy_vwap` - VWAP result for the buy side
    /// * `sell_vwap` - VWAP result for the sell side
    /// * `buy_book` - Order book from the buy exchange
    /// * `sell_book` - Order book from the sell exchange
    ///
    /// # Returns
    ///
    /// A `ConfidenceFactors` struct with individual scores (0-100).
    ///
    /// # Factors Calculated
    ///
    /// - **depth_score**: Based on filled USD value normalized by target VWAP
    /// - **reliability_score**: Based on order book validity and exchange uptime
    /// - **spread_stability_score**: Inverse of average slippage
    /// - **freshness_score**: Based on data age for both books
    /// - **volatility_score**: Based on slippage difference and maximum slippage
    pub fn calculate_confidence_factors(
        &self,
        buy_vwap: &VwapResult,
        sell_vwap: &VwapResult,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
    ) -> ConfidenceFactors {
        let min_filled = buy_vwap.filled_quantity.min(sell_vwap.filled_quantity);
        let mid_price = buy_book
            .mid_price()
            .unwrap_or_else(|| sell_book.mid_price().unwrap_or(Decimal::from(50000)));
        let filled_usd = min_filled * mid_price;
        let depth_ratio = filled_usd / self.config.target_vwap_usd;
        let depth_score = (depth_ratio * Decimal::from(100)).min(self.config.depth_cap);

        let book_valid = buy_book.is_valid() && sell_book.is_valid();
        let fills_complete = buy_vwap.is_fully_filled && sell_vwap.is_fully_filled;

        let buy_reliability = self
            .exchange_reliability
            .get(&buy_book.exchange)
            .map(|r| r.uptime_percent / Decimal::from(100))
            .unwrap_or(Decimal::ONE);
        let sell_reliability = self
            .exchange_reliability
            .get(&sell_book.exchange)
            .map(|r| r.uptime_percent / Decimal::from(100))
            .unwrap_or(Decimal::ONE);
        let avg_reliability = (buy_reliability + sell_reliability) / Decimal::from(2);

        let base_reliability = match (book_valid, fills_complete) {
            (true, true) => Decimal::from(100),
            (true, false) => Decimal::from(70),
            (false, true) => Decimal::from(50),
            (false, false) => Decimal::from(20),
        };
        let reliability_score = base_reliability * avg_reliability;

        let avg_slippage_bps = (buy_vwap.slippage_bps + sell_vwap.slippage_bps) / 2;
        let spread_stability_score = (Decimal::from(100) - Decimal::from(avg_slippage_bps))
            .max(Decimal::ZERO)
            .min(Decimal::from(100));

        let buy_fresh = self.is_data_fresh(buy_book.timestamp);
        let sell_fresh = self.is_data_fresh(sell_book.timestamp);
        let freshness_score = match (buy_fresh, sell_fresh) {
            (true, true) => Decimal::from(100),
            (true, false) | (false, true) => Decimal::from(50),
            (false, false) => Decimal::ZERO,
        };

        let slippage_diff = (buy_vwap.slippage_bps - sell_vwap.slippage_bps).abs();
        let max_slippage = buy_vwap.slippage_bps.max(sell_vwap.slippage_bps);

        let volatility_penalty =
            Decimal::from(slippage_diff) + (Decimal::from(max_slippage) / Decimal::from(2));
        let volatility_score = (Decimal::from(100) - volatility_penalty)
            .max(Decimal::ZERO)
            .min(self.config.volatility_cap);

        ConfidenceFactors {
            depth_score,
            volatility_score,
            reliability_score,
            spread_stability_score,
            freshness_score,
        }
    }

    /// Calculates overall confidence score from factors.
    ///
    /// Applies configured weights to each factor and normalizes to 0-100 scale.
    ///
    /// # Arguments
    ///
    /// * `factors` - The confidence factors to score
    ///
    /// # Returns
    ///
    /// A decimal value between 0 and 100 representing overall confidence.
    pub fn calculate_confidence(&self, factors: &ConfidenceFactors) -> Decimal {
        let total_weight = self.cached_total_weight;

        if total_weight.is_zero() {
            return Decimal::ZERO;
        }

        let weighted_score = factors.depth_score * self.config.depth_weight
            + factors.volatility_score * self.config.volatility_weight
            + factors.reliability_score * self.config.reliability_weight
            + factors.spread_stability_score * self.config.spread_stability_weight
            + factors.freshness_score * self.config.freshness_weight;

        let normalized_score = (weighted_score / total_weight) * Decimal::from(100);
        normalized_score.min(Decimal::from(100)).max(Decimal::ZERO)
    }

    /// Updates exchange reliability metrics.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange to update
    /// * `reliability` - The new reliability metrics
    pub fn update_exchange_reliability(
        &mut self,
        exchange: ExchangeId,
        reliability: ExchangeReliability,
    ) {
        self.exchange_reliability.insert(exchange, reliability);
    }

    /// Gets exchange reliability metrics.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange to query
    ///
    /// # Returns
    ///
    /// `Some(&ExchangeReliability)` if metrics exist, `None` otherwise.
    pub fn get_exchange_reliability(&self, exchange: ExchangeId) -> Option<&ExchangeReliability> {
        self.exchange_reliability.get(&exchange)
    }

    /// Updates fee schedule for an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange to update
    /// * `schedule` - The new fee schedule
    pub fn update_fee_schedule(&mut self, exchange: ExchangeId, schedule: FeeSchedule) {
        self.fee_schedules.insert(exchange, schedule);
    }

    /// Gets fee rate for a specific exchange and side.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange to query
    /// * `is_maker` - True for maker fee, false for taker fee
    ///
    /// # Returns
    ///
    /// The fee rate, or 0.1% (0.001) if not configured.
    pub fn get_fee_rate(&self, exchange: ExchangeId, is_maker: bool) -> Decimal {
        self.fee_schedules
            .get(&exchange)
            .map(|f| f.get_fee_rate(is_maker))
            .unwrap_or_else(|| Decimal::new(1, 3))
    }

    /// Gets the minimum confidence threshold.
    ///
    /// # Returns
    ///
    /// The configured minimum confidence threshold.
    pub fn get_min_confidence_threshold(&self) -> Decimal {
        self.config.min_confidence_threshold
    }
}

impl Default for ConfidenceScorer {
    fn default() -> Self {
        Self::new(ConfidenceConfig::default())
    }
}
