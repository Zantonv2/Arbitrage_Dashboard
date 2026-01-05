use crate::{
    types::{ExchangeId, OrderBook, Signal},
    Result,
};
use rust_decimal::Decimal;
use std::collections::HashMap;

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
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            depth_weight: Decimal::from_str_exact("0.3").unwrap(),
            volatility_weight: Decimal::from_str_exact("0.2").unwrap(),
            reliability_weight: Decimal::from_str_exact("0.2").unwrap(),
            spread_stability_weight: Decimal::from_str_exact("0.15").unwrap(),
            freshness_weight: Decimal::from_str_exact("0.15").unwrap(),
            min_confidence_threshold: Decimal::from_str_exact("0.3").unwrap(),
        }
    }
}

/// Exchange reliability metrics
#[derive(Debug, Clone)]
pub struct ExchangeReliability {
    pub uptime_percent: Decimal,
    pub error_rate: Decimal,
    pub avg_latency_ms: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for ExchangeReliability {
    fn default() -> Self {
        Self {
            uptime_percent: Decimal::from(95), // Default 95% uptime
            error_rate: Decimal::from_str_exact("0.01").unwrap(), // 1% error rate
            avg_latency_ms: 100,
            last_updated: chrono::Utc::now(),
        }
    }
}

/// Volatility tracking for symbols
#[derive(Debug, Clone)]
pub struct VolatilityMetrics {
    pub price_variance: Decimal,
    pub rolling_std_dev: Decimal,
    pub sample_count: u32,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for VolatilityMetrics {
    fn default() -> Self {
        Self {
            price_variance: Decimal::ZERO,
            rolling_std_dev: Decimal::ZERO,
            sample_count: 0,
            last_updated: chrono::Utc::now(),
        }
    }
}

/// Spread stability tracking
#[derive(Debug, Clone)]
pub struct SpreadStability {
    pub avg_spread: Decimal,
    pub spread_variance: Decimal,
    pub stability_score: Decimal,
    pub sample_count: u32,
}

impl Default for SpreadStability {
    fn default() -> Self {
        Self {
            avg_spread: Decimal::ZERO,
            spread_variance: Decimal::ZERO,
            stability_score: Decimal::from_str_exact("0.5").unwrap(), // Neutral default
            sample_count: 0,
        }
    }
}

/// Confidence scorer for arbitrage signals
pub struct ConfidenceScorer {
    config: ConfidenceConfig,
    exchange_reliability: HashMap<ExchangeId, ExchangeReliability>,
    volatility_metrics: HashMap<String, VolatilityMetrics>, // symbol -> metrics
    spread_stability: HashMap<(ExchangeId, String), SpreadStability>, // (exchange, symbol) -> stability
}

impl ConfidenceScorer {
    pub fn new(config: ConfidenceConfig) -> Self {
        Self {
            config,
            exchange_reliability: HashMap::new(),
            volatility_metrics: HashMap::new(),
            spread_stability: HashMap::new(),
        }
    }

    /// Score a signal's confidence
    pub fn score_signal(
        &self,
        signal: &Signal,
        buy_order_book: &OrderBook,
        sell_order_book: &OrderBook,
    ) -> Result<(Decimal, ConfidenceFactors)> {
        let factors = ConfidenceFactors {
            depth_score: self.calculate_depth_score(buy_order_book, sell_order_book, signal)?,
            volatility_score: self.calculate_volatility_score(&signal.symbol.to_pair())?,
            reliability_score: self.calculate_reliability_score(signal.buy_exchange, signal.sell_exchange)?,
            spread_stability_score: self.calculate_spread_stability_score(buy_order_book, sell_order_book)?,
            freshness_score: self.calculate_freshness_score(buy_order_book, sell_order_book)?,
        };

        let weighted_score = self.calculate_weighted_score(&factors)?;
        
        Ok((weighted_score, factors))
    }

    /// Calculate depth score based on order book liquidity
    fn calculate_depth_score(
        &self,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
        signal: &Signal,
    ) -> Result<Decimal> {
        // Calculate available liquidity at signal prices
        let buy_liquidity = self.calculate_liquidity_at_price(&buy_book.asks, signal.buy_price)?;
        let sell_liquidity = self.calculate_liquidity_at_price(&sell_book.bids, signal.sell_price)?;

        // Use the limiting factor (minimum liquidity)
        let min_liquidity = buy_liquidity.min(sell_liquidity);

        // Normalize to 0-1 score (higher liquidity = higher score)
        // Using simple linear scale for now instead of logarithmic
        let score = if min_liquidity > Decimal::ZERO {
            // Simple normalization: cap at 1000 and scale to 0-1
            let normalized = min_liquidity / Decimal::from(1000);
            normalized.max(Decimal::ZERO).min(Decimal::ONE)
        } else {
            Decimal::ZERO
        };

        Ok(score)
    }

    /// Calculate available liquidity at a specific price level
    fn calculate_liquidity_at_price(
        &self,
        levels: &[crate::types::OrderBookLevel],
        target_price: Decimal,
    ) -> Result<Decimal> {
        let mut total_liquidity = Decimal::ZERO;

        for level in levels {
            // For asks, we can buy up to target_price
            // For bids, we can sell down to target_price
            if (levels == &[] || level.price <= target_price) || 
               (levels != &[] && level.price >= target_price) {
                total_liquidity += level.quantity;
            } else {
                break; // Levels are sorted, so we can stop here
            }
        }

        Ok(total_liquidity)
    }

    /// Calculate volatility score (lower volatility = higher confidence)
    fn calculate_volatility_score(&self, symbol: &str) -> Result<Decimal> {
        if let Some(metrics) = self.volatility_metrics.get(symbol) {
            // Convert standard deviation to confidence score
            // Lower std dev = higher confidence
            let volatility_factor = metrics.rolling_std_dev / Decimal::from(100); // Normalize
            let score = (Decimal::ONE - volatility_factor.min(Decimal::ONE)).max(Decimal::ZERO);
            Ok(score)
        } else {
            // Default neutral score if no volatility data
            Ok(Decimal::from_str_exact("0.5").unwrap())
        }
    }

    /// Calculate exchange reliability score
    fn calculate_reliability_score(
        &self,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
    ) -> Result<Decimal> {
        let buy_reliability = self.exchange_reliability
            .get(&buy_exchange)
            .cloned()
            .unwrap_or_default();
        
        let sell_reliability = self.exchange_reliability
            .get(&sell_exchange)
            .cloned()
            .unwrap_or_default();

        // Combine uptime and error rate for both exchanges
        let buy_score = (buy_reliability.uptime_percent / Decimal::from(100)) * 
                       (Decimal::ONE - buy_reliability.error_rate);
        let sell_score = (sell_reliability.uptime_percent / Decimal::from(100)) * 
                        (Decimal::ONE - sell_reliability.error_rate);

        // Use the minimum (weakest link)
        Ok(buy_score.min(sell_score))
    }

    /// Calculate spread stability score
    fn calculate_spread_stability_score(
        &self,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
    ) -> Result<Decimal> {
        let buy_key = (buy_book.exchange, buy_book.symbol.to_pair());
        let sell_key = (sell_book.exchange, sell_book.symbol.to_pair());

        let buy_stability = self.spread_stability
            .get(&buy_key)
            .map(|s| s.stability_score)
            .unwrap_or(Decimal::from_str_exact("0.5").unwrap());

        let sell_stability = self.spread_stability
            .get(&sell_key)
            .map(|s| s.stability_score)
            .unwrap_or(Decimal::from_str_exact("0.5").unwrap());

        // Average the stability scores
        Ok((buy_stability + sell_stability) / Decimal::from(2))
    }

    /// Calculate freshness score based on order book age
    fn calculate_freshness_score(
        &self,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
    ) -> Result<Decimal> {
        let now = chrono::Utc::now();
        let buy_age_ms = (now - buy_book.timestamp).num_milliseconds() as u64;
        let sell_age_ms = (now - sell_book.timestamp).num_milliseconds() as u64;

        // Use the older (worse) age
        let max_age_ms = buy_age_ms.max(sell_age_ms);

        // Fresher data gets higher score (linear decay for simplicity)
        let decay_seconds = max_age_ms / 1000; // Convert to seconds
        let score = if decay_seconds < 60 {
            // Linear decay over 60 seconds
            Decimal::ONE - (Decimal::from(decay_seconds) / Decimal::from(60))
        } else {
            Decimal::ZERO
        };

        Ok(score.max(Decimal::ZERO).min(Decimal::ONE))
    }

    /// Calculate weighted confidence score
    fn calculate_weighted_score(&self, factors: &ConfidenceFactors) -> Result<Decimal> {
        let weighted_sum = 
            factors.depth_score * self.config.depth_weight +
            factors.volatility_score * self.config.volatility_weight +
            factors.reliability_score * self.config.reliability_weight +
            factors.spread_stability_score * self.config.spread_stability_weight +
            factors.freshness_score * self.config.freshness_weight;

        // Ensure score is between 0 and 1
        let score = weighted_sum.max(Decimal::ZERO).min(Decimal::ONE);
        Ok(score)
    }

    /// Update exchange reliability metrics
    pub fn update_exchange_reliability(&mut self, exchange: ExchangeId, reliability: ExchangeReliability) {
        self.exchange_reliability.insert(exchange, reliability);
    }

    /// Update volatility metrics for a symbol
    pub fn update_volatility_metrics(&mut self, symbol: String, metrics: VolatilityMetrics) {
        self.volatility_metrics.insert(symbol, metrics);
    }

    /// Update spread stability for exchange/symbol pair
    pub fn update_spread_stability(&mut self, exchange: ExchangeId, symbol: String, stability: SpreadStability) {
        self.spread_stability.insert((exchange, symbol), stability);
    }

    /// Check if signal meets minimum confidence threshold
    pub fn meets_threshold(&self, confidence: Decimal) -> bool {
        confidence >= self.config.min_confidence_threshold
    }

    /// Get current configuration
    pub fn get_config(&self) -> &ConfidenceConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: ConfidenceConfig) {
        self.config = config;
    }
}

impl Default for ConfidenceScorer {
    fn default() -> Self {
        Self::new(ConfidenceConfig::default())
    }
}