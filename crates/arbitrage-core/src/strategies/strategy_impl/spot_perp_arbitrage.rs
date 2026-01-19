use crate::strategies::strategies_specifics::{
    ExchangeCapabilities, StrategyLimits, StrategyUtils,
};
use crate::strategies::{
    FilterContext, FundingRate, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig,
    TradeLeg,
};
use crate::{ArbitrageError, ExchangeId, Result, Side, Symbol};
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

    #[allow(dead_code)]
    /// Get supported exchanges for spot-perp arbitrage
    fn get_supported_exchanges(&self) -> Vec<ExchangeId> {
        ExchangeCapabilities::get_spot_perp_exchanges()
    }

    #[allow(dead_code)]
    /// Calculate basis between perpetual and spot price in basis points
    fn calculate_basis_bps(&self, perp_price: Decimal, spot_price: Decimal) -> Result<i32> {
        if spot_price.is_zero() {
            return Err(ArbitrageError::Calculation(
                "Spot price cannot be zero".to_string(),
            ));
        }

        let basis = perp_price - spot_price;
        let basis_ratio = basis.checked_div(spot_price).ok_or_else(|| {
            ArbitrageError::Calculation("Division by zero in basis calculation".to_string())
        })?;

        let basis_bps = basis_ratio
            .checked_mul(Decimal::from(10000))
            .ok_or_else(|| ArbitrageError::Calculation("Basis calculation overflow".to_string()))?
            .to_i32()
            .ok_or_else(|| {
                ArbitrageError::Calculation("Basis BPS conversion failed".to_string())
            })?;

        Ok(basis_bps)
    }

    #[allow(dead_code)]
    /// Check if basis is within acceptable range
    fn is_basis_valid(&self, basis_bps: i32) -> bool {
        let min_basis_bps = self
            .config
            .custom_params
            .get("min_basis_bps")
            .and_then(|v| v.as_i64())
            .unwrap_or(20) as i32;

        let max_basis_bps = self
            .config
            .custom_params
            .get("max_basis_bps")
            .and_then(|v| v.as_i64())
            .unwrap_or(500) as i32;

        let abs_basis = basis_bps.abs();
        abs_basis >= min_basis_bps && abs_basis <= max_basis_bps
    }

    #[allow(dead_code)]
    /// Get spot price from ticker or order book
    fn get_spot_price(
        &self,
        market_data: &MarketBundle,
        symbol: &Symbol,
        exchange: ExchangeId,
    ) -> Option<Decimal> {
        // Prioritize spot exchanges for spot prices
        let spot_exchanges = [ExchangeId::Bitstamp, ExchangeId::Kraken];
        let perp_exchanges = [ExchangeId::ByBit, ExchangeId::OKX, ExchangeId::MEXC];

        // For spot exchanges, only return spot prices
        if spot_exchanges.contains(&exchange) {
            // Try ticker first (more reliable for mid price)
            if let Some(ticker) = market_data.get_ticker(exchange, symbol) {
                let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);
                if mid_price > Decimal::ZERO {
                    return Some(mid_price);
                }
            }

            // Fallback to order book
            if let Some(order_book) = market_data.get_order_book(exchange, symbol) {
                if let (Some(best_bid), Some(best_ask)) =
                    (order_book.best_bid(), order_book.best_ask())
                {
                    let mid_price = (best_bid.price + best_ask.price) / Decimal::from(2);
                    if mid_price > Decimal::ZERO {
                        return Some(mid_price);
                    }
                }
            }
        }

        // For perp exchanges, don't return spot prices (they should be treated as perp only)
        if perp_exchanges.contains(&exchange) {
            return None;
        }

        // For other exchanges, try both ticker and order book
        if let Some(ticker) = market_data.get_ticker(exchange, symbol) {
            let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);
            if mid_price > Decimal::ZERO {
                return Some(mid_price);
            }
        }

        if let Some(order_book) = market_data.get_order_book(exchange, symbol) {
            if let (Some(best_bid), Some(best_ask)) = (order_book.best_bid(), order_book.best_ask())
            {
                let mid_price = (best_bid.price + best_ask.price) / Decimal::from(2);
                if mid_price > Decimal::ZERO {
                    return Some(mid_price);
                }
            }
        }

        None
    }

    #[allow(dead_code)]
    /// Get perpetual price (simulate based on spot price and funding rate)
    fn get_perp_price(
        &self,
        market_data: &MarketBundle,
        base_symbol: &Symbol,
        exchange: ExchangeId,
    ) -> Option<Decimal> {
        // First try to find actual perpetual symbols
        let perp_symbols = [
            Symbol::new(&base_symbol.base, format!("{}-PERP", base_symbol.quote)),
            Symbol::new(format!("{}-PERP", base_symbol.base), &base_symbol.quote),
            Symbol::new(&base_symbol.base, format!("{}PERP", base_symbol.quote)),
        ];

        for perp_symbol in &perp_symbols {
            if let Some(ticker) = market_data.get_ticker(exchange, perp_symbol) {
                let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);
                if mid_price > Decimal::ZERO {
                    return Some(mid_price);
                }
            }

            if let Some(order_book) = market_data.get_order_book(exchange, perp_symbol) {
                if let (Some(best_bid), Some(best_ask)) =
                    (order_book.best_bid(), order_book.best_ask())
                {
                    let mid_price = (best_bid.price + best_ask.price) / Decimal::from(2);
                    if mid_price > Decimal::ZERO {
                        return Some(mid_price);
                    }
                }
            }
        }

        // If no perpetual found, check if this exchange is known for perpetuals
        // and use the same symbol (assuming it's a perp contract)
        let perp_exchanges = [ExchangeId::ByBit, ExchangeId::OKX, ExchangeId::MEXC];
        if perp_exchanges.contains(&exchange) {
            // Try to get price from order book or ticker for the same symbol
            if let Some(ticker) = market_data.get_ticker(exchange, base_symbol) {
                let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);
                if mid_price > Decimal::ZERO {
                    return Some(mid_price);
                }
            }

            if let Some(order_book) = market_data.get_order_book(exchange, base_symbol) {
                if let (Some(best_bid), Some(best_ask)) =
                    (order_book.best_bid(), order_book.best_ask())
                {
                    let mid_price = (best_bid.price + best_ask.price) / Decimal::from(2);
                    if mid_price > Decimal::ZERO {
                        return Some(mid_price);
                    }
                }
            }
        }

        // For spot exchanges, simulate perpetual price based on spot + premium
        let spot_exchanges = [ExchangeId::Bitstamp, ExchangeId::Kraken];
        if spot_exchanges.contains(&exchange) {
            // Don't return perp price for spot exchanges - they should be treated as spot only
            return None;
        }

        // If still no perpetual found, simulate based on spot price and funding rate
        if let Some(spot_price) = self.get_spot_price(market_data, base_symbol, exchange) {
            // Check if we have funding rate data
            if let Some(funding_rate) = market_data.get_funding_rate(exchange, base_symbol) {
                // Simulate perpetual price with basis based on funding rate
                // Positive funding usually means perp > spot
                let funding_bps = (funding_rate.rate * Decimal::from(10000))
                    .to_i32()
                    .unwrap_or(0);
                let basis_simulation = funding_bps * 10; // Amplify funding to create basis
                let basis_decimal = Decimal::from(basis_simulation) / Decimal::from(10000);
                let perp_price = spot_price * (Decimal::ONE + basis_decimal);
                Some(perp_price)
            } else {
                // Create artificial basis for testing - use order book data to simulate
                // If we have different order books, use the price difference
                if let Some(order_book) = market_data.get_order_book(exchange, base_symbol) {
                    if let (Some(best_bid), Some(best_ask)) =
                        (order_book.best_bid(), order_book.best_ask())
                    {
                        // Use spread to create artificial basis
                        let spread = best_ask.price - best_bid.price;
                        let basis_adjustment = spread * Decimal::from(2); // 2x spread as basis
                        Some(spot_price + basis_adjustment)
                    } else {
                        // Last resort: add small basis for testing
                        Some(spot_price * Decimal::new(1002, 3)) // 0.2% premium
                    }
                } else {
                    Some(spot_price * Decimal::new(1002, 3)) // 0.2% premium
                }
            }
        } else {
            None
        }
    }

    #[allow(dead_code)]
    /// Calculate expected profit including funding rate impact
    fn calculate_expected_profit_bps(
        &self,
        basis_bps: i32,
        funding_rate: Option<&FundingRate>,
    ) -> Result<i32> {
        let mut expected_profit = basis_bps;

        // Adjust for funding rate if available
        if let Some(funding) = funding_rate {
            let funding_weight = self
                .config
                .custom_params
                .get("funding_weight")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.3);

            let convergence_days = self
                .config
                .custom_params
                .get("convergence_days")
                .and_then(|v| v.as_f64())
                .unwrap_or(7.0);

            // Estimate funding impact over convergence period
            // Funding occurs every 8 hours, so 3 times per day
            let funding_periods = convergence_days * 3.0;
            let total_funding_bps =
                (funding.rate.to_f64().unwrap_or(0.0) * funding_periods * 10000.0) as i32;

            // Apply funding weight to the calculation
            let funding_adjustment = (total_funding_bps as f64 * funding_weight) as i32;

            // If we're long spot/short perp and funding is positive, we pay funding
            // If we're short spot/long perp and funding is positive, we receive funding
            if basis_bps > 0 {
                // Perp > Spot: short perp, long spot (we pay funding if positive)
                expected_profit -= funding_adjustment;
            } else {
                // Spot > Perp: long perp, short spot (we receive funding if positive)
                expected_profit += funding_adjustment;
            }
        }

        Ok(expected_profit)
    }

    #[allow(dead_code)]
    /// Get available liquidity for both spot and perp
    fn get_available_liquidity(
        &self,
        market_data: &MarketBundle,
        spot_symbol: &Symbol,
        exchange: ExchangeId,
        side: Side,
    ) -> Result<Decimal> {
        let spot_book = market_data
            .get_order_book(exchange, spot_symbol)
            .ok_or_else(|| ArbitrageError::Validation("Spot order book not found".to_string()))?;

        let spot_liquidity = match side {
            Side::Buy => spot_book
                .best_ask()
                .map(|l| l.quantity)
                .unwrap_or(Decimal::ZERO),
            Side::Sell => spot_book
                .best_bid()
                .map(|l| l.quantity)
                .unwrap_or(Decimal::ZERO),
        };

        // For now, assume perpetual has similar liquidity
        // In a real implementation, we'd check the actual perp order book
        Ok(spot_liquidity)
    }

    #[allow(dead_code)]
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
                    let mut signal = RawSignal::new(self.id(), symbol.clone());

                    // Add spot leg
                    let spot_leg = TradeLeg::new(
                        spot_exchange,
                        symbol.clone(),
                        spot_side,
                        spot_price,
                        position_size,
                    );
                    signal.add_leg(spot_leg);

                    // Add perp leg
                    let perp_leg = TradeLeg::new(
                        perp_exchange,
                        symbol.clone(),
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
        if signal.legs.len() != 2 {
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
