use crate::strategies::strategies_specifics::{
    ExchangeCapabilities, FundingRateDefaults, StrategyLimits,
};
use crate::strategies::{
    FilterContext, FundingRate, MarketBundle, RawSignal, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ArbitrageError, ExchangeId, Result, Side, Symbol};
use chrono::{Duration, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, warn};

/// Funding Rate Arbitrage Strategy
///
/// **Principle**: Earn periodic funding payments by holding the side of perpetual
/// contracts that receives funding while hedging with spot positions.
///
/// **Focus**:
/// - Monitor funding rates across perpetual exchanges
/// - Calculate time-weighted returns from funding payments
/// - Hedge spot positions to minimize price risk
/// - Account for position holding costs and margin requirements
///
/// **Supported Exchanges**: OKX, Bybit, MEXC (perpetual contracts only)
///
/// **Data Sources**:
/// - Funding rates from perpetual contract exchanges
/// - Spot prices for hedging calculations
/// - Time to next funding for opportunity timing
pub struct FundingRateArbitrageStrategy {
    config: StrategyConfig,
}

impl FundingRateArbitrageStrategy {
    /// Create a new funding rate arbitrage strategy with default configuration
    pub fn new() -> Self {
        Self {
            config: StrategyConfig {
                min_profit_bps: StrategyLimits::get_min_profit_bps("funding_rate_arbitrage"),
                max_exposure: StrategyLimits::get_max_exposure("funding_rate_arbitrage"),
                custom_params: FundingRateDefaults::get_custom_params(),
                ..Default::default()
            },
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        Self { config }
    }

    #[allow(dead_code)]
    /// Get supported exchanges for funding rate arbitrage
    fn get_supported_exchanges(&self) -> Vec<ExchangeId> {
        ExchangeCapabilities::get_funding_rate_exchanges()
    }

    /// Calculate annualized funding rate return
    fn calculate_annualized_return(&self, funding_rate: &FundingRate) -> Result<Decimal> {
        // Funding typically occurs every 8 hours (3 times per day)
        let daily_rate = funding_rate
            .rate
            .checked_mul(Decimal::from(3))
            .ok_or_else(|| {
                ArbitrageError::Calculation("Daily funding rate overflow".to_string())
            })?;

        let annual_rate = daily_rate
            .checked_mul(Decimal::from(365))
            .ok_or_else(|| ArbitrageError::Calculation("Annual rate overflow".to_string()))?;

        Ok(annual_rate)
    }

    /// Calculate time to next funding in hours
    fn time_to_funding_hours(&self, funding_rate: &FundingRate) -> Result<Decimal> {
        let now = Utc::now();
        let duration = funding_rate.next_funding - now;

        if duration < Duration::zero() {
            return Err(ArbitrageError::Validation(
                "Funding time is in the past".to_string(),
            ));
        }

        let hours = duration.num_seconds() as f64 / 3600.0;
        Decimal::try_from(hours)
            .map_err(|e| ArbitrageError::Calculation(format!("Time conversion error: {}", e)))
    }

    /// Calculate expected profit from funding rate
    fn calculate_funding_profit_bps(
        &self,
        funding_rate: &FundingRate,
        _time_to_funding: Decimal,
    ) -> Result<i32> {
        // For funding rate arbitrage, we earn the full funding rate for each funding period
        // The time_to_funding is just used to check if we have enough time to execute

        // Convert funding rate directly to basis points
        let funding_bps = funding_rate
            .rate
            .checked_mul(Decimal::from(10000))
            .ok_or_else(|| {
                ArbitrageError::Calculation("Funding rate BPS conversion overflow".to_string())
            })?
            .to_i32()
            .ok_or_else(|| {
                ArbitrageError::Calculation("Funding BPS conversion failed".to_string())
            })?;

        Ok(funding_bps.abs())
    }

    /// Check if funding rate meets minimum thresholds
    fn is_funding_rate_valid(&self, funding_rate: &FundingRate) -> bool {
        let min_rate_pct = self
            .config
            .custom_params
            .get("min_funding_rate_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(FundingRateDefaults::MIN_FUNDING_RATE_PCT);

        let max_rate_pct = self
            .config
            .custom_params
            .get("max_funding_rate_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(FundingRateDefaults::MAX_FUNDING_RATE_PCT);

        let rate_pct = funding_rate
            .rate
            .checked_mul(Decimal::from(100))
            .unwrap_or(Decimal::ZERO)
            .to_f64()
            .unwrap_or(0.0);

        rate_pct.abs() >= min_rate_pct && rate_pct.abs() <= max_rate_pct
    }

    /// Check if there's sufficient time until next funding
    fn has_sufficient_time(&self, time_to_funding: Decimal) -> bool {
        let min_time_hours = self
            .config
            .custom_params
            .get("min_time_to_funding_hours")
            .and_then(|v| v.as_f64())
            .unwrap_or(FundingRateDefaults::MIN_TIME_TO_FUNDING_HOURS);

        time_to_funding.to_f64().unwrap_or(0.0) >= min_time_hours
    }

    /// Get spot price for hedging calculation
    fn get_spot_price(
        &self,
        market_data: &MarketBundle,
        symbol: &Symbol,
        exchange: ExchangeId,
    ) -> Option<Decimal> {
        market_data
            .get_ticker(exchange, symbol)
            .map(|ticker| (ticker.bid + ticker.ask) / Decimal::from(2))
            .filter(|price| *price > Decimal::ZERO)
    }

    /// Calculate hedge ratio (typically 1:1 for neutral strategy)
    fn calculate_hedge_ratio(&self) -> Decimal {
        // For basic funding arbitrage, use 1:1 hedge ratio
        // Advanced implementations could use beta-adjusted ratios
        Decimal::ONE
    }

    /// Get maximum position size for funding arbitrage
    fn get_max_position_size(&self) -> Decimal {
        let max_position_usd = self
            .config
            .custom_params
            .get("max_funding_position_usd")
            .and_then(|v| v.as_f64())
            .unwrap_or(FundingRateDefaults::MAX_FUNDING_POSITION_USD);

        Decimal::try_from(max_position_usd).unwrap_or_else(|_| Decimal::from(50000))
    }
}

impl Default for FundingRateArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for FundingRateArbitrageStrategy {
    fn id(&self) -> &'static str {
        "funding_rate_arbitrage"
    }

    fn name(&self) -> &'static str {
        "Funding Rate Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();

        debug!(
            "Funding rate arbitrage scanning {} funding rates",
            market_data.funding_rates.len()
        );

        for ((exchange, symbol), funding_rate) in &market_data.funding_rates {
            // Skip if exchange doesn't support funding rates
            if !ExchangeCapabilities::supports_funding_rates(*exchange) {
                continue;
            }

            // Validate funding rate
            if !self.is_funding_rate_valid(funding_rate) {
                debug!(
                    "Funding rate invalid for {} on {}: {}",
                    symbol, exchange, funding_rate.rate
                );
                continue;
            }

            // Calculate time to next funding
            let time_to_funding = match self.time_to_funding_hours(funding_rate) {
                Ok(time) => time,
                Err(e) => {
                    warn!(
                        "Failed to calculate funding time for {} on {}: {}",
                        symbol, exchange, e
                    );
                    continue;
                }
            };

            // Check if there's sufficient time
            if !self.has_sufficient_time(time_to_funding) {
                debug!(
                    "Insufficient time to funding for {} on {}: {} hours",
                    symbol, exchange, time_to_funding
                );
                continue;
            }

            // Calculate expected profit
            let profit_bps = match self.calculate_funding_profit_bps(funding_rate, time_to_funding)
            {
                Ok(profit) => profit,
                Err(e) => {
                    warn!(
                        "Failed to calculate funding profit for {} on {}: {}",
                        symbol, exchange, e
                    );
                    continue;
                }
            };

            // Skip if profit too low
            if profit_bps.abs() < self.config.min_profit_bps {
                debug!(
                    "Funding profit too low for {} on {}: {}bps < {}bps",
                    symbol, exchange, profit_bps, self.config.min_profit_bps
                );
                continue;
            }

            // Get spot price for hedging
            let spot_price = match self.get_spot_price(market_data, symbol, *exchange) {
                Some(price) => price,
                None => {
                    debug!("No spot price available for {} on {}", symbol, exchange);
                    continue;
                }
            };

            // Calculate position size
            let max_position_usd = self.get_max_position_size();
            let position_quantity = max_position_usd
                .checked_div(spot_price)
                .unwrap_or(Decimal::ZERO);

            if position_quantity <= Decimal::ZERO {
                continue;
            }

            // Create signal
            let mut signal = RawSignal::new(self.id(), (**symbol).clone().into());

            // Determine position side based on funding rate sign
            let (perp_side, spot_side) = if funding_rate.rate > Decimal::ZERO {
                // Positive funding: shorts pay longs
                // Go long perpetual (receive funding), short spot (hedge)
                (Side::Buy, Side::Sell)
            } else {
                // Negative funding: longs pay shorts
                // Go short perpetual (receive funding), long spot (hedge)
                (Side::Sell, Side::Buy)
            };

            // Add perpetual leg
            let perp_leg = TradeLeg::new(
                *exchange,
                Arc::new((**symbol).clone()),
                perp_side,
                spot_price, // Use spot price as approximation for perp price
                position_quantity,
            );
            signal.add_leg(perp_leg);

            // Add spot hedge leg (use same exchange for simplicity)
            let hedge_ratio = self.calculate_hedge_ratio();
            let hedge_quantity = position_quantity
                .checked_mul(hedge_ratio)
                .unwrap_or(position_quantity);

            let spot_leg = TradeLeg::new(
                *exchange,
                Arc::new((**symbol).clone()),
                spot_side,
                spot_price,
                hedge_quantity,
            );
            signal.add_leg(spot_leg);

            // Set profit (use absolute value since we handle both positive and negative funding)
            signal.set_profit_bps(profit_bps.abs());

            // Add metadata
            signal.add_metadata("funding_rate", json!(funding_rate.rate.to_string()));
            signal.add_metadata("time_to_funding_hours", json!(time_to_funding.to_string()));
            signal.add_metadata(
                "annualized_return",
                json!(self.calculate_annualized_return(funding_rate)?.to_string()),
            );
            signal.add_metadata(
                "next_funding",
                json!(funding_rate.next_funding.to_rfc3339()),
            );
            signal.add_metadata("position_side", json!(perp_side.to_string()));
            signal.add_metadata("hedge_ratio", json!(hedge_ratio.to_string()));

            signals.push(signal);

            debug!(
                "Funding rate signal: {} {} rate={} profit={}bps time={}h",
                symbol, exchange, funding_rate.rate, profit_bps, time_to_funding
            );
        }

        debug!("Funding rate arbitrage detected {} signals", signals.len());
        Ok(signals)
    }

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        println!(
            "Filtering signal: strategy_id={}, legs={}, profit_bps={}",
            signal.strategy_id,
            signal.legs.len(),
            signal.expected_profit_bps
        );

        // Basic validation
        if !signal.is_valid() {
            println!("Signal failed is_valid() check");
            return Ok(false);
        }

        // Must have exactly 2 legs (perpetual + spot hedge)
        if signal.legs.len() < 2 {
            return Ok(false);
        }

        let perp_leg = &signal.legs[0];
        let spot_leg = &signal.legs[1];

        println!(
            "Perp leg: {} {:?} @ {}, Spot leg: {} {:?} @ {}",
            perp_leg.exchange,
            perp_leg.side,
            perp_leg.price,
            spot_leg.exchange,
            spot_leg.side,
            spot_leg.price
        );

        // Validate exchange is allowed
        if !context.is_exchange_allowed(perp_leg.exchange) {
            println!("Exchange {} not allowed", perp_leg.exchange);
            return Ok(false);
        }

        // Check profit threshold
        if signal.expected_profit_bps < context.min_profit_bps {
            println!(
                "Profit too low: {} < {}",
                signal.expected_profit_bps, context.min_profit_bps
            );
            return Ok(false);
        }

        // Check maximum exposure
        let total_notional = signal.total_notional();
        if total_notional > context.max_exposure {
            println!(
                "Exposure too high: {} > {}",
                total_notional, context.max_exposure
            );
            return Ok(false);
        }

        // Check if we have sufficient inventory for both legs
        let base_asset = &signal.symbol.base;
        let quote_asset = &signal.symbol.quote;

        println!(
            "Checking inventory for base: {}, quote: {}",
            base_asset, quote_asset
        );

        // Check spot leg inventory requirements
        match spot_leg.side {
            Side::Sell => {
                let available = context.get_inventory(spot_leg.exchange, base_asset);
                println!(
                    "Spot leg sell: need {} {}, have {}",
                    spot_leg.quantity, base_asset, available
                );
                if !context.can_sell(spot_leg.exchange, base_asset, spot_leg.quantity) {
                    println!(
                        "Cannot sell {} {} on {}",
                        spot_leg.quantity, base_asset, spot_leg.exchange
                    );
                    return Ok(false);
                }
            }
            Side::Buy => {
                let required_quote = spot_leg
                    .price
                    .checked_mul(spot_leg.quantity)
                    .unwrap_or(Decimal::ZERO);
                let available = context.get_inventory(spot_leg.exchange, quote_asset);
                println!(
                    "Spot leg buy: need {} {}, have {}",
                    required_quote, quote_asset, available
                );
                if !context.can_sell(spot_leg.exchange, quote_asset, required_quote) {
                    println!(
                        "Cannot buy with {} {} on {}",
                        required_quote, quote_asset, spot_leg.exchange
                    );
                    return Ok(false);
                }
            }
        }

        // Check minimum notional
        if total_notional < context.min_notional_usd {
            println!(
                "Notional too low: {} < {}",
                total_notional, context.min_notional_usd
            );
            return Ok(false);
        }

        println!("All checks passed!");
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
