use crate::market_utils;
use crate::strategies::strategies_specifics::{
    ExchangeCapabilities, StrategyLimits, StrategyUtils,
};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ArbitrageError, ExchangeId, Result, Side, Symbol};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, warn};

/// Cross-Exchange Arbitrage Strategy
///
/// **Principle**: Execute instant arbitrage trades by keeping balances on multiple
/// exchanges. Similar to CEX arbitrage but with pre-positioned capital optimization.
///
/// **Focus**:
/// - Maintain optimal balance distribution across exchanges
/// - Execute arbitrage without transfer delays
/// - Account for rebalancing costs and timing
/// - Optimize capital efficiency across exchange pairs
///
/// **Supported Exchanges**: All 6 exchanges (OKX, Bybit, MEXC, Gate.io, Bitstamp, Kraken)
///
/// **Data Sources**:
/// - Order books from all exchanges
/// - Real-time balance tracking
/// - Transfer cost and time estimates
/// - Exchange-specific fee schedules
#[derive(Debug, Clone)]
pub struct CrossExchangeArbitrageStrategy {
    config: StrategyConfig,
}

impl CrossExchangeArbitrageStrategy {
    /// Create a new cross-exchange arbitrage strategy with default configuration
    pub fn new() -> Self {
        let custom_params = StrategyUtils::create_base_custom_params();

        Self {
            config: StrategyConfig {
                enabled: true,
                min_profit_bps: StrategyLimits::get_min_profit_bps("cex_arbitrage"),
                max_exposure: StrategyLimits::get_max_exposure("cex_arbitrage"),
                confidence_threshold: Decimal::new(7, 1),
                risk_limits: RiskLimits::default(),
                custom_params,
            },
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        Self { config }
    }

    /// Get supported exchanges for cross-exchange arbitrage
    fn get_supported_exchanges(&self) -> Vec<ExchangeId> {
        ExchangeCapabilities::get_cex_arbitrage_exchanges()
    }



    /// Calculate gross profit percentage before fees
    fn calculate_gross_profit_bps(&self, buy_price: Decimal, sell_price: Decimal) -> Result<i32> {
        if buy_price.is_zero() {
            return Err(ArbitrageError::Calculation(
                "Buy price cannot be zero".to_string(),
            ));
        }

        let profit_ratio = (sell_price - buy_price)
            .checked_div(buy_price)
            .ok_or_else(|| {
                ArbitrageError::Calculation("Division by zero in profit calculation".to_string())
            })?;

        let profit_bps = profit_ratio
            .checked_mul(Decimal::from(10000))
            .ok_or_else(|| ArbitrageError::Calculation("Profit calculation overflow".to_string()))?
            .to_i32()
            .ok_or_else(|| {
                ArbitrageError::Calculation("Profit BPS conversion failed".to_string())
            })?;

        Ok(profit_bps)
    }

    /// Estimate net profit after fees and transfer costs
    ///
    /// Calculates net profit by subtracting combined taker fees and optional
    /// transfer costs from gross profit. Uses checked arithmetic to prevent
    /// overflow for unusual fee values.
    ///
    /// # Arguments
    ///
    /// * `gross_profit_bps` - Gross profit in basis points
    /// * `buy_exchange` - Exchange for buy leg
    /// * `sell_exchange` - Exchange for sell leg
    /// * `needs_rebalancing` - Whether transfer costs should be deducted
    ///
    /// # Returns
    ///
    /// Net profit in basis points after fees, or an error if calculation overflows.
    pub(crate) fn estimate_net_profit_bps(
        &self,
        gross_profit_bps: i32,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
        needs_rebalancing: bool,
    ) -> Result<i32> {
        let (_, buy_taker_fee) = ExchangeCapabilities::get_typical_fees(buy_exchange);
        let (_, sell_taker_fee) = ExchangeCapabilities::get_typical_fees(sell_exchange);

        let combined_fee_pct = buy_taker_fee + sell_taker_fee;

        if combined_fee_pct > 1.0 {
            return Err(ArbitrageError::Calculation(format!(
                "Combined fee rate {} exceeds 100%, cannot calculate fee BPS",
                combined_fee_pct
            )));
        }

        let total_fee_bps = (combined_fee_pct * 100.0).floor() as i32;

        let mut net_profit = gross_profit_bps.checked_sub(total_fee_bps).ok_or_else(|| {
            ArbitrageError::Calculation(format!(
                "Net profit calculation underflow: {} - {}",
                gross_profit_bps, total_fee_bps
            ))
        })?;

        if net_profit < 0 {
            return Err(ArbitrageError::Calculation(format!(
                "Net profit would be negative: {} - {} = {}",
                gross_profit_bps, total_fee_bps, net_profit
            )));
        }

        if needs_rebalancing {
            let transfer_cost_bps = self
                .config
                .custom_params
                .get("transfer_cost_bps")
                .and_then(|v| v.as_i64())
                .unwrap_or(10) as i32;
            net_profit = net_profit.checked_sub(transfer_cost_bps).ok_or_else(|| {
                ArbitrageError::Calculation(format!(
                    "Net profit underflow after transfer costs: {} - {}",
                    net_profit, transfer_cost_bps
                ))
            })?;

            if net_profit < 0 {
                return Err(ArbitrageError::Calculation(format!(
                    "Net profit would be negative after transfer costs: {} - {} = {}",
                    net_profit + transfer_cost_bps,
                    transfer_cost_bps,
                    net_profit
                )));
            }
        }

        Ok(net_profit)
    }

    /// Check if exchanges need rebalancing based on current balances
    fn needs_rebalancing(
        &self,
        context: &FilterContext,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
        symbol: &Symbol,
        quantity: Decimal,
        price: Decimal,
    ) -> bool {
        let base_asset = &symbol.base;
        let quote_asset = &symbol.quote;

        // Check if we have sufficient balance for sell side
        let available_base = context.get_inventory(sell_exchange, base_asset);
        if available_base < quantity {
            return true;
        }

        // Check if we have sufficient balance for buy side
        let required_quote = quantity.checked_mul(price).unwrap_or(Decimal::ZERO);
        let available_quote = context.get_inventory(buy_exchange, quote_asset);
        if available_quote < required_quote {
            return true;
        }

        // Check balance ratios
        let min_balance_ratio = self
            .config
            .custom_params
            .get("min_balance_ratio")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.1);

        let total_base = self.get_total_balance(context, base_asset);
        let total_quote = self.get_total_balance(context, quote_asset);

        if total_base > Decimal::ZERO {
            let sell_ratio = available_base
                .checked_div(total_base)
                .unwrap_or(Decimal::ZERO);
            if sell_ratio.to_f64().unwrap_or(0.0) < min_balance_ratio {
                return true;
            }
        }

        if total_quote > Decimal::ZERO {
            let buy_ratio = available_quote
                .checked_div(total_quote)
                .unwrap_or(Decimal::ZERO);
            if buy_ratio.to_f64().unwrap_or(0.0) < min_balance_ratio {
                return true;
            }
        }

        false
    }

    /// Get total balance across all exchanges for an asset
    fn get_total_balance(&self, context: &FilterContext, asset: &str) -> Decimal {
        let mut total = Decimal::ZERO;

        for exchange in self.get_supported_exchanges() {
            total += context.get_inventory(exchange, asset);
        }

        total
    }

    /// Calculate optimal position size considering balance constraints
    fn calculate_optimal_position_size(
        &self,
        context: &FilterContext,
        symbol: &Symbol,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
        buy_price: Decimal,
        available_liquidity: Decimal,
    ) -> Decimal {
        let base_asset = &symbol.base;
        let quote_asset = &symbol.quote;

        // Get available balances
        let available_base = context.get_inventory(sell_exchange, base_asset);
        let available_quote = context.get_inventory(buy_exchange, quote_asset);

        // Calculate maximum quantity based on balances
        let max_by_base = available_base;
        let max_by_quote = available_quote
            .checked_div(buy_price)
            .unwrap_or(Decimal::ZERO);
        let max_by_exposure = self
            .config
            .max_exposure
            .checked_div(buy_price)
            .unwrap_or(Decimal::ZERO);

        // Take minimum of all constraints
        available_liquidity
            .min(max_by_base)
            .min(max_by_quote)
            .min(max_by_exposure)
    }

    /// Check if balance distribution is optimal
    fn is_balance_distribution_optimal(&self, context: &FilterContext, asset: &str) -> bool {
        let max_imbalance_pct = self
            .config
            .custom_params
            .get("max_imbalance_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.3);

        let total_balance = self.get_total_balance(context, asset);
        if total_balance.is_zero() {
            return true; // No balance to distribute
        }

        let num_exchanges = self.get_supported_exchanges().len() as f64;
        let ideal_balance_per_exchange = total_balance
            .checked_div(Decimal::try_from(num_exchanges).unwrap_or(Decimal::ONE))
            .unwrap_or(Decimal::ZERO);

        for exchange in self.get_supported_exchanges() {
            let balance = context.get_inventory(exchange, asset);
            let deviation = (balance - ideal_balance_per_exchange).abs();
            let deviation_ratio = deviation
                .checked_div(total_balance)
                .unwrap_or(Decimal::ZERO);

            if deviation_ratio.to_f64().unwrap_or(0.0) > max_imbalance_pct {
                return false;
            }
        }

        true
    }
}

impl Default for CrossExchangeArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for CrossExchangeArbitrageStrategy {
    fn id(&self) -> &'static str {
        "cross_exchange_arbitrage"
    }

    fn name(&self) -> &'static str {
        "Cross-Exchange Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();

        let symbols = market_data.get_all_symbols();
        debug!(
            "Cross-exchange arbitrage scanning {} symbols",
            symbols.len()
        );

        for symbol in symbols {
            // Find best bid and ask across all exchanges using market_utils
            let best_bid = match market_utils::find_best_bid(market_data, &symbol, &self.get_supported_exchanges()) {
                Some(bid) => bid,
                None => {
                    debug!("No bids found for {}", symbol);
                    continue;
                }
            };

            let best_ask = match market_utils::find_best_ask(market_data, &symbol, &self.get_supported_exchanges()) {
                Some(ask) => ask,
                None => {
                    debug!("No asks found for {}", symbol);
                    continue;
                }
            };

            let (sell_exchange, sell_price) = best_bid;
            let (buy_exchange, buy_price) = best_ask;

            // Get quantities from order books
            let sell_quantity = market_data
                .get_order_book(sell_exchange, &symbol)
                .and_then(|ob| ob.best_bid())
                .map(|level| level.quantity)
                .unwrap_or(Decimal::ZERO);
            
            let buy_quantity = market_data
                .get_order_book(buy_exchange, &symbol)
                .and_then(|ob| ob.best_ask())
                .map(|level| level.quantity)
                .unwrap_or(Decimal::ZERO);

            // Skip if same exchange
            if buy_exchange == sell_exchange {
                continue;
            }

            // Calculate gross profit
            let gross_profit_bps = match self.calculate_gross_profit_bps(buy_price, sell_price) {
                Ok(profit) => profit,
                Err(e) => {
                    warn!("Failed to calculate profit for {}: {}", symbol, e);
                    continue;
                }
            };

            // Skip if no profit
            if gross_profit_bps <= 0 {
                continue;
            }

            // Determine available liquidity
            let available_liquidity = sell_quantity.min(buy_quantity);

            if available_liquidity <= Decimal::ZERO {
                continue;
            }

            // Create signal
            let mut signal = RawSignal::new(self.id(), Arc::new((*symbol).clone()));

            // Add buy leg
            let buy_leg = TradeLeg::new(
                buy_exchange,
                Arc::new((*symbol).clone()),
                Side::Buy,
                buy_price,
                available_liquidity,
            );
            signal.add_leg(buy_leg);

            // Add sell leg
            let sell_leg = TradeLeg::new(
                sell_exchange,
                Arc::new((*symbol).clone()),
                Side::Sell,
                sell_price,
                available_liquidity,
            );
            signal.add_leg(sell_leg);

            // Set gross profit (will be adjusted in filter based on rebalancing needs)
            signal.set_profit_bps(gross_profit_bps);

            // Add metadata
            signal.add_metadata("buy_exchange", json!(buy_exchange.to_string()));
            signal.add_metadata("sell_exchange", json!(sell_exchange.to_string()));
            signal.add_metadata("buy_price", json!(buy_price.to_string()));
            signal.add_metadata("sell_price", json!(sell_price.to_string()));
            signal.add_metadata("gross_profit_bps", json!(gross_profit_bps));
            signal.add_metadata(
                "available_liquidity",
                json!(available_liquidity.to_string()),
            );

            signals.push(signal);

            debug!(
                "Cross-exchange arbitrage signal: {} buy@{} {} sell@{} {} profit={}bps",
                symbol, buy_exchange, buy_price, sell_exchange, sell_price, gross_profit_bps
            );
        }

        debug!(
            "Cross-exchange arbitrage detected {} signals",
            signals.len()
        );
        Ok(signals)
    }

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have exactly 2 legs (buy + sell)
        if signal.legs.len() < 2 {
            return Ok(false);
        }

        let buy_leg = &signal.legs[0];
        let sell_leg = &signal.legs[1];

        // Validate exchanges are allowed
        if !context.are_exchanges_allowed(buy_leg.exchange, sell_leg.exchange) {
            return Ok(false);
        }

        // Check if rebalancing is needed
        let needs_rebalancing = self.needs_rebalancing(
            context,
            buy_leg.exchange,
            sell_leg.exchange,
            &signal.symbol,
            buy_leg.quantity,
            buy_leg.price,
        );

        // Calculate net profit considering rebalancing costs
        let gross_profit_bps = signal.expected_profit_bps;
        let net_profit_bps = match self.estimate_net_profit_bps(
            gross_profit_bps,
            buy_leg.exchange,
            sell_leg.exchange,
            needs_rebalancing,
        ) {
            Ok(net) => net,
            Err(e) => {
                warn!("Failed to calculate net profit: {}", e);
                return Ok(false);
            }
        };

        // Check minimum profit threshold
        if net_profit_bps < context.min_profit_bps {
            return Ok(false);
        }

        // Calculate optimal position size
        let optimal_size = self.calculate_optimal_position_size(
            context,
            &signal.symbol,
            buy_leg.exchange,
            sell_leg.exchange,
            buy_leg.price,
            buy_leg.quantity,
        );

        if optimal_size <= Decimal::ZERO {
            return Ok(false);
        }

        // Check maximum exposure
        let total_notional = optimal_size
            .checked_mul(buy_leg.price)
            .unwrap_or(Decimal::ZERO);
        if total_notional > context.max_exposure {
            return Ok(false);
        }

        // Check minimum notional
        if total_notional < context.min_notional_usd {
            return Ok(false);
        }

        // For balance optimization, prefer signals that improve balance distribution
        let balance_optimization = self
            .config
            .custom_params
            .get("balance_optimization")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if balance_optimization {
            let base_asset = &signal.symbol.base;
            let quote_asset = &signal.symbol.quote;

            // Check if this trade improves balance distribution
            let base_distribution_optimal =
                self.is_balance_distribution_optimal(context, base_asset);
            let quote_distribution_optimal =
                self.is_balance_distribution_optimal(context, quote_asset);

            // If both distributions are already optimal and rebalancing is needed,
            // this might not be the best trade
            if base_distribution_optimal && quote_distribution_optimal && needs_rebalancing {
                // Still allow but with lower priority (could be implemented via confidence scoring)
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
