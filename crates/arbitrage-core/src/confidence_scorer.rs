use crate::types::{ExchangeId, OrderBook, VwapResult};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Result type for net spread calculation
pub enum NetSpreadResult {
    Profit(i32),
    Unprofitable,
}

impl NetSpreadResult {
    pub fn is_unprofitable(&self) -> bool {
        matches!(self, NetSpreadResult::Unprofitable)
    }

    pub fn is_profitable(&self) -> bool {
        matches!(self, NetSpreadResult::Profit(_))
    }

    pub fn profit_value(self) -> Option<i32> {
        match self {
            NetSpreadResult::Profit(value) => Some(value),
            NetSpreadResult::Unprofitable => None,
        }
    }
}

/// Factors contributing to confidence score
#[derive(Debug, Clone)]
pub struct ConfidenceFactors {
    pub depth_score: Decimal,
    pub volatility_score: Decimal,
    pub reliability_score: Decimal,
    pub spread_stability_score: Decimal,
    pub freshness_score: Decimal,
}

/// Configuration for confidence scoring weights
#[derive(Debug, Clone)]
pub struct ConfidenceConfig {
    pub depth_weight: Decimal,
    pub volatility_weight: Decimal,
    pub reliability_weight: Decimal,
    pub spread_stability_weight: Decimal,
    pub freshness_weight: Decimal,
    pub min_confidence_threshold: Decimal,
    pub max_latency_ms: u64,
    pub target_vwap_usd: Decimal, // Target VWAP amount for depth normalization
    pub depth_cap: Decimal,       // Maximum depth score cap
    pub volatility_cap: Decimal,  // Maximum volatility score cap
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            depth_weight: Decimal::new(3, 1),             // 0.3
            volatility_weight: Decimal::new(2, 1),        // 0.2
            reliability_weight: Decimal::new(2, 1),       // 0.2
            spread_stability_weight: Decimal::new(15, 2), // 0.15
            freshness_weight: Decimal::new(15, 2),        // 0.15
            min_confidence_threshold: Decimal::new(3, 1), // 0.3
            max_latency_ms: 500,                          // 500ms max latency
            target_vwap_usd: Decimal::from(10000),        // $10k target for depth normalization
            depth_cap: Decimal::from(100),
            volatility_cap: Decimal::from(100),
        }
    }
}

/// Exchange reliability metrics
#[derive(Debug, Clone)]
pub struct ExchangeReliability {
    pub uptime_percent: Decimal,
    pub error_rate: Decimal,
    pub avg_latency_ms: u64,
    pub last_updated: DateTime<Utc>,
}

/// Fee schedule for profit calculations
#[derive(Debug, Clone)]
pub struct FeeSchedule {
    pub exchange: ExchangeId,
    pub maker_fee: Decimal,
    pub taker_fee: Decimal,
    pub tier: Option<String>,
}

impl FeeSchedule {
    pub fn new(exchange: ExchangeId, maker_fee: Decimal, taker_fee: Decimal) -> Self {
        Self {
            exchange,
            maker_fee,
            taker_fee,
            tier: None,
        }
    }

    pub fn get_fee_rate(&self, is_maker: bool) -> Decimal {
        if is_maker {
            self.maker_fee
        } else {
            self.taker_fee
        }
    }
}

/// Confidence scorer with fee-aware profit calculation
/// This is a critical Phase 1 component that handles:
/// - Fee-adjusted profit calculations (prevents unprofitable signals)
/// - Data freshness validation (prevents phantom signals)
/// - Multi-factor confidence scoring
pub struct ConfidenceScorer {
    config: ConfidenceConfig,
    exchange_reliability: HashMap<ExchangeId, ExchangeReliability>,
    fee_schedules: HashMap<ExchangeId, FeeSchedule>,
}

impl ConfidenceScorer {
    pub fn new(config: ConfidenceConfig) -> Self {
        Self {
            config,
            exchange_reliability: HashMap::new(),
            fee_schedules: Self::default_fee_schedules(),
        }
    }

    /// Create default fee schedules for major exchanges
    fn default_fee_schedules() -> HashMap<ExchangeId, FeeSchedule> {
        let mut schedules = HashMap::new();

        // Default taker fees for major exchanges (approximate)
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

    /// Calculate net spread after fees in basis points
    /// This is THE critical function that prevents unprofitable signals
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

        // Get fee rates (default to 0.1% if not found)
        let default_fee = Decimal::new(1, 3); // 0.001 = 0.1%
        let buy_fee_rate = self
            .fee_schedules
            .get(&buy_exchange)
            .map(|f| f.get_fee_rate(false)) // Assume taker for now
            .unwrap_or(default_fee);
        let sell_fee_rate = self
            .fee_schedules
            .get(&sell_exchange)
            .map(|f| f.get_fee_rate(false)) // Assume taker for now
            .unwrap_or(default_fee);

        // Calculate effective prices after fees
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

        let spread_bps_i32 = match spread_bps.to_i32() {
            Some(bps) => bps,
            None => return NetSpreadResult::Unprofitable,
        };

        NetSpreadResult::Profit(spread_bps_i32)
    }

    /// Check if market data is fresh enough
    pub fn is_data_fresh(&self, timestamp: DateTime<Utc>) -> bool {
        let age_ms = (Utc::now() - timestamp).num_milliseconds();
        age_ms >= 0 && (age_ms as u64) <= self.config.max_latency_ms
    }

    /// Calculate confidence factors based on market conditions
    pub fn calculate_confidence_factors(
        &self,
        buy_vwap: &VwapResult,
        sell_vwap: &VwapResult,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
    ) -> ConfidenceFactors {
        let depth_score = self.calculate_depth_score(buy_vwap, sell_vwap, buy_book, sell_book);
        let reliability_score =
            self.calculate_reliability_score(buy_vwap, sell_vwap, buy_book, sell_book);
        let spread_stability_score = self.calculate_spread_stability_score(buy_vwap, sell_vwap);
        let freshness_score = self.calculate_freshness_score(buy_book, sell_book);
        let volatility_score = self.calculate_volatility_score(buy_vwap, sell_vwap);

        ConfidenceFactors {
            depth_score,
            volatility_score,
            reliability_score,
            spread_stability_score,
            freshness_score,
        }
    }

    fn calculate_depth_score(
        &self,
        buy_vwap: &VwapResult,
        sell_vwap: &VwapResult,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
    ) -> Decimal {
        let min_filled = buy_vwap.filled_quantity.min(sell_vwap.filled_quantity);
        let mid_price = buy_book
            .mid_price()
            .unwrap_or_else(|| sell_book.mid_price().unwrap_or(Decimal::from(50000)));
        let filled_usd = min_filled * mid_price;
        let depth_ratio = filled_usd / self.config.target_vwap_usd;
        (depth_ratio * Decimal::from(100)).min(self.config.depth_cap)
    }

    fn calculate_reliability_score(
        &self,
        buy_vwap: &VwapResult,
        sell_vwap: &VwapResult,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
    ) -> Decimal {
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
        base_reliability * avg_reliability
    }

    fn calculate_spread_stability_score(
        &self,
        buy_vwap: &VwapResult,
        sell_vwap: &VwapResult,
    ) -> Decimal {
        let avg_slippage_bps = (buy_vwap.slippage_bps + sell_vwap.slippage_bps) / 2;
        (Decimal::from(100) - Decimal::from(avg_slippage_bps))
            .max(Decimal::ZERO)
            .min(Decimal::from(100))
    }

    fn calculate_freshness_score(&self, buy_book: &OrderBook, sell_book: &OrderBook) -> Decimal {
        let buy_fresh = self.is_data_fresh(buy_book.timestamp);
        let sell_fresh = self.is_data_fresh(sell_book.timestamp);
        match (buy_fresh, sell_fresh) {
            (true, true) => Decimal::from(100),
            (true, false) | (false, true) => Decimal::from(50),
            (false, false) => Decimal::ZERO,
        }
    }

    fn calculate_volatility_score(&self, buy_vwap: &VwapResult, sell_vwap: &VwapResult) -> Decimal {
        let slippage_diff = (buy_vwap.slippage_bps - sell_vwap.slippage_bps).abs();
        let max_slippage = buy_vwap.slippage_bps.max(sell_vwap.slippage_bps);
        let volatility_penalty =
            Decimal::from(slippage_diff) + (Decimal::from(max_slippage) / Decimal::from(2));
        (Decimal::from(100) - volatility_penalty)
            .max(Decimal::ZERO)
            .min(self.config.volatility_cap)
    }

    /// Calculate overall confidence score with normalized weights
    pub fn calculate_confidence(&self, factors: &ConfidenceFactors) -> Decimal {
        // Calculate total weight for normalization
        let total_weight = self.config.depth_weight
            + self.config.volatility_weight
            + self.config.reliability_weight
            + self.config.spread_stability_weight
            + self.config.freshness_weight;

        if total_weight.is_zero() {
            return Decimal::ZERO;
        }

        let weighted_score = factors.depth_score * self.config.depth_weight
            + factors.volatility_score * self.config.volatility_weight
            + factors.reliability_score * self.config.reliability_weight
            + factors.spread_stability_score * self.config.spread_stability_weight
            + factors.freshness_score * self.config.freshness_weight;

        // Normalize to 0-100 scale using total weights
        let normalized_score = (weighted_score / total_weight) * Decimal::from(100);
        normalized_score.min(Decimal::from(100)).max(Decimal::ZERO)
    }

    /// Update exchange reliability metrics
    pub fn update_exchange_reliability(
        &mut self,
        exchange: ExchangeId,
        reliability: ExchangeReliability,
    ) {
        self.exchange_reliability.insert(exchange, reliability);
    }

    /// Get exchange reliability metrics
    pub fn get_exchange_reliability(&self, exchange: ExchangeId) -> Option<&ExchangeReliability> {
        self.exchange_reliability.get(&exchange)
    }

    /// Update fee schedule for an exchange
    pub fn update_fee_schedule(&mut self, exchange: ExchangeId, schedule: FeeSchedule) {
        self.fee_schedules.insert(exchange, schedule);
    }

    /// Get fee rate for a specific exchange and side
    pub fn get_fee_rate(&self, exchange: ExchangeId, is_maker: bool) -> Decimal {
        self.fee_schedules
            .get(&exchange)
            .map(|f| f.get_fee_rate(is_maker))
            .unwrap_or_else(|| Decimal::new(1, 3)) // Default 0.1%
    }

    /// Get minimum confidence threshold
    pub fn get_min_confidence_threshold(&self) -> Decimal {
        self.config.min_confidence_threshold
    }
}

impl Default for ConfidenceScorer {
    fn default() -> Self {
        Self::new(ConfidenceConfig::default())
    }
}
