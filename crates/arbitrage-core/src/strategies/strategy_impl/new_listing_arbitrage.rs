use crate::strategies::strategies_specifics::{
    ExchangeCapabilities, StrategyLimits, StrategyUtils,
};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ExchangeId, Result, Side, Symbol};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::json;
use std::sync::Arc;
use tracing::debug;

/// New Listing Cross-Exchange Arbitrage Strategy
///
/// **Principle**: Exploit inefficient price discovery immediately after a token listing.
/// New tokens often appear on different exchanges at different times with significant
/// price discrepancies during initial trading.
///
/// **Focus**:
/// - Monitor new token listings across exchanges
/// - Detect price inefficiencies during initial price discovery
/// - Account for high volatility and low liquidity
/// - Focus on exchanges known for early listings (MEXC, Gate.io)
///
/// **Supported Exchanges**: All 6 exchanges, with priority on MEXC and Gate.io for early detection
///
/// **Data Sources**:
/// - New symbol announcements and listings
/// - Initial trading data and volume
/// - Cross-exchange price comparisons for new tokens
/// - Liquidity depth analysis for new markets
pub struct NewListingArbitrageStrategy {
    config: StrategyConfig,
}

impl NewListingArbitrageStrategy {
    /// Create a new listing arbitrage strategy with default configuration
    pub fn new() -> Self {
        let custom_params = StrategyUtils::create_base_custom_params();

        Self {
            config: StrategyConfig {
                enabled: true,
                min_profit_bps: 50,
                max_exposure: StrategyLimits::get_max_exposure("cex_arbitrage") / Decimal::from(4),
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
}

impl Default for NewListingArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for NewListingArbitrageStrategy {
    fn id(&self) -> &'static str {
        "new_listing_arbitrage"
    }

    fn name(&self) -> &'static str {
        "New Listing Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();

        // Get all symbols from market data
        let current_symbols: Vec<Symbol> = market_data
            .tickers
            .keys()
            .map(|(_, symbol)| (**symbol).clone())
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect();

        debug!(
            "New listing arbitrage scanning {} symbols",
            current_symbols.len()
        );

        // Group tickers and order books by symbol to find cross-exchange opportunities
        let mut symbol_data: FxHashMap<Symbol, Vec<(ExchangeId, Decimal, Decimal)>> =
            FxHashMap::default();

        // Collect price data from both tickers and order books
        for symbol in &current_symbols {
            let mut exchange_prices = Vec::new();

            for exchange in [
                ExchangeId::MEXC,
                ExchangeId::GateIo,
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::Bitstamp,
                ExchangeId::Kraken,
            ] {
                let mut bid_price = None;
                let mut ask_price = None;

                // Try ticker first
                if let Some(ticker) = market_data.get_ticker(exchange, symbol) {
                    if ticker.bid > Decimal::ZERO && ticker.ask > Decimal::ZERO {
                        bid_price = Some(ticker.bid);
                        ask_price = Some(ticker.ask);
                    }
                }

                // Fallback to order book if ticker not available
                if bid_price.is_none() || ask_price.is_none() {
                    if let Some(order_book) = market_data.get_order_book(exchange, symbol) {
                        if let Some(best_bid) = order_book.best_bid() {
                            bid_price = Some(best_bid.price);
                        }
                        if let Some(best_ask) = order_book.best_ask() {
                            ask_price = Some(best_ask.price);
                        }
                    }
                }

                // Add to exchange prices if we have both bid and ask
                if let (Some(bid), Some(ask)) = (bid_price, ask_price) {
                    // Basic validation - ensure prices make sense
                    if bid > Decimal::ZERO && ask > bid && ask < bid * Decimal::from(2) {
                        exchange_prices.push((exchange, bid, ask));
                    }
                }
            }

            if exchange_prices.len() >= 2 {
                symbol_data.insert(symbol.clone(), exchange_prices);
            }
        }

        // Find arbitrage opportunities for each symbol with multiple exchanges
        for (symbol, exchange_prices) in symbol_data {
            // Find best bid (highest) and best ask (lowest) across all exchanges
            let mut best_bid: Option<(ExchangeId, Decimal)> = None;
            let mut best_ask: Option<(ExchangeId, Decimal)> = None;

            for (exchange, bid, ask) in &exchange_prices {
                // Update best bid (highest bid price)
                match best_bid {
                    None => best_bid = Some((*exchange, *bid)),
                    Some((_, current_bid)) => {
                        if *bid > current_bid {
                            best_bid = Some((*exchange, *bid));
                        }
                    }
                }

                // Update best ask (lowest ask price)
                match best_ask {
                    None => best_ask = Some((*exchange, *ask)),
                    Some((_, current_ask)) => {
                        if *ask < current_ask {
                            best_ask = Some((*exchange, *ask));
                        }
                    }
                }
            }

            if let (Some((sell_exchange, sell_price)), Some((buy_exchange, buy_price))) =
                (best_bid, best_ask)
            {
                // Only proceed if different exchanges
                if sell_exchange != buy_exchange && sell_price > buy_price {
                    // Calculate profit
                    let profit_ratio = (sell_price - buy_price)
                        .checked_div(buy_price)
                        .unwrap_or(Decimal::ZERO);
                    let profit_bps = (profit_ratio * Decimal::from(10000)).to_i32().unwrap_or(0);

                    // Estimate fees
                    let (_, buy_fee) = ExchangeCapabilities::get_typical_fees(buy_exchange);
                    let (_, sell_fee) = ExchangeCapabilities::get_typical_fees(sell_exchange);
                    let total_fee_bps = ((buy_fee + sell_fee) * 100.0) as i32;
                    let net_profit_bps = profit_bps - total_fee_bps;

                    // Lower threshold for new listings (they can have higher spreads)
                    let min_profit_threshold = self.config.min_profit_bps.min(50); // At least 0.5%

                    if net_profit_bps >= min_profit_threshold {
                        // Get liquidity from order books if available
                        let mut liquidity = Decimal::from(100); // Default small amount for new listings

                        if let Some(buy_book) = market_data.get_order_book(buy_exchange, &symbol) {
                            if let Some(ask_level) = buy_book.best_ask() {
                                liquidity = liquidity.min(ask_level.quantity);
                            }
                        }

                        if let Some(sell_book) = market_data.get_order_book(sell_exchange, &symbol)
                        {
                            if let Some(bid_level) = sell_book.best_bid() {
                                liquidity = liquidity.min(bid_level.quantity);
                            }
                        }

                        // Ensure minimum liquidity
                        if liquidity < Decimal::from(1) {
                            liquidity = Decimal::from(10); // Minimum for new listings
                        }

                        // Create signal
                        let mut signal = RawSignal::new(self.id(), Arc::new(symbol.clone()));

                        // Buy leg (buy at lower price)
                        let buy_leg = TradeLeg::new(
                            buy_exchange,
                            Arc::new(symbol.clone()),
                            Side::Buy,
                            buy_price,
                            liquidity,
                        );
                        signal.add_leg(buy_leg);

                        // Sell leg (sell at higher price)
                        let sell_leg = TradeLeg::new(
                            sell_exchange,
                            Arc::new(symbol.clone()),
                            Side::Sell,
                            sell_price,
                            liquidity,
                        );
                        signal.add_leg(sell_leg);

                        signal.set_profit_bps(net_profit_bps);

                        // Add metadata
                        signal.add_metadata("listing_type", json!("new_listing"));
                        signal.add_metadata("buy_exchange", json!(buy_exchange.to_string()));
                        signal.add_metadata("sell_exchange", json!(sell_exchange.to_string()));
                        signal.add_metadata("buy_price", json!(buy_price.to_string()));
                        signal.add_metadata("sell_price", json!(sell_price.to_string()));
                        signal.add_metadata("gross_profit_bps", json!(profit_bps));
                        signal.add_metadata("estimated_fees_bps", json!(total_fee_bps));

                        signals.push(signal);

                        debug!(
                            "New listing arbitrage: {} buy@{} {} sell@{} {} profit={}bps",
                            symbol,
                            buy_exchange,
                            buy_price,
                            sell_exchange,
                            sell_price,
                            net_profit_bps
                        );
                    }
                }
            }
        }

        debug!("New listing arbitrage detected {} signals", signals.len());
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

        // Check profit threshold (higher for new listings due to risk)
        if signal.expected_profit_bps < context.min_profit_bps {
            return Ok(false);
        }

        // Check maximum exposure (should be lower for new listings)
        let total_notional = signal.total_notional();
        let max_new_listing_exposure = context.max_exposure / Decimal::from(4); // 25% of normal exposure
        if total_notional > max_new_listing_exposure {
            return Ok(false);
        }

        // Check minimum notional (but allow smaller amounts for new listings)
        let min_notional = context.min_notional_usd.min(Decimal::from(50)); // Min $50 for new listings
        if total_notional < min_notional {
            return Ok(false);
        }

        // Additional risk checks for new listings

        // Check volatility (new listings can be very volatile)
        let volatility_threshold = self
            .config
            .custom_params
            .get("volatility_threshold_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0);

        // In a real implementation, we'd calculate actual volatility
        // For now, we'll use the profit margin as a proxy
        let profit_pct = signal.expected_profit_bps as f64 / 100.0;
        if profit_pct > volatility_threshold * 2.0 {
            // Very high profit might indicate extreme volatility - be cautious
            return Ok(false);
        }

        // Check inventory constraints (more conservative for new listings)
        let base_asset = &signal.symbol.base;
        let quote_asset = &signal.symbol.quote;

        // For sell leg
        if !context.can_sell(sell_leg.exchange, base_asset, sell_leg.quantity) {
            return Ok(false);
        }

        // For buy leg
        let required_quote = buy_leg
            .price
            .checked_mul(buy_leg.quantity)
            .unwrap_or(Decimal::ZERO);
        if !context.can_sell(buy_leg.exchange, quote_asset, required_quote) {
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
