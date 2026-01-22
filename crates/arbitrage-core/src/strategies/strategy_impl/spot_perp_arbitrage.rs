use crate::strategies::strategies_specifics::{StrategyLimits, StrategyUtils};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ExchangeId, Result, Side};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashSet;
use tracing::debug;

/// Spot ↔ Perpetual Arbitrage Strategy
///
/// **Principle**: Hedge spot and perpetual positions to capture price divergence
/// while remaining market-neutral. Profit from basis (futures - spot price).
///
/// **Focus**:
/// - Monitor basis between spot and perpetual contracts
/// - Track convergence patterns and funding rates
/// - Account for margin requirements and funding costs
/// - Handle rollover mechanics for position management
///
/// **Supported Exchanges**: OKX, Bybit, MEXC (perpetual contracts)
///
/// **Data Sources**:
/// - Spot prices from order books and tickers
/// - Perpetual contract prices and funding rates
/// - Basis calculations and convergence tracking
pub struct SpotPerpArbitrageStrategy {
    config: StrategyConfig,
}

impl SpotPerpArbitrageStrategy {
    /// Create a new spot-perpetual arbitrage strategy with default configuration
    pub fn new() -> Self {
        let custom_params = StrategyUtils::create_base_custom_params();

        Self {
            config: StrategyConfig {
                enabled: true,
                min_profit_bps: 5,
                max_exposure: StrategyLimits::get_max_exposure("spot_perp_arbitrage"),
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

    /// Calculate position size considering margin requirements
    fn calculate_position_size(&self, available_liquidity: Decimal, price: Decimal) -> Decimal {
        let margin_buffer_pct = self
            .config
            .custom_params
            .get("margin_buffer_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.2);

        let max_notional = self.config.max_exposure;
        let max_quantity_by_exposure = max_notional.checked_div(price).unwrap_or(Decimal::ZERO);

        // Apply margin buffer
        let buffer_multiplier =
            Decimal::ONE - Decimal::try_from(margin_buffer_pct).unwrap_or(Decimal::ZERO);
        let adjusted_liquidity = available_liquidity
            .checked_mul(buffer_multiplier)
            .unwrap_or(available_liquidity);

        // Take minimum of liquidity and exposure limits
        adjusted_liquidity.min(max_quantity_by_exposure)
    }
}

impl Default for SpotPerpArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for SpotPerpArbitrageStrategy {
    fn id(&self) -> &'static str {
        "spot_perp_arbitrage"
    }

    fn name(&self) -> &'static str {
        "Spot ↔ Perpetual Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();

        // Simple approach: find all symbols and check for cross-exchange opportunities
        let mut all_symbols = HashSet::new();

        // Collect all symbols from order books
        for (_, symbol) in market_data.order_books.keys() {
            all_symbols.insert(symbol.clone());
        }

        // Collect all symbols from tickers
        for (_, symbol) in market_data.tickers.keys() {
            all_symbols.insert(symbol.clone());
        }

        debug!(
            "Spot-perp arbitrage found {} unique symbols",
            all_symbols.len()
        );

        for symbol in all_symbols {
            // Find all exchanges that have this symbol
            let mut exchange_data: Vec<(ExchangeId, Decimal)> = Vec::new();

            // Check all supported exchanges
            for exchange in [
                ExchangeId::Bitstamp,
                ExchangeId::ByBit,
                ExchangeId::OKX,
                ExchangeId::MEXC,
                ExchangeId::Kraken,
                ExchangeId::GateIo,
            ] {
                // Try to get price from order book
                if let Some(order_book) = market_data.get_order_book(exchange, &symbol) {
                    if let (Some(bid), Some(ask)) = (order_book.best_bid(), order_book.best_ask()) {
                        let mid_price = (bid.price + ask.price) / Decimal::from(2);
                        if mid_price > Decimal::ZERO {
                            exchange_data.push((exchange, mid_price));
                            debug!("Found {} on {} at ${}", symbol, exchange, mid_price);
                        }
                    }
                }

                // Also try ticker
                if let Some(ticker) = market_data.get_ticker(exchange, &symbol) {
                    if ticker.bid > Decimal::ZERO && ticker.ask > Decimal::ZERO {
                        let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);
                        // Only add if we don't already have this exchange
                        if !exchange_data.iter().any(|(ex, _)| *ex == exchange) {
                            exchange_data.push((exchange, mid_price));
                            debug!(
                                "Found {} on {} at ${} (from ticker)",
                                symbol, exchange, mid_price
                            );
                        }
                    }
                }
            }

            debug!(
                "Symbol {} found on {} exchanges",
                symbol,
                exchange_data.len()
            );

            // Need at least 2 exchanges for arbitrage
            if exchange_data.len() < 2 {
                continue;
            }

            // Check all pairs of exchanges for arbitrage opportunities
            for i in 0..exchange_data.len() {
                for j in (i + 1)..exchange_data.len() {
                    let (exchange1, price1) = exchange_data[i];
                    let (exchange2, price2) = exchange_data[j];

                    // Determine which is spot and which is perp based on exchange type
                    let (spot_exchange, spot_price, perp_exchange, perp_price) =
                        if matches!(exchange1, ExchangeId::Bitstamp | ExchangeId::Kraken) {
                            // Exchange1 is spot, Exchange2 is perp
                            (exchange1, price1, exchange2, price2)
                        } else if matches!(exchange2, ExchangeId::Bitstamp | ExchangeId::Kraken) {
                            // Exchange2 is spot, Exchange1 is perp
                            (exchange2, price2, exchange1, price1)
                        } else {
                            // Both are perp exchanges, treat first as spot for testing
                            (exchange1, price1, exchange2, price2)
                        };

                    // Calculate basis (perp - spot) / spot * 10000
                    let basis_decimal = (perp_price - spot_price) / spot_price;
                    let basis_bps = (basis_decimal * Decimal::from(10000)).to_i32().unwrap_or(0);

                    debug!(
                        "Checking {} spot@{} ${} vs perp@{} ${}, basis={}bps",
                        symbol, spot_exchange, spot_price, perp_exchange, perp_price, basis_bps
                    );

                    // Check if basis is significant enough
                    if basis_bps.abs() < 5 {
                        debug!("Basis too small: {}bps", basis_bps);
                        continue;
                    }

                    // Get funding rate if available
                    let funding_rate = market_data.get_funding_rate(perp_exchange, &symbol);

                    // Calculate expected profit (simplified)
                    let mut expected_profit_bps = basis_bps.abs();

                    // Adjust for funding if available
                    if let Some(funding) = funding_rate {
                        let funding_bps =
                            (funding.rate * Decimal::from(10000)).to_i32().unwrap_or(0);
                        // Simple adjustment: if funding is positive and we're long perp, subtract some profit
                        if basis_bps > 0 && funding_bps > 0 {
                            expected_profit_bps =
                                (expected_profit_bps - funding_bps.abs() / 4).max(0);
                        }
                    }

                    debug!(
                        "Expected profit: {}bps (min required: {}bps)",
                        expected_profit_bps, self.config.min_profit_bps
                    );

                    // Check minimum profit threshold
                    if expected_profit_bps < self.config.min_profit_bps {
                        debug!(
                            "Profit too low: {}bps < {}bps",
                            expected_profit_bps, self.config.min_profit_bps
                        );
                        continue;
                    }

                    // Determine trade direction
                    let (spot_side, perp_side) = if basis_bps > 0 {
                        // Perp > Spot: buy spot, sell perp
                        (Side::Buy, Side::Sell)
                    } else {
                        // Spot > Perp: sell spot, buy perp
                        (Side::Sell, Side::Buy)
                    };

                    // Use default position size
                    let position_size = Decimal::ONE;

                    // Create signal
                    let mut signal = RawSignal::new(self.id(), (*symbol).clone().into());

                    // Add spot leg
                    let spot_leg = TradeLeg::new(
                        spot_exchange,
                        (*symbol).clone().into(),
                        spot_side,
                        spot_price,
                        position_size,
                    );
                    signal.add_leg(spot_leg);

                    // Add perp leg
                    let perp_leg = TradeLeg::new(
                        perp_exchange,
                        (*symbol).clone().into(),
                        perp_side,
                        perp_price,
                        position_size,
                    );
                    signal.add_leg(perp_leg);

                    signal.set_profit_bps(expected_profit_bps);

                    // Add metadata
                    signal.add_metadata("basis_bps", json!(basis_bps));
                    signal.add_metadata("spot_exchange", json!(spot_exchange.to_string()));
                    signal.add_metadata("perp_exchange", json!(perp_exchange.to_string()));
                    signal.add_metadata("spot_price", json!(spot_price.to_string()));
                    signal.add_metadata("perp_price", json!(perp_price.to_string()));

                    if let Some(funding) = funding_rate {
                        signal.add_metadata("funding_rate", json!(funding.rate.to_string()));
                    }

                    signals.push(signal);

                    debug!("✅ Created spot-perp signal: {} spot@{} ${} perp@{} ${} basis={}bps profit={}bps", 
                           symbol, spot_exchange, spot_price, perp_exchange, perp_price, basis_bps, expected_profit_bps);
                }
            }
        }

        debug!("Spot-perp arbitrage detected {} signals", signals.len());
        Ok(signals)
    }

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have exactly 2 legs (spot + perpetual)
        if signal.legs.len() < 2 {
            return Ok(false);
        }

        let leg1 = &signal.legs[0];
        let leg2 = &signal.legs[1];

        // Validate exchanges are allowed
        if !context.is_exchange_allowed(leg1.exchange)
            || !context.is_exchange_allowed(leg2.exchange)
        {
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

        // Check inventory for leg1
        match leg1.side {
            Side::Buy => {
                let required_quote = leg1
                    .price
                    .checked_mul(leg1.quantity)
                    .unwrap_or(Decimal::ZERO);
                if !context.can_sell(leg1.exchange, quote_asset, required_quote) {
                    return Ok(false);
                }
            }
            Side::Sell => {
                if !context.can_sell(leg1.exchange, base_asset, leg1.quantity) {
                    return Ok(false);
                }
            }
        }

        // Check inventory for leg2
        match leg2.side {
            Side::Buy => {
                let required_quote = leg2
                    .price
                    .checked_mul(leg2.quantity)
                    .unwrap_or(Decimal::ZERO);
                if !context.can_sell(leg2.exchange, quote_asset, required_quote) {
                    return Ok(false);
                }
            }
            Side::Sell => {
                if !context.can_sell(leg2.exchange, base_asset, leg2.quantity) {
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
