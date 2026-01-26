#![allow(clippy::type_complexity)]

use crate::constants::{BASIS_POINTS_DIVISOR, SLIPPIER_TIER_1_BPS, SLIPPIER_TIER_2_BPS};
use crate::strategies::strategies_specifics::{
    ExchangeCapabilities, StrategyLimits, StrategyUtils,
};
use crate::strategies::{
    FilterContext, FundingRate, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig,
    TradeLeg,
};
use crate::{ArbitrageError, ExchangeId, Result, Side, Symbol};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;
use tracing::debug;

/// Hedged Funding Strategy
///
/// **Principle**: Earn funding payments on perpetual contracts while maintaining
/// market-neutral exposure through sophisticated hedging across multiple exchanges
/// and instruments. Maximize funding income while minimizing directional risk.
///
/// **Focus**:
/// - Complex position management across multiple exchanges
/// - Dynamic hedge ratio optimization
/// - Cross-exchange basis risk management
/// - Multi-leg execution coordination
///
/// **Supported Exchanges**: OKX, Bybit, MEXC (perpetuals) + all exchanges (spot hedging)
///
/// **Data Sources**:
/// - Funding rates across multiple perpetual exchanges
/// - Spot prices for hedging calculations
/// - Cross-exchange basis spreads
/// - Volatility and correlation metrics
///
/// # Example
///
/// ```rust
/// use arbitrage_core::strategies::StrategyConfig;
///
/// let strategy = HedgedFundingStrategy::new();
/// assert_eq!(strategy.id(), "hedged_funding");
/// assert_eq!(strategy.name(), "Hedged Funding Strategy");
/// ```
pub struct HedgedFundingStrategy {
    config: StrategyConfig,
}

impl HedgedFundingStrategy {
    /// Creates a new hedged funding strategy with default configuration.
    ///
    /// Initializes the strategy with default limits and parameters for
    /// hedged funding arbitrage. The strategy is enabled by default.
    ///
    /// # Returns
    ///
    /// A new HedgedFundingStrategy instance with default configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// let strategy = HedgedFundingStrategy::new();
    /// assert!(strategy.config().enabled);
    /// ```
    pub fn new() -> Self {
        let custom_params = StrategyUtils::create_base_custom_params();

        Self {
            config: StrategyConfig {
                enabled: true,
                min_profit_bps: StrategyLimits::get_min_profit_bps("hedged_funding"),
                max_exposure: StrategyLimits::get_max_exposure("hedged_funding"),
                confidence_threshold: Decimal::new(7, 1),
                risk_limits: RiskLimits::default(),
                custom_params,
            },
        }
    }

    /// Creates a hedged funding strategy with custom configuration.
    ///
    /// Allows overriding the default configuration with custom parameters
    /// for fine-tuned control over strategy behavior.
    ///
    /// # Arguments
    ///
    /// * `config` - Custom strategy configuration
    ///
    /// # Returns
    ///
    /// A new HedgedFundingStrategy instance with the provided configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::strategies::{StrategyConfig, RiskLimits};
    /// use rust_decimal::Decimal;
    ///
    /// let config = StrategyConfig {
    ///     enabled: true,
    ///     min_profit_bps: 15,
    ///     max_exposure: Decimal::from(100000),
    ///     confidence_threshold: Decimal::new(8, 1),
    ///     risk_limits: RiskLimits::default(),
    ///     custom_params: serde_json::Map::new(),
    /// };
    ///
    /// let strategy = HedgedFundingStrategy::with_config(config);
    /// ```
    pub fn with_config(config: StrategyConfig) -> Self {
        let mut strategy = Self::new();
        strategy.config = config;
        strategy
    }

    /// Check if funding rate meets strategy criteria
    fn is_funding_rate_attractive(&self, funding_rate: &FundingRate) -> bool {
        let min_rate_pct = self
            .config
            .custom_params
            .get("min_funding_rate_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.01);

        let max_rate_pct = self
            .config
            .custom_params
            .get("max_funding_rate_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        let rate_pct = funding_rate.rate.abs().to_f64().unwrap_or(0.0);

        rate_pct >= min_rate_pct && rate_pct <= max_rate_pct
    }

    /// Check if there's sufficient time until funding
    fn has_sufficient_time_to_funding(&self, funding_rate: &FundingRate) -> bool {
        let min_time_hours = self
            .config
            .custom_params
            .get("min_time_to_funding_hours")
            .and_then(|v| v.as_f64())
            .unwrap_or(2.0);

        let time_to_funding =
            (funding_rate.next_funding - Utc::now()).num_seconds() as f64 / 3600.0;
        time_to_funding >= min_time_hours
    }
}

impl Default for HedgedFundingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for HedgedFundingStrategy {
    /// Returns the unique identifier for this strategy.
    ///
    /// # Returns
    ///
    /// A static string slice identifying the strategy type.
    fn id(&self) -> &'static str {
        "hedged_funding"
    }

    /// Returns the human-readable name for this strategy.
    ///
    /// # Returns
    ///
    /// A static string slice containing the strategy name.
    fn name(&self) -> &'static str {
        "Hedged Funding Strategy"
    }

    /// Detects arbitrage opportunities in the current market data.
    ///
    /// Scans all available funding rates across supported exchanges to identify
    /// hedged funding opportunities where the funding rate justifies the basis
    /// risk between perpetual and spot markets.
    ///
    /// # Arguments
    ///
    /// * `market_data` - Bundle containing market data from all exchanges
    ///
    /// # Returns
    ///
    /// A vector of detected arbitrage signals, empty if no opportunities found.
    ///
    /// # Errors
    ///
    /// Returns an error if market data processing fails.
    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        debug!("Hedged funding scanning market data");

        // For hedged funding, we'll use a simplified approach without maintaining state
        // In a real implementation, funding history and position tracking would be
        // managed by external services.

        let mut signals = Vec::new();

        for ((exchange, symbol), funding_rate) in &market_data.funding_rates {
            // Skip if exchange doesn't support funding rates
            if !ExchangeCapabilities::supports_funding_rates(*exchange) {
                continue;
            }

            // Check if funding rate is attractive
            if !self.is_funding_rate_attractive(funding_rate) {
                continue;
            }

            // Check timing
            if !self.has_sufficient_time_to_funding(funding_rate) {
                continue;
            }

            // Find optimal hedge exchange (simplified - just pick first available)
            let mut hedge_exchange = None;
            for ex in [
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::MEXC,
                ExchangeId::GateIo,
                ExchangeId::Kraken,
                ExchangeId::Bitstamp,
            ] {
                if ex != *exchange && market_data.get_ticker(ex, symbol).is_some() {
                    hedge_exchange = Some(ex);
                    break;
                }
            }

            let hedge_exchange = match hedge_exchange {
                Some(ex) => ex,
                None => continue,
            };

            // Get prices
            let perp_price = market_data
                .get_ticker(*exchange, symbol)
                .map(|t| (t.bid + t.ask) / Decimal::from(2));
            let spot_price = market_data
                .get_ticker(hedge_exchange, symbol)
                .map(|t| (t.bid + t.ask) / Decimal::from(2));

            let (perp_price, spot_price) = match (perp_price, spot_price) {
                (Some(p1), Some(p2)) => (p1, p2),
                _ => continue,
            };

            // Calculate position size (simplified)
            let position_value = self.config.max_exposure / Decimal::from(5); // 20% of max exposure
            let position_size = position_value / perp_price;

            if position_size <= Decimal::ZERO {
                continue;
            }

            // Determine position sides based on funding rate sign
            let (perp_side, spot_side) = if funding_rate.rate > Decimal::ZERO {
                // Positive funding: shorts pay longs
                // Go long perpetual (receive funding), short spot (hedge)
                (Side::Buy, Side::Sell)
            } else {
                // Negative funding: longs pay shorts
                // Go short perpetual (receive funding), long spot (hedge)
                (Side::Sell, Side::Buy)
            };

            // Calculate expected profit
            let funding_bps = (funding_rate.rate.abs() * Decimal::from(BASIS_POINTS_DIVISOR))
                .to_i32()
                .unwrap_or(0);

            // Subtract estimated costs (fees, basis risk)
            let estimated_costs_bps = SLIPPIER_TIER_2_BPS; // 0.15% estimated costs
            let net_profit_bps = funding_bps - estimated_costs_bps;

            if net_profit_bps < self.config.min_profit_bps {
                continue;
            }

            // Create hedged funding signal
            let mut signal = RawSignal::new(self.id(), symbol.clone());

            // Perpetual leg
            let perp_leg = TradeLeg::new(
                *exchange,
                symbol.clone(),
                perp_side,
                perp_price,
                position_size,
            );
            signal.add_leg(perp_leg);

            // Spot hedge leg
            let spot_leg = TradeLeg::new(
                hedge_exchange,
                symbol.clone(),
                spot_side,
                spot_price,
                position_size, // 1:1 hedge ratio for simplicity
            );
            signal.add_leg(spot_leg);

            signal.set_profit_bps(net_profit_bps);

            // Add metadata
            signal.add_metadata("funding_rate", json!(funding_rate.rate.to_string()));
            signal.add_metadata("perp_exchange", json!(exchange.to_string()));
            signal.add_metadata("hedge_exchange", json!(hedge_exchange.to_string()));
            signal.add_metadata(
                "time_to_funding_hours",
                json!((funding_rate.next_funding - Utc::now()).num_seconds() as f64 / 3600.0),
            );
            signal.add_metadata("estimated_costs_bps", json!(estimated_costs_bps));

            signals.push(signal);

            debug!(
                "Hedged funding opportunity: {} perp@{} hedge@{} rate={} profit={}bps",
                symbol, exchange, hedge_exchange, funding_rate.rate, net_profit_bps
            );
        }

        debug!("Hedged funding detected {} signals", signals.len());
        Ok(signals)
    }

    /// Filters a signal based on additional context and risk checks.
    ///
    /// Validates that a detected signal meets all requirements for execution,
    /// including exchange permissions, inventory constraints, and exposure limits.
    ///
    /// # Arguments
    ///
    /// * `signal` - The signal to filter
    /// * `context` - The filter context containing additional validation rules
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the signal passes all filters, `Ok(false)` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if validation encounters an unexpected error.
    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have exactly 2 legs (perpetual + spot hedge)
        if signal.legs.len() < 2 {
            return Ok(false);
        }

        let perp_leg = &signal.legs[0];
        let hedge_leg = &signal.legs[1];

        // Legs should be on different exchanges
        if perp_leg.exchange == hedge_leg.exchange {
            return Ok(false);
        }

        // Validate both exchanges are allowed
        if !context.are_exchanges_allowed(perp_leg.exchange, hedge_leg.exchange) {
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
        let base_asset = &signal.symbol.base;
        let quote_asset = &signal.symbol.quote;

        // Check perpetual leg inventory
        match perp_leg.side {
            Side::Buy => {
                let required_quote = perp_leg
                    .price
                    .checked_mul(perp_leg.quantity)
                    .unwrap_or(Decimal::ZERO);
                if !context.can_sell(perp_leg.exchange, quote_asset, required_quote) {
                    return Ok(false);
                }
            }
            Side::Sell => {
                if !context.can_sell(perp_leg.exchange, base_asset, perp_leg.quantity) {
                    return Ok(false);
                }
            }
        }

        // Check hedge leg inventory
        match hedge_leg.side {
            Side::Buy => {
                let required_quote = hedge_leg
                    .price
                    .checked_mul(hedge_leg.quantity)
                    .unwrap_or(Decimal::ZERO);
                if !context.can_sell(hedge_leg.exchange, quote_asset, required_quote) {
                    return Ok(false);
                }
            }
            Side::Sell => {
                if !context.can_sell(hedge_leg.exchange, base_asset, hedge_leg.quantity) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Returns a reference to the strategy configuration.
    ///
    /// # Returns
    ///
    /// Immutable reference to the current strategy configuration.
    fn config(&self) -> &StrategyConfig {
        &self.config
    }

    /// Updates the strategy configuration.
    ///
    /// Allows runtime modification of strategy parameters such as
    /// min profit threshold, max exposure, and risk limits.
    ///
    /// # Arguments
    ///
    /// * `config` - The new strategy configuration
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful update.
    fn update_config(&mut self, config: StrategyConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }
}
