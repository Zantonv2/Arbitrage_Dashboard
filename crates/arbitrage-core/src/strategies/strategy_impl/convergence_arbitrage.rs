#![allow(clippy::type_complexity)]

use crate::strategies::strategies_specifics::{StrategyLimits, StrategyUtils};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ExchangeId, Result, Side, Symbol};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;
use tracing::debug;

/// Convergence Arbitrage Strategy
///
/// **Principle**: Exploit temporary price divergences between related instruments
/// that are expected to converge over time. Trade on mean reversion patterns
/// and statistical relationships between correlated assets.
///
/// **Focus**:
/// - Statistical modeling of price relationships
/// - Mean reversion detection for correlated pairs
/// - Cointegration analysis between related assets
/// - Time-based convergence prediction
///
/// **Supported Exchanges**: All exchanges for maximum pair coverage
///
/// **Data Sources**:
/// - Historical price correlations
/// - Cross-asset price relationships
/// - Statistical deviation metrics
/// - Volume and volatility indicators
pub struct ConvergenceArbitrageStrategy {
    config: StrategyConfig,
    #[allow(dead_code)]
    /// Historical price data for correlation analysis
    price_history: HashMap<(ExchangeId, Symbol), Vec<(DateTime<Utc>, Decimal)>>,
    #[allow(dead_code)]
    /// Correlation coefficients between asset pairs
    correlations: HashMap<(Symbol, Symbol), Decimal>,
    #[allow(dead_code)]
    /// Z-score thresholds for mean reversion
    z_score_cache: HashMap<(Symbol, Symbol), Decimal>,
}

impl ConvergenceArbitrageStrategy {
    /// Create a new convergence arbitrage strategy with default configuration
    pub fn new() -> Self {
        let custom_params = StrategyUtils::create_base_custom_params();

        Self {
            config: StrategyConfig {
                enabled: true,
                min_profit_bps: StrategyLimits::get_min_profit_bps("convergence_arbitrage"),
                max_exposure: StrategyLimits::get_max_exposure("convergence_arbitrage"),
                confidence_threshold: Decimal::new(7, 1),
                risk_limits: RiskLimits::default(),
                custom_params,
            },
            price_history: HashMap::new(),
            correlations: HashMap::new(),
            z_score_cache: HashMap::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        let mut strategy = Self::new();
        strategy.config = config;
        strategy
    }

    #[allow(dead_code)]
    /// Update price history for correlation analysis
    fn update_price_history(&mut self, market_data: &MarketBundle) {
        let current_time = market_data.timestamp;

        // Update from tickers
        for ((exchange, symbol), ticker) in &market_data.tickers {
            let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);
            let key = (*exchange, (**symbol).clone());

            let history = self.price_history.entry(key).or_default();

            // Keep only recent history
            let lookback_hours = self
                .config
                .custom_params
                .get("lookback_period_hours")
                .and_then(|v| v.as_i64())
                .unwrap_or(24);
            let cutoff = current_time - Duration::hours(lookback_hours);

            history.retain(|(ts, _)| *ts > cutoff);
            history.push((current_time, mid_price));

            // Keep sorted by timestamp
            history.sort_by_key(|(ts, _)| *ts);
        }
    }

    #[allow(dead_code)]
    /// Calculate correlation between two symbols
    fn calculate_correlation(&self, symbol1: &Symbol, symbol2: &Symbol) -> Option<Decimal> {
        // Get price series for both symbols (using first available exchange)
        let prices1 = self.get_price_series(symbol1)?;
        let prices2 = self.get_price_series(symbol2)?;

        if prices1.len() < 10 || prices2.len() < 10 {
            return None;
        }

        // Align timestamps and calculate returns
        let returns1 = self.calculate_returns(&prices1);
        let returns2 = self.calculate_returns(&prices2);

        if returns1.len() != returns2.len() || returns1.len() < 5 {
            return None;
        }

        // Calculate Pearson correlation coefficient
        let n = returns1.len() as f64;
        let sum1: f64 = returns1.iter().map(|r| r.to_f64().unwrap_or(0.0)).sum();
        let sum2: f64 = returns2.iter().map(|r| r.to_f64().unwrap_or(0.0)).sum();
        let sum1_sq: f64 = returns1
            .iter()
            .map(|r| r.to_f64().unwrap_or(0.0).powi(2))
            .sum();
        let sum2_sq: f64 = returns2
            .iter()
            .map(|r| r.to_f64().unwrap_or(0.0).powi(2))
            .sum();
        let sum_prod: f64 = returns1
            .iter()
            .zip(&returns2)
            .map(|(r1, r2)| r1.to_f64().unwrap_or(0.0) * r2.to_f64().unwrap_or(0.0))
            .sum();

        let numerator = n * sum_prod - sum1 * sum2;
        let denominator = ((n * sum1_sq - sum1.powi(2)) * (n * sum2_sq - sum2.powi(2))).sqrt();

        if denominator == 0.0 {
            return None;
        }

        let correlation = numerator / denominator;
        Decimal::try_from(correlation).ok()
    }

    #[allow(dead_code)]
    /// Get price series for a symbol (from any exchange)
    fn get_price_series(&self, symbol: &Symbol) -> Option<Vec<(DateTime<Utc>, Decimal)>> {
        // Find the exchange with the most data for this symbol
        let mut best_series: Option<Vec<(DateTime<Utc>, Decimal)>> = None;
        let mut max_length = 0;

        for ((_, sym), history) in &self.price_history {
            if sym == symbol && history.len() > max_length {
                max_length = history.len();
                best_series = Some(history.clone());
            }
        }

        best_series
    }

    #[allow(dead_code)]
    /// Calculate returns from price series
    fn calculate_returns(&self, prices: &[(DateTime<Utc>, Decimal)]) -> Vec<Decimal> {
        let mut returns = Vec::new();

        for i in 1..prices.len() {
            let prev_price = prices[i - 1].1;
            let curr_price = prices[i].1;

            if !prev_price.is_zero() {
                let return_val = (curr_price - prev_price) / prev_price;
                returns.push(return_val);
            }
        }

        returns
    }

    #[allow(dead_code)]
    /// Calculate Z-score for price divergence
    fn calculate_z_score(
        &self,
        symbol1: &Symbol,
        symbol2: &Symbol,
        current_ratio: Decimal,
    ) -> Option<Decimal> {
        let prices1 = self.get_price_series(symbol1)?;
        let prices2 = self.get_price_series(symbol2)?;

        if prices1.len() < 20 || prices2.len() < 20 {
            return None;
        }

        // Calculate historical price ratios
        let mut ratios = Vec::new();
        let min_len = prices1.len().min(prices2.len());

        for i in 0..min_len {
            let price1 = prices1[i].1;
            let price2 = prices2[i].1;

            if !price2.is_zero() {
                ratios.push(price1 / price2);
            }
        }

        if ratios.len() < 10 {
            return None;
        }

        // Calculate mean and standard deviation
        let mean = ratios.iter().sum::<Decimal>() / Decimal::from(ratios.len());

        let variance: Decimal = ratios
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff // Use multiplication instead of powi
            })
            .sum::<Decimal>()
            / Decimal::from(ratios.len());

        // Use f64 for sqrt calculation, then convert back to Decimal
        let variance_f64 = variance.to_f64().unwrap_or(0.0);
        let std_dev_f64 = variance_f64.sqrt();
        let std_dev = Decimal::try_from(std_dev_f64).ok()?;

        if std_dev.is_zero() {
            return None;
        }

        // Calculate Z-score
        let z_score = (current_ratio - mean) / std_dev;
        Some(z_score)
    }

    #[allow(dead_code)]
    /// Find convergence opportunities
    fn find_convergence_opportunities(&mut self, market_data: &MarketBundle) -> Vec<RawSignal> {
        let mut signals = Vec::new();

        // Update price history
        self.update_price_history(market_data);

        // Get all unique symbols
        let symbols = market_data.get_all_symbols();

        // Check pairs of symbols for convergence opportunities
        for i in 0..symbols.len() {
            for j in (i + 1)..symbols.len() {
                let symbol1 = &symbols[i];
                let symbol2 = &symbols[j];

                // Skip if symbols are too similar (same base or quote)
                if symbol1.base == symbol2.base || symbol1.quote == symbol2.quote {
                    continue;
                }

                // Calculate correlation
                let correlation = match self.calculate_correlation(symbol1, symbol2) {
                    Some(corr) => corr,
                    None => continue,
                };

                let min_correlation = self
                    .config
                    .custom_params
                    .get("min_correlation")
                    .and_then(|v| v.as_f64())
                    .and_then(|f| Decimal::try_from(f).ok())
                    .unwrap_or_else(|| Decimal::new(7, 1)); // 0.7

                // Skip if correlation is too low
                if correlation.abs() < min_correlation {
                    continue;
                }

                // Get current prices
                let price1 = self.get_current_price(market_data, symbol1);
                let price2 = self.get_current_price(market_data, symbol2);

                let (price1, price2) = match (price1, price2) {
                    (Some(p1), Some(p2)) => (p1, p2),
                    _ => continue,
                };

                if price2.is_zero() {
                    continue;
                }

                // Calculate current ratio and Z-score
                let current_ratio = price1 / price2;
                let z_score = match self.calculate_z_score(symbol1, symbol2, current_ratio) {
                    Some(z) => z,
                    None => continue,
                };

                let z_threshold = self
                    .config
                    .custom_params
                    .get("z_score_entry_threshold")
                    .and_then(|v| v.as_f64())
                    .and_then(|f| Decimal::try_from(f).ok())
                    .unwrap_or_else(|| Decimal::from(2));

                // Check if Z-score indicates divergence
                if z_score.abs() < z_threshold {
                    continue;
                }

                // Determine trade direction
                let (long_symbol, short_symbol, long_price, short_price) =
                    if z_score > Decimal::ZERO {
                        // Ratio is above mean - short symbol1, long symbol2
                        (symbol2, symbol1, price2, price1)
                    } else {
                        // Ratio is below mean - long symbol1, short symbol2
                        (symbol1, symbol2, price1, price2)
                    };

                // Find exchanges with both symbols
                let common_exchanges =
                    self.find_common_exchanges(market_data, long_symbol, short_symbol);

                for exchange in common_exchanges {
                    // Calculate position sizes
                    let position_value = self.config.max_exposure / Decimal::from(4); // 25% of max exposure
                    let long_quantity = position_value / long_price;
                    let short_quantity = position_value / short_price;

                    // Create convergence signal
                    let mut signal = RawSignal::new(self.id(), (**long_symbol).clone());

                    // Long leg
                    let long_leg = TradeLeg::new(
                        exchange,
                        (**long_symbol).clone(),
                        Side::Buy,
                        long_price,
                        long_quantity,
                    );
                    signal.add_leg(long_leg);

                    // Short leg
                    let short_leg = TradeLeg::new(
                        exchange,
                        (**short_symbol).clone(),
                        Side::Sell,
                        short_price,
                        short_quantity,
                    );
                    signal.add_leg(short_leg);

                    // Estimate profit based on expected convergence
                    let expected_convergence_bps = (z_score.abs() * Decimal::from(50))
                        .to_i32()
                        .unwrap_or(0)
                        .min(500); // Cap at 5%

                    signal.set_profit_bps(expected_convergence_bps);

                    // Add metadata
                    signal.add_metadata("pair_correlation", json!(correlation.to_string()));
                    signal.add_metadata("z_score", json!(z_score.to_string()));
                    signal.add_metadata("current_ratio", json!(current_ratio.to_string()));
                    signal.add_metadata("long_symbol", json!(long_symbol.to_pair()));
                    signal.add_metadata("short_symbol", json!(short_symbol.to_pair()));
                    signal.add_metadata("convergence_type", json!("mean_reversion"));

                    signals.push(signal);

                    debug!(
                        "Convergence arbitrage: {} vs {} correlation={} z_score={} profit={}bps",
                        long_symbol, short_symbol, correlation, z_score, expected_convergence_bps
                    );
                }
            }
        }

        signals
    }

    /// Get current mid price for a symbol
    fn get_current_price(&self, market_data: &MarketBundle, symbol: &Symbol) -> Option<Decimal> {
        // Try to get from any available exchange
        for ((_, sym), ticker) in &market_data.tickers {
            if **sym == *symbol {
                return Some((ticker.bid + ticker.ask) / Decimal::from(2));
            }
        }
        None
    }

    /// Find exchanges that have both symbols
    fn find_common_exchanges(
        &self,
        market_data: &MarketBundle,
        symbol1: &Symbol,
        symbol2: &Symbol,
    ) -> Vec<ExchangeId> {
        let mut exchanges1 = std::collections::HashSet::new();
        let mut exchanges2 = std::collections::HashSet::new();

        // Collect exchanges for symbol1
        for (exchange, sym) in market_data.tickers.keys() {
            if **sym == *symbol1 {
                exchanges1.insert(*exchange);
            }
        }

        // Collect exchanges for symbol2
        for (exchange, sym) in market_data.tickers.keys() {
            if **sym == *symbol2 {
                exchanges2.insert(*exchange);
            }
        }

        // Return intersection
        exchanges1.intersection(&exchanges2).copied().collect()
    }
}

impl Default for ConvergenceArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for ConvergenceArbitrageStrategy {
    fn id(&self) -> &'static str {
        "convergence_arbitrage"
    }

    fn name(&self) -> &'static str {
        "Convergence Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        debug!("Convergence arbitrage scanning market data");

        // For convergence arbitrage, we'll use a simplified approach that doesn't require
        // maintaining historical state. In a real implementation, correlation analysis
        // would be done by an external service.

        let mut signals = Vec::new();

        // Get all unique symbols
        let symbols = market_data.get_all_symbols();

        // Look for simple price ratio divergences between related symbols
        for i in 0..symbols.len() {
            for j in (i + 1)..symbols.len() {
                let symbol1 = &symbols[i];
                let symbol2 = &symbols[j];

                // Skip if symbols have the same base (e.g., BTC/USDT vs BTC/USD)
                if symbol1.base == symbol2.base {
                    continue;
                }

                // Only look at symbols with same quote currency for simplicity
                if symbol1.quote != symbol2.quote {
                    continue;
                }

                // Get current prices
                let price1 = self.get_current_price(market_data, symbol1);
                let price2 = self.get_current_price(market_data, symbol2);

                let (price1, price2) = match (price1, price2) {
                    (Some(p1), Some(p2)) => (p1, p2),
                    _ => continue,
                };

                if price2.is_zero() {
                    continue;
                }

                // Calculate current ratio
                let current_ratio = price1 / price2;

                // Use a simple heuristic: if ratio is very different from 1.0, it might be an opportunity
                // In reality, this would use historical mean and standard deviation
                let ratio_deviation = (current_ratio - Decimal::ONE).abs();
                let deviation_threshold = Decimal::new(2, 1); // 0.2 = 20%

                if ratio_deviation < deviation_threshold {
                    continue;
                }

                // Determine trade direction (mean reversion assumption)
                let (long_symbol, short_symbol, long_price, short_price) =
                    if current_ratio > Decimal::ONE {
                        // Ratio > 1: symbol1 expensive relative to symbol2
                        // Short symbol1, long symbol2
                        (symbol2, symbol1, price2, price1)
                    } else {
                        // Ratio < 1: symbol1 cheap relative to symbol2
                        // Long symbol1, short symbol2
                        (symbol1, symbol2, price1, price2)
                    };

                // Find exchanges with both symbols
                let common_exchanges =
                    self.find_common_exchanges(market_data, long_symbol, short_symbol);

                for exchange in common_exchanges {
                    // Calculate position sizes
                    let position_value = self.config.max_exposure / Decimal::from(4); // 25% of max exposure
                    let long_quantity = position_value / long_price;
                    let short_quantity = position_value / short_price;

                    // Create convergence signal
                    let mut signal = RawSignal::new(self.id(), (**long_symbol).clone());

                    // Long leg
                    let long_leg = TradeLeg::new(
                        exchange,
                        (**long_symbol).clone(),
                        Side::Buy,
                        long_price,
                        long_quantity,
                    );
                    signal.add_leg(long_leg);

                    // Short leg
                    let short_leg = TradeLeg::new(
                        exchange,
                        (**short_symbol).clone(),
                        Side::Sell,
                        short_price,
                        short_quantity,
                    );
                    signal.add_leg(short_leg);

                    // Estimate profit based on expected convergence
                    let expected_convergence_bps = (ratio_deviation * Decimal::from(1000))
                        .to_i32()
                        .unwrap_or(0)
                        .min(500); // Cap at 5%

                    signal.set_profit_bps(expected_convergence_bps);

                    // Add metadata
                    signal.add_metadata("current_ratio", json!(current_ratio.to_string()));
                    signal.add_metadata("ratio_deviation", json!(ratio_deviation.to_string()));
                    signal.add_metadata("long_symbol", json!(long_symbol.to_pair()));
                    signal.add_metadata("short_symbol", json!(short_symbol.to_pair()));
                    signal.add_metadata("convergence_type", json!("ratio_reversion"));

                    signals.push(signal);

                    debug!(
                        "Convergence arbitrage: {} vs {} ratio={} deviation={} profit={}bps",
                        long_symbol,
                        short_symbol,
                        current_ratio,
                        ratio_deviation,
                        expected_convergence_bps
                    );
                }
            }
        }

        debug!("Convergence arbitrage detected {} signals", signals.len());
        Ok(signals)
    }

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have exactly 2 legs (long + short)
        if signal.legs.len() != 2 {
            return Ok(false);
        }

        let long_leg = &signal.legs[0];
        let short_leg = &signal.legs[1];

        // Both legs should be on the same exchange
        if long_leg.exchange != short_leg.exchange {
            return Ok(false);
        }

        // Validate exchange is allowed
        if !context.is_exchange_allowed(long_leg.exchange) {
            return Ok(false);
        }

        // Check profit threshold
        if signal.expected_profit_bps < context.min_profit_bps {
            return Ok(false);
        }

        // Check maximum exposure
        let total_notional = signal.total_notional();
        if total_notional > context.max_exposure {
            return Ok(false);
        }

        // Check minimum notional
        if total_notional < context.min_notional_usd {
            return Ok(false);
        }

        // Check inventory for both legs
        let _long_base = &long_leg.symbol.base;
        let long_quote = &long_leg.symbol.quote;
        let short_base = &short_leg.symbol.base;

        // For long leg (buying), need quote currency
        let required_quote = long_leg
            .price
            .checked_mul(long_leg.quantity)
            .unwrap_or(Decimal::ZERO);
        if !context.can_sell(long_leg.exchange, long_quote, required_quote) {
            return Ok(false);
        }

        // For short leg (selling), need base currency
        if !context.can_sell(short_leg.exchange, short_base, short_leg.quantity) {
            return Ok(false);
        }

        Ok(true)
    }

    fn config(&self) -> &StrategyConfig {
        &self.config
    }

    fn update_config(&mut self, config: StrategyConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }
}
