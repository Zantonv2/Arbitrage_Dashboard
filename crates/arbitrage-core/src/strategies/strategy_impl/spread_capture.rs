use crate::strategies::strategies_specifics::{StrategyLimits, StrategyUtils};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ArbitrageError, ExchangeId, OrderType, Result, Side, Symbol};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use tracing::debug;

/// Spread Capture Strategy
///
/// **Principle**: Act as a market maker by placing limit orders on both sides of the order book
/// to capture the bid-ask spread. Profit from providing liquidity to the market.
///
/// **Focus**:
/// - Identify symbols with wide, stable spreads
/// - Place maker orders to capture spread
/// - Manage inventory risk through balanced positioning
/// - Monitor spread stability and market volatility
///
/// **Supported Exchanges**: All exchanges with maker fee rebates
///
/// **Data Sources**:
/// - Order book depth and spread analysis
/// - Historical spread stability metrics
/// - Volume and volatility indicators
/// - Maker fee schedules for profitability
pub struct SpreadCaptureStrategy {
    config: StrategyConfig,
}

impl SpreadCaptureStrategy {
    /// Create a new spread capture strategy with default configuration
    pub fn new() -> Self {
        let custom_params = StrategyUtils::create_base_custom_params();

        Self {
            config: StrategyConfig {
                enabled: true,
                min_profit_bps: StrategyLimits::get_min_profit_bps("spread_capture"),
                max_exposure: StrategyLimits::get_max_exposure("spread_capture"),
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

    /// Calculate bid-ask spread in basis points
    fn calculate_spread_bps(&self, bid: Decimal, ask: Decimal) -> Result<i32> {
        if bid.is_zero() || ask.is_zero() || ask <= bid {
            return Err(ArbitrageError::Calculation(
                "Invalid bid/ask prices".to_string(),
            ));
        }

        let mid_price = (bid + ask) / Decimal::from(2);
        let spread = ask - bid;

        let spread_ratio = spread.checked_div(mid_price).ok_or_else(|| {
            ArbitrageError::Calculation("Division by zero in spread calculation".to_string())
        })?;

        let spread_bps = (spread_ratio * Decimal::from(10000))
            .to_i32()
            .ok_or_else(|| {
                ArbitrageError::Calculation("Spread BPS conversion failed".to_string())
            })?;

        Ok(spread_bps)
    }

    /// Check if spread is within acceptable range
    fn is_spread_valid(&self, spread_bps: i32) -> bool {
        let min_spread_bps = self
            .config
            .custom_params
            .get("min_spread_bps")
            .and_then(|v| v.as_i64())
            .unwrap_or(20) as i32;

        let max_spread_bps = self
            .config
            .custom_params
            .get("max_spread_bps")
            .and_then(|v| v.as_i64())
            .unwrap_or(500) as i32;

        spread_bps >= min_spread_bps && spread_bps <= max_spread_bps
    }

    /// Calculate order book depth in USD
    fn calculate_order_book_depth(
        &self,
        market_data: &MarketBundle,
        exchange: ExchangeId,
        symbol: &Symbol,
    ) -> Result<Decimal> {
        let order_book = market_data
            .get_order_book(exchange, symbol)
            .ok_or_else(|| ArbitrageError::Validation("Order book not found".to_string()))?;

        // Calculate depth on both sides (top 5 levels)
        let bid_depth: Decimal = order_book
            .bids
            .iter()
            .take(5)
            .map(|level| level.price * level.quantity)
            .sum();

        let ask_depth: Decimal = order_book
            .asks
            .iter()
            .take(5)
            .map(|level| level.price * level.quantity)
            .sum();

        // Return minimum of both sides
        Ok(bid_depth.min(ask_depth))
    }

    /// Check if order book has sufficient depth
    fn has_sufficient_depth(&self, depth_usd: Decimal) -> bool {
        let min_depth = self
            .config
            .custom_params
            .get("min_order_book_depth")
            .and_then(|v| v.as_f64())
            .and_then(|f| Decimal::try_from(f).ok())
            .unwrap_or_else(|| Decimal::from(1000));

        depth_usd >= min_depth
    }

    /// Calculate optimal maker order prices
    fn calculate_maker_prices(&self, bid: Decimal, ask: Decimal) -> Result<(Decimal, Decimal)> {
        let spread_capture_ratio = self
            .config
            .custom_params
            .get("spread_capture_ratio")
            .and_then(|v| v.as_f64())
            .and_then(|f| Decimal::try_from(f).ok())
            .unwrap_or_else(|| Decimal::new(6, 1)); // 0.6

        let spread = ask - bid;
        let capture_amount = spread.checked_mul(spread_capture_ratio).ok_or_else(|| {
            ArbitrageError::Calculation("Spread capture calculation overflow".to_string())
        })?;

        // Place our bid higher than current best bid
        let our_bid = bid + capture_amount / Decimal::from(2);

        // Place our ask lower than current best ask
        let our_ask = ask - capture_amount / Decimal::from(2);

        // Ensure our bid is still below our ask
        if our_bid >= our_ask {
            return Err(ArbitrageError::Calculation(
                "Invalid maker prices: bid >= ask".to_string(),
            ));
        }

        Ok((our_bid, our_ask))
    }

    /// Calculate position size based on configuration
    fn calculate_position_size(&self, price: Decimal) -> Result<Decimal> {
        let position_size_usd = self
            .config
            .custom_params
            .get("position_size_usd")
            .and_then(|v| v.as_f64())
            .and_then(|f| Decimal::try_from(f).ok())
            .unwrap_or_else(|| Decimal::from(500));

        let quantity = position_size_usd.checked_div(price).ok_or_else(|| {
            ArbitrageError::Calculation("Position size calculation failed".to_string())
        })?;

        Ok(quantity)
    }

    /// Estimate profit from spread capture
    fn estimate_spread_profit_bps(
        &self,
        our_bid: Decimal,
        our_ask: Decimal,
        exchange: ExchangeId,
    ) -> Result<i32> {
        // Calculate profit per round trip (buy at bid, sell at ask)
        let gross_profit = our_ask - our_bid;
        let mid_price = (our_bid + our_ask) / Decimal::from(2);

        let gross_profit_ratio = gross_profit.checked_div(mid_price).ok_or_else(|| {
            ArbitrageError::Calculation("Profit ratio calculation failed".to_string())
        })?;

        let gross_profit_bps = (gross_profit_ratio * Decimal::from(10000))
            .to_i32()
            .ok_or_else(|| {
                ArbitrageError::Calculation("Profit BPS conversion failed".to_string())
            })?;

        // Subtract maker fees (should be negative for rebates)
        let maker_fee_bps = self.get_maker_fee_bps(exchange);
        let net_profit_bps = gross_profit_bps - (maker_fee_bps * 2); // Two trades per round trip

        Ok(net_profit_bps)
    }

    /// Get maker fee in basis points for an exchange
    fn get_maker_fee_bps(&self, exchange: ExchangeId) -> i32 {
        // Typical maker fees (negative = rebate)
        match exchange {
            ExchangeId::OKX => -2,      // -0.02% maker rebate
            ExchangeId::ByBit => -2,    // -0.02% maker rebate
            ExchangeId::MEXC => 0,      // 0% maker fee
            ExchangeId::GateIo => -2,   // -0.02% maker rebate
            ExchangeId::Kraken => 16,   // 0.16% maker fee
            ExchangeId::Bitstamp => 50, // 0.50% maker fee
            _ => 20,                    // Default 0.20% maker fee for other exchanges
        }
    }

    /// Check if exchange has favorable maker fees
    fn has_favorable_maker_fees(&self, exchange: ExchangeId) -> bool {
        let maker_fee_bps = self.get_maker_fee_bps(exchange);
        let threshold_bps = self
            .config
            .custom_params
            .get("maker_fee_threshold_bps")
            .and_then(|v| v.as_i64())
            .unwrap_or(-5) as i32;

        maker_fee_bps <= threshold_bps
    }

    /// Find spread capture opportunities
    fn find_spread_opportunities(&self, market_data: &MarketBundle) -> Vec<RawSignal> {
        let mut signals = Vec::new();

        // Get all unique symbols
        let symbols = market_data.get_all_symbols();

        for symbol in symbols {
            // Check each exchange for spread opportunities
            for exchange in [
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::MEXC,
                ExchangeId::GateIo,
            ] {
                // Skip exchanges without favorable maker fees
                if !self.has_favorable_maker_fees(exchange) {
                    continue;
                }

                // Get order book
                let order_book = match market_data.get_order_book(exchange, &symbol) {
                    Some(book) => book,
                    None => continue,
                };

                // Get best bid and ask
                let (best_bid, best_ask) = match (order_book.best_bid(), order_book.best_ask()) {
                    (Some(bid), Some(ask)) => (bid.price, ask.price),
                    _ => continue,
                };

                // Calculate spread
                let spread_bps = match self.calculate_spread_bps(best_bid, best_ask) {
                    Ok(spread) => spread,
                    Err(_) => continue,
                };

                // Check if spread is valid
                if !self.is_spread_valid(spread_bps) {
                    continue;
                }

                // Check order book depth
                let depth_usd =
                    match self.calculate_order_book_depth(market_data, exchange, &symbol) {
                        Ok(depth) => depth,
                        Err(_) => continue,
                    };

                if !self.has_sufficient_depth(depth_usd) {
                    continue;
                }

                // Calculate optimal maker prices
                let (our_bid, our_ask) = match self.calculate_maker_prices(best_bid, best_ask) {
                    Ok(prices) => prices,
                    Err(_) => continue,
                };

                // Calculate position size
                let mid_price = (our_bid + our_ask) / Decimal::from(2);
                let position_size = match self.calculate_position_size(mid_price) {
                    Ok(size) => size,
                    Err(_) => continue,
                };

                // Estimate profit
                let profit_bps = match self.estimate_spread_profit_bps(our_bid, our_ask, exchange) {
                    Ok(profit) => profit,
                    Err(_) => continue,
                };

                // Check minimum profit
                if profit_bps < self.config.min_profit_bps {
                    continue;
                }

                // Create signal with both bid and ask orders
                let mut signal = RawSignal::new(self.id(), (*symbol).clone());

                // Add bid leg (buy order)
                let bid_leg = TradeLeg::new(
                    exchange,
                    (*symbol).clone(),
                    Side::Buy,
                    our_bid,
                    position_size,
                )
                .with_order_type(OrderType::Limit);
                signal.add_leg(bid_leg);

                // Add ask leg (sell order)
                let ask_leg = TradeLeg::new(
                    exchange,
                    (*symbol).clone(),
                    Side::Sell,
                    our_ask,
                    position_size,
                )
                .with_order_type(OrderType::Limit);
                signal.add_leg(ask_leg);

                signal.set_profit_bps(profit_bps);

                // Add metadata
                signal.add_metadata("spread_bps", json!(spread_bps));
                signal.add_metadata("our_bid", json!(our_bid.to_string()));
                signal.add_metadata("our_ask", json!(our_ask.to_string()));
                signal.add_metadata("market_bid", json!(best_bid.to_string()));
                signal.add_metadata("market_ask", json!(best_ask.to_string()));
                signal.add_metadata("depth_usd", json!(depth_usd.to_string()));
                signal.add_metadata("maker_fee_bps", json!(self.get_maker_fee_bps(exchange)));
                signal.add_metadata("position_size", json!(position_size.to_string()));

                signals.push(signal);

                debug!(
                    "Spread capture opportunity: {} on {} spread={}bps profit={}bps",
                    symbol, exchange, spread_bps, profit_bps
                );
            }
        }

        signals
    }
}

impl Default for SpreadCaptureStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for SpreadCaptureStrategy {
    fn id(&self) -> &'static str {
        "spread_capture"
    }

    fn name(&self) -> &'static str {
        "Spread Capture Strategy"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        debug!(
            "Spread capture scanning {} order books",
            market_data.order_books.len()
        );

        let signals = self.find_spread_opportunities(market_data);

        debug!("Spread capture detected {} signals", signals.len());
        Ok(signals)
    }

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have exactly 2 legs (bid + ask orders)
        if signal.legs.len() != 2 {
            return Ok(false);
        }

        let bid_leg = &signal.legs[0];
        let ask_leg = &signal.legs[1];

        // Both legs should be on the same exchange
        if bid_leg.exchange != ask_leg.exchange {
            return Ok(false);
        }

        // Validate exchange is allowed
        if !context.is_exchange_allowed(bid_leg.exchange) {
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

        // Check inventory requirements for both sides
        let base_asset = &signal.symbol.base;
        let quote_asset = &signal.symbol.quote;

        // For bid leg (buying), need quote currency
        let required_quote = bid_leg
            .price
            .checked_mul(bid_leg.quantity)
            .unwrap_or(Decimal::ZERO);
        if !context.can_sell(bid_leg.exchange, quote_asset, required_quote) {
            return Ok(false);
        }

        // For ask leg (selling), need base currency
        if !context.can_sell(ask_leg.exchange, base_asset, ask_leg.quantity) {
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
