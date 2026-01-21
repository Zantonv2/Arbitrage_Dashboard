#![allow(clippy::type_complexity)]

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
pub struct HedgedFundingStrategy {
    config: StrategyConfig,
    #[allow(dead_code)]
    /// Track funding rate history for prediction
    funding_history: HashMap<(ExchangeId, Symbol), Vec<(DateTime<Utc>, Decimal)>>,
    #[allow(dead_code)]
    /// Track hedge effectiveness metrics
    hedge_ratios: HashMap<Symbol, Decimal>,
    #[allow(dead_code)]
    /// Position tracking for risk management
    current_positions: HashMap<(ExchangeId, Symbol), Decimal>,
}

impl HedgedFundingStrategy {
    /// Create a new hedged funding strategy with default configuration
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
            funding_history: HashMap::new(),
            hedge_ratios: HashMap::new(),
            current_positions: HashMap::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        let mut strategy = Self::new();
        strategy.config = config;
        strategy
    }

    #[allow(dead_code)]
    /// Update funding rate history for prediction
    fn update_funding_history(&mut self, market_data: &MarketBundle) {
        let current_time = market_data.timestamp;

        for ((exchange, symbol), funding_rate) in &market_data.funding_rates {
            let key = (*exchange, (**symbol).clone());
            let history = self.funding_history.entry(key).or_default();

            // Keep only recent history (last 7 days)
            let cutoff = current_time - Duration::days(7);
            history.retain(|(ts, _)| *ts > cutoff);

            // Add new funding rate
            history.push((current_time, funding_rate.rate));

            // Keep sorted by timestamp
            history.sort_by_key(|(ts, _)| *ts);
        }
    }

    #[allow(dead_code)]
    /// Predict next funding rate based on history
    fn predict_funding_rate(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<Decimal> {
        let key = (exchange, symbol.clone());
        let history = self.funding_history.get(&key)?;

        if history.len() < 3 {
            return None;
        }

        // Simple moving average of last 3 funding rates
        let recent_rates: Vec<Decimal> = history
            .iter()
            .rev()
            .take(3)
            .map(|(_, rate)| *rate)
            .collect();

        let avg_rate = recent_rates.iter().sum::<Decimal>() / Decimal::from(recent_rates.len());

        // Apply prediction weight
        let prediction_weight = self
            .config
            .custom_params
            .get("funding_prediction_weight")
            .and_then(|v| v.as_f64())
            .and_then(|f| Decimal::try_from(f).ok())
            .unwrap_or_else(|| Decimal::new(3, 1)); // 0.3

        let current_rate = recent_rates.first().copied().unwrap_or(Decimal::ZERO);
        let predicted_rate =
            current_rate * (Decimal::ONE - prediction_weight) + avg_rate * prediction_weight;

        Some(predicted_rate)
    }

    #[allow(dead_code)]
    /// Calculate optimal hedge ratio for a symbol
    fn calculate_hedge_ratio(&mut self, symbol: &Symbol, _market_data: &MarketBundle) -> Decimal {
        // Check if we have a cached ratio
        if let Some(cached_ratio) = self.hedge_ratios.get(symbol) {
            return *cached_ratio;
        }

        // For basic implementation, use 1:1 hedge ratio
        // Advanced implementation would calculate based on:
        // - Beta between perpetual and spot
        // - Volatility ratios
        // - Correlation coefficients
        let target_ratio = self
            .config
            .custom_params
            .get("hedge_ratio_target")
            .and_then(|v| v.as_f64())
            .and_then(|f| Decimal::try_from(f).ok())
            .unwrap_or(Decimal::ONE);

        // Cache the ratio
        self.hedge_ratios.insert(symbol.clone(), target_ratio);

        target_ratio
    }

    #[allow(dead_code)]
    /// Calculate basis risk between perpetual and spot
    fn calculate_basis_risk(
        &self,
        market_data: &MarketBundle,
        perp_exchange: ExchangeId,
        spot_exchange: ExchangeId,
        symbol: &Symbol,
    ) -> Option<Decimal> {
        // Get perpetual price (use ticker mid price as approximation)
        let perp_price = market_data
            .get_ticker(perp_exchange, symbol)
            .map(|t| (t.bid + t.ask) / Decimal::from(2))?;

        // Get spot price
        let spot_price = market_data
            .get_ticker(spot_exchange, symbol)
            .map(|t| (t.bid + t.ask) / Decimal::from(2))?;

        if spot_price.is_zero() {
            return None;
        }

        // Calculate basis (perpetual - spot) / spot
        let basis = (perp_price - spot_price) / spot_price;
        Some(basis.abs())
    }

    #[allow(dead_code)]
    /// Find optimal hedge exchange for a perpetual position
    fn find_optimal_hedge_exchange(
        &self,
        market_data: &MarketBundle,
        perp_exchange: ExchangeId,
        symbol: &Symbol,
    ) -> Option<ExchangeId> {
        let mut best_exchange = None;
        let mut lowest_basis_risk = Decimal::from(1); // 100%

        // Check all exchanges for spot hedging
        for exchange in [
            ExchangeId::OKX,
            ExchangeId::ByBit,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
            ExchangeId::Kraken,
            ExchangeId::Bitstamp,
        ] {
            if exchange == perp_exchange {
                continue; // Don't hedge on same exchange
            }

            if let Some(basis_risk) =
                self.calculate_basis_risk(market_data, perp_exchange, exchange, symbol)
            {
                if basis_risk < lowest_basis_risk {
                    lowest_basis_risk = basis_risk;
                    best_exchange = Some(exchange);
                }
            }
        }

        // Check if basis risk is acceptable
        let max_basis_risk_bps = self
            .config
            .custom_params
            .get("max_basis_risk_bps")
            .and_then(|v| v.as_i64())
            .unwrap_or(50) as i32;
        let max_basis_risk = Decimal::from(max_basis_risk_bps) / Decimal::from(10000);

        if lowest_basis_risk <= max_basis_risk {
            best_exchange
        } else {
            None
        }
    }

    #[allow(dead_code)]
    /// Calculate position size based on risk limits
    fn calculate_position_size(
        &self,
        funding_rate: &FundingRate,
        market_data: &MarketBundle,
    ) -> Result<Decimal> {
        // Get current price for notional calculation
        let current_price = market_data
            .get_ticker(funding_rate.exchange, &funding_rate.symbol)
            .map(|t| (t.bid + t.ask) / Decimal::from(2))
            .ok_or_else(|| {
                ArbitrageError::Validation("No price data for funding rate symbol".to_string())
            })?;

        // Calculate maximum position based on concentration limits
        let max_concentration = self
            .config
            .custom_params
            .get("max_position_concentration")
            .and_then(|v| v.as_f64())
            .and_then(|f| Decimal::try_from(f).ok())
            .unwrap_or_else(|| Decimal::new(4, 1)); // 0.4

        let max_position_value = self.config.max_exposure * max_concentration;
        let max_quantity = max_position_value / current_price;

        // Consider current positions for risk management
        let current_exposure = self.get_current_exposure(&funding_rate.symbol);
        let available_capacity = max_position_value - current_exposure;

        if available_capacity <= Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        let available_quantity = available_capacity / current_price;
        Ok(max_quantity.min(available_quantity))
    }

    #[allow(dead_code)]
    /// Get current exposure for a symbol across all exchanges
    fn get_current_exposure(&self, symbol: &Symbol) -> Decimal {
        self.current_positions
            .iter()
            .filter(|((_, sym), _)| sym == symbol)
            .map(|(_, position)| position.abs())
            .sum()
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

    #[allow(dead_code)]
    /// Find hedged funding opportunities
    fn find_hedged_funding_opportunities(&mut self, market_data: &MarketBundle) -> Vec<RawSignal> {
        let mut signals = Vec::new();

        // Update funding history
        self.update_funding_history(market_data);

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

            // Find optimal hedge exchange
            let hedge_exchange =
                match self.find_optimal_hedge_exchange(market_data, *exchange, symbol) {
                    Some(ex) => ex,
                    None => continue,
                };

            // Calculate position size
            let position_size = match self.calculate_position_size(funding_rate, market_data) {
                Ok(size) => size,
                Err(_) => continue,
            };

            if position_size <= Decimal::ZERO {
                continue;
            }

            // Calculate hedge ratio
            let hedge_ratio = self.calculate_hedge_ratio(symbol, market_data);
            let hedge_size = position_size * hedge_ratio;

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
            let funding_bps = (funding_rate.rate.abs() * Decimal::from(10000))
                .to_i32()
                .unwrap_or(0);

            // Subtract estimated costs (fees, basis risk)
            let estimated_costs_bps = 10; // 0.1% estimated costs
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
                hedge_size,
            );
            signal.add_leg(spot_leg);

            signal.set_profit_bps(net_profit_bps);

            // Add metadata
            signal.add_metadata("funding_rate", json!(funding_rate.rate.to_string()));
            signal.add_metadata("perp_exchange", json!(exchange.to_string()));
            signal.add_metadata("hedge_exchange", json!(hedge_exchange.to_string()));
            signal.add_metadata("hedge_ratio", json!(hedge_ratio.to_string()));
            signal.add_metadata(
                "time_to_funding_hours",
                json!((funding_rate.next_funding - Utc::now()).num_seconds() as f64 / 3600.0),
            );
            signal.add_metadata(
                "predicted_funding",
                json!(self
                    .predict_funding_rate(*exchange, symbol)
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "N/A".to_string())),
            );
            signal.add_metadata(
                "basis_risk_bps",
                json!(self
                    .calculate_basis_risk(market_data, *exchange, hedge_exchange, symbol)
                    .map(|r| (r * Decimal::from(10000)).to_i32().unwrap_or(0))
                    .unwrap_or(0)),
            );

            signals.push(signal);

            debug!(
                "Hedged funding opportunity: {} perp@{} hedge@{} rate={} profit={}bps",
                symbol, exchange, hedge_exchange, funding_rate.rate, net_profit_bps
            );
        }

        signals
    }
}

impl Default for HedgedFundingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for HedgedFundingStrategy {
    fn id(&self) -> &'static str {
        "hedged_funding"
    }

    fn name(&self) -> &'static str {
        "Hedged Funding Strategy"
    }

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
            let funding_bps = (funding_rate.rate.abs() * Decimal::from(10000))
                .to_i32()
                .unwrap_or(0);

            // Subtract estimated costs (fees, basis risk)
            let estimated_costs_bps = 15; // 0.15% estimated costs
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

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have exactly 2 legs (perpetual + spot hedge)
        if signal.legs.len() != 2 {
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

    fn config(&self) -> &StrategyConfig {
        &self.config
    }

    fn update_config(&mut self, config: StrategyConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }
}
