#![allow(clippy::type_complexity)]

use crate::constants::{
    BASIS_POINTS_DIVISOR, DEFAULT_CONVERGENCE_POSITION_DIVISOR, DEFAULT_CONVERGENCE_PROFIT_CAP_BPS,
    DEFAULT_CONVERGENCE_PROFIT_MULTIPLIER, DEFAULT_RATIO_DEVIATION_MULTIPLIER,
};
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
///
/// **Note**: This is a simplified implementation that uses ratio-based analysis
/// instead of full statistical correlation. Future enhancements could include:
/// - Price history tracking for correlation analysis
/// - Z-score based mean reversion signals
/// - Dynamic correlation thresholds
/// - Machine learning-based convergence prediction
pub struct ConvergenceArbitrageStrategy {
    config: StrategyConfig,
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
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        let mut strategy = Self::new();
        strategy.config = config;
        strategy
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

        // Pre-index symbols by quote currency for O(1) lookup within groups
        // This reduces complexity from O(n²) to O(n log n) for opportunity detection
        let mut symbols_by_quote: HashMap<String, Vec<std::sync::Arc<crate::types::Symbol>>> =
            HashMap::new();
        for symbol in &symbols {
            symbols_by_quote
                .entry(symbol.quote.clone())
                .or_default()
                .push(symbol.clone());
        }

        // For each quote currency group, use optimized pair selection
        // instead of O(n²) nested loops
        for (_, quote_symbols) in &symbols_by_quote {
            if quote_symbols.len() < 2 {
                continue;
            }

            // Get current prices for all symbols in this group
            let mut symbol_prices: Vec<(&std::sync::Arc<crate::types::Symbol>, Decimal)> =
                Vec::new();
            for sym in quote_symbols {
                if let Some(price) = self.get_current_price(market_data, sym) {
                    symbol_prices.push((sym, price));
                }
            }

            if symbol_prices.len() < 2 {
                continue;
            }

            // Sort by price for efficient sampling - O(n log n)
            symbol_prices.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            // O(n) comparison strategy: only compare extreme and median symbols
            // This reduces O(n²) to O(n) while maintaining signal quality
            let n = symbol_prices.len();

            // Pre-calculate reference indices for efficient O(n) comparison
            let reference_indices: Vec<usize> = if n <= 10 {
                (0..n).collect()
            } else {
                vec![
                    0,         // min price
                    n / 4,     // 25th percentile
                    n / 2,     // median
                    3 * n / 4, // 75th percentile
                    n - 1,     // max price
                ]
            };

            // For each reference symbol, compare with sampled other symbols
            for &ref_idx in &reference_indices {
                if ref_idx >= n {
                    continue;
                }

                let symbol1 = symbol_prices[ref_idx].0;
                let price1 = symbol_prices[ref_idx].1;

                // Compare with a sample of other symbols (not all)
                // For each reference, only check a subset to maintain O(n)
                let sample_count = std::cmp::min(10, n.saturating_sub(1));
                let step = if n > sample_count {
                    (n as f64 / sample_count as f64).ceil() as usize
                } else {
                    1
                };

                for j in (0..n).step_by(step) {
                    if j == ref_idx {
                        continue;
                    }

                    let symbol2 = symbol_prices[j].0;
                    let price2 = symbol_prices[j].1;

                    // Skip if symbols have same base
                    if symbol1.base == symbol2.base {
                        continue;
                    }

                    if price2.is_zero() {
                        continue;
                    }

                    // Calculate current ratio
                    let current_ratio = price1 / price2;

                    // Use a simple heuristic: if ratio is very different from 1.0, it might be an opportunity
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
                        let position_value = self.config.max_exposure
                            / Decimal::from(DEFAULT_CONVERGENCE_POSITION_DIVISOR); // 25% of max exposure
                        let long_quantity = position_value / long_price;
                        let short_quantity = position_value / short_price;

                        // Create convergence signal
                        let mut signal = RawSignal::new(self.id(), (**long_symbol).clone().into());

                        // Long leg
                        let long_leg = TradeLeg::new(
                            exchange,
                            (**long_symbol).clone().into(),
                            Side::Buy,
                            long_price,
                            long_quantity,
                        );
                        signal.add_leg(long_leg);

                        // Short leg
                        let short_leg = TradeLeg::new(
                            exchange,
                            (**short_symbol).clone().into(),
                            Side::Sell,
                            short_price,
                            short_quantity,
                        );
                        signal.add_leg(short_leg);

                        // Estimate profit based on expected convergence
                        let expected_convergence_bps = (ratio_deviation
                            * Decimal::from(DEFAULT_RATIO_DEVIATION_MULTIPLIER))
                        .to_i32()
                        .unwrap_or(0)
                        .min(DEFAULT_CONVERGENCE_PROFIT_CAP_BPS); // Cap at 5%

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
        if signal.legs.len() < 2 {
            return Ok(false);
        }

        debug_assert!(
            signal.legs.len() >= 2,
            "Signal should have at least 2 legs for convergence arbitrage"
        );

        let long_leg = &signal.legs[0];
        let short_leg = &signal.legs[1];

        debug_assert_eq!(
            long_leg.exchange, short_leg.exchange,
            "Both legs should be on the same exchange for convergence arbitrage"
        );

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
        debug_assert!(
            !long_leg.price.is_zero() && !long_leg.quantity.is_zero(),
            "Long leg price and quantity should be non-zero"
        );
        debug_assert!(
            !short_leg.price.is_zero() && !short_leg.quantity.is_zero(),
            "Short leg price and quantity should be non-zero"
        );
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
