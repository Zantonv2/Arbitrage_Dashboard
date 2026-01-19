use crate::strategies::strategies_specifics::{
    CexArbitrageDefaults, ExchangeCapabilities, StrategyLimits,
};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ArbitrageError, ExchangeId, Result, Side, Symbol};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use tracing::{debug, warn};

/// CEX ↔ CEX Price Arbitrage Strategy
///
/// **Principle**: Simultaneously buy on the exchange with the lowest ask
/// and sell on the exchange with the highest bid using pre-funded balances.
///
/// **Focus**:
/// - Bid/ask spread analysis across all supported exchanges
/// - Account for trading fees on both exchanges  
/// - Validate symbol availability on both exchanges
/// - Consider execution latency and slippage
///
/// **Supported Exchanges**: OKX, Bybit, MEXC, Gate.io, Bitstamp, Kraken
///
/// **Data Sources**:
/// - Order books from all 6 exchanges via WebSocket feeds
/// - Real-time tickers for price validation
/// - Exchange-specific fee schedules
pub struct CexArbitrageStrategy {
    config: StrategyConfig,
}

impl CexArbitrageStrategy {
    /// Create a new CEX arbitrage strategy with default configuration
    pub fn new() -> Self {
        Self {
            config: StrategyConfig {
                min_profit_bps: StrategyLimits::get_min_profit_bps("cex_arbitrage"),
                max_exposure: StrategyLimits::get_max_exposure("cex_arbitrage"),
                custom_params: CexArbitrageDefaults::get_custom_params(),
                ..Default::default()
            },
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        Self { config }
    }

    /// Get supported exchanges for CEX arbitrage
    fn get_supported_exchanges(&self) -> Vec<ExchangeId> {
        ExchangeCapabilities::get_cex_arbitrage_exchanges()
    }

    /// Find the best bid (highest price to sell at) across all exchanges
    fn find_best_bid(
        &self,
        market_data: &MarketBundle,
        symbol: &Symbol,
    ) -> Option<(ExchangeId, Decimal)> {
        let mut best_bid: Option<(ExchangeId, Decimal)> = None;

        for exchange in self.get_supported_exchanges() {
            if let Some(order_book) = market_data.get_order_book(exchange, symbol) {
                if let Some(best_bid_level) = order_book.best_bid() {
                    match best_bid {
                        None => best_bid = Some((exchange, best_bid_level.price)),
                        Some((_, current_best)) => {
                            if best_bid_level.price > current_best {
                                best_bid = Some((exchange, best_bid_level.price));
                            }
                        }
                    }
                }
            }
        }

        best_bid
    }

    /// Find the best ask (lowest price to buy at) across all exchanges
    fn find_best_ask(
        &self,
        market_data: &MarketBundle,
        symbol: &Symbol,
    ) -> Option<(ExchangeId, Decimal)> {
        let mut best_ask: Option<(ExchangeId, Decimal)> = None;

        for exchange in self.get_supported_exchanges() {
            if let Some(order_book) = market_data.get_order_book(exchange, symbol) {
                if let Some(best_ask_level) = order_book.best_ask() {
                    match best_ask {
                        None => best_ask = Some((exchange, best_ask_level.price)),
                        Some((_, current_best)) => {
                            if best_ask_level.price < current_best {
                                best_ask = Some((exchange, best_ask_level.price));
                            }
                        }
                    }
                }
            }
        }

        best_ask
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

        // Convert to basis points (1% = 100 bps)
        let profit_decimal = profit_ratio * Decimal::from(10000);
        let profit_bps = profit_decimal.to_i32().ok_or_else(|| {
            ArbitrageError::Calculation(format!("Profit calculation overflow: {}", profit_decimal))
        })?;

        Ok(profit_bps)
    }

    /// Get available liquidity for the arbitrage opportunity
    fn get_available_liquidity(
        &self,
        market_data: &MarketBundle,
        symbol: &Symbol,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
    ) -> Result<Decimal> {
        let buy_book = market_data
            .get_order_book(buy_exchange, symbol)
            .ok_or_else(|| ArbitrageError::Validation("Buy order book not found".to_string()))?;

        let sell_book = market_data
            .get_order_book(sell_exchange, symbol)
            .ok_or_else(|| ArbitrageError::Validation("Sell order book not found".to_string()))?;

        // Get liquidity from both sides
        let buy_liquidity = buy_book
            .best_ask()
            .map(|level| level.quantity)
            .unwrap_or_else(|| Decimal::ZERO);

        let sell_liquidity = sell_book
            .best_bid()
            .map(|level| level.quantity)
            .unwrap_or_else(|| Decimal::ZERO);

        // Take minimum of both sides
        Ok(buy_liquidity.min(sell_liquidity))
    }

    /// Check if symbol is in allowed list
    fn is_symbol_allowed(&self, symbol: &Symbol) -> bool {
        if let Some(allowed_symbols) = self.config.custom_params.get("allowed_symbols") {
            if let Some(symbols_array) = allowed_symbols.as_array() {
                let symbol_str = symbol.to_pair();
                return symbols_array
                    .iter()
                    .any(|s| s.as_str() == Some(&symbol_str));
            }
        }

        // Default to allowing all symbols if not configured
        true
    }

    /// Get custom parameter as u64
    fn get_custom_param_u64(&self, key: &str, default: u64) -> u64 {
        self.config
            .custom_params
            .get(key)
            .and_then(|v| v.as_u64())
            .unwrap_or(default)
    }

    /// Get custom parameter as Decimal
    fn get_custom_param_decimal(&self, key: &str, default: Decimal) -> Decimal {
        self.config
            .custom_params
            .get(key)
            .and_then(|v| v.as_f64())
            .and_then(|f| Decimal::try_from(f).ok())
            .unwrap_or(default)
    }

    /// Estimate net profit after fees
    fn estimate_net_profit_bps(
        &self,
        gross_profit_bps: i32,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
    ) -> i32 {
        let (_, buy_taker_fee) = ExchangeCapabilities::get_typical_fees(buy_exchange);
        let (_, sell_taker_fee) = ExchangeCapabilities::get_typical_fees(sell_exchange);

        // Fees are returned as percentages (e.g., 0.10 = 0.10%), convert to basis points
        let total_fee_bps = ((buy_taker_fee + sell_taker_fee) * 100.0) as i32;

        // Net profit = gross profit - fees
        gross_profit_bps - total_fee_bps
    }
}

impl Default for CexArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for CexArbitrageStrategy {
    fn id(&self) -> &'static str {
        "cex_arbitrage"
    }

    fn name(&self) -> &'static str {
        "CEX ↔ CEX Price Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();

        // Get all unique symbols from market data
        let symbols = market_data.get_all_symbols();

        debug!("CEX arbitrage scanning {} symbols", symbols.len());
        println!(
            "CEX arbitrage scanning {} symbols: {:?}",
            symbols.len(),
            symbols
        );

        for symbol in symbols {
            println!("Processing symbol: {}", symbol);

            // Skip if symbol not allowed
            if !self.is_symbol_allowed(&symbol) {
                println!("Symbol {} not allowed", symbol);
                continue;
            }

            // Find best bid and ask across all exchanges
            let best_bid = match self.find_best_bid(market_data, &symbol) {
                Some(bid) => {
                    println!("Best bid for {}: {} @ {}", symbol, bid.0, bid.1);
                    bid
                }
                None => {
                    println!("No bids found for {}", symbol);
                    debug!("No bids found for {}", symbol);
                    continue;
                }
            };

            let best_ask = match self.find_best_ask(market_data, &symbol) {
                Some(ask) => {
                    println!("Best ask for {}: {} @ {}", symbol, ask.0, ask.1);
                    ask
                }
                None => {
                    println!("No asks found for {}", symbol);
                    debug!("No asks found for {}", symbol);
                    continue;
                }
            };

            let (sell_exchange, sell_price) = best_bid;
            let (buy_exchange, buy_price) = best_ask;

            println!(
                "Arbitrage opportunity: Buy {} @ {} on {}, Sell {} @ {} on {}",
                symbol, buy_price, buy_exchange, symbol, sell_price, sell_exchange
            );

            // Skip if same exchange (no arbitrage possible)
            if buy_exchange == sell_exchange {
                println!("Same exchange, skipping");
                continue;
            }

            // Calculate gross profit
            let gross_profit_bps = match self.calculate_gross_profit_bps(buy_price, sell_price) {
                Ok(profit) => {
                    println!("Gross profit: {} bps", profit);
                    profit
                }
                Err(e) => {
                    println!("Failed to calculate profit: {}", e);
                    warn!("Failed to calculate profit for {}: {}", symbol, e);
                    continue;
                }
            };

            // Skip if no profit
            if gross_profit_bps <= 0 {
                println!("No profit: {} bps", gross_profit_bps);
                continue;
            }

            // Estimate net profit after fees
            let net_profit_bps =
                self.estimate_net_profit_bps(gross_profit_bps, buy_exchange, sell_exchange);
            println!(
                "Net profit after fees: {} bps (min required: {} bps)",
                net_profit_bps, self.config.min_profit_bps
            );

            // Skip if net profit is too low
            if net_profit_bps < self.config.min_profit_bps {
                println!(
                    "Net profit too low: {}bps < {}bps",
                    net_profit_bps, self.config.min_profit_bps
                );
                debug!(
                    "Net profit too low for {}: {}bps < {}bps",
                    symbol, net_profit_bps, self.config.min_profit_bps
                );
                continue;
            }

            // Get available liquidity
            let liquidity = match self.get_available_liquidity(
                market_data,
                &symbol,
                buy_exchange,
                sell_exchange,
            ) {
                Ok(liq) => {
                    println!("Available liquidity: {}", liq);
                    liq
                }
                Err(e) => {
                    println!("Failed to get liquidity: {}", e);
                    warn!("Failed to get liquidity for {}: {}", symbol, e);
                    continue;
                }
            };

            // Skip if insufficient liquidity
            let min_notional = self.get_custom_param_decimal("min_notional_usd", Decimal::from(10));
            let min_quantity = min_notional.checked_div(buy_price).unwrap_or(Decimal::ZERO);

            println!(
                "Min notional: {} USD, Min quantity: {}, Available: {}",
                min_notional, min_quantity, liquidity
            );

            if liquidity < min_quantity {
                println!("Insufficient liquidity: {} < {}", liquidity, min_quantity);
                debug!(
                    "Insufficient liquidity for {}: {} < {}",
                    symbol, liquidity, min_quantity
                );
                continue;
            }

            println!("Creating signal...");

            // Create raw signal
            let mut signal = RawSignal::new(self.id(), symbol.clone());

            // Add buy leg
            let buy_leg = TradeLeg::new(
                buy_exchange,
                symbol.clone(),
                Side::Buy,
                buy_price,
                liquidity,
            );
            signal.add_leg(buy_leg);

            // Add sell leg
            let sell_leg = TradeLeg::new(
                sell_exchange,
                symbol.clone(),
                Side::Sell,
                sell_price,
                liquidity,
            );
            signal.add_leg(sell_leg);

            // Set net profit (after fees)
            signal.set_profit_bps(net_profit_bps);

            // Add metadata
            signal.add_metadata("buy_exchange", json!(buy_exchange.to_string()));
            signal.add_metadata("sell_exchange", json!(sell_exchange.to_string()));
            signal.add_metadata("buy_price", json!(buy_price.to_string()));
            signal.add_metadata("sell_price", json!(sell_price.to_string()));
            signal.add_metadata("liquidity", json!(liquidity.to_string()));
            signal.add_metadata("gross_profit_bps", json!(gross_profit_bps));
            signal.add_metadata("net_profit_bps", json!(net_profit_bps));
            signal.add_metadata(
                "estimated_fees_bps",
                json!(gross_profit_bps - net_profit_bps),
            );

            signals.push(signal);

            println!("Signal created and added! Total signals: {}", signals.len());

            debug!(
                "CEX arbitrage signal: {} buy@{} {} sell@{} {} liquidity={} net_profit={}bps",
                symbol,
                buy_exchange,
                buy_price,
                sell_exchange,
                sell_price,
                liquidity,
                net_profit_bps
            );
        }

        println!("Final signal count: {}", signals.len());
        debug!("CEX arbitrage detected {} signals", signals.len());
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

        // Must have exactly 2 legs (buy + sell)
        if signal.legs.len() != 2 {
            println!("Signal has {} legs, expected 2", signal.legs.len());
            return Ok(false);
        }

        let buy_leg = &signal.legs[0];
        let sell_leg = &signal.legs[1];

        println!(
            "Buy leg: {} {:?} @ {}, Sell leg: {} {:?} @ {}",
            buy_leg.exchange,
            buy_leg.side,
            buy_leg.price,
            sell_leg.exchange,
            sell_leg.side,
            sell_leg.price
        );

        // Validate exchanges are allowed
        if !context.are_exchanges_allowed(buy_leg.exchange, sell_leg.exchange) {
            println!(
                "Exchanges not allowed: {} and {}",
                buy_leg.exchange, sell_leg.exchange
            );
            return Ok(false);
        }

        // Check minimum profit threshold (already checked in detect, but double-check)
        if signal.expected_profit_bps < context.min_profit_bps {
            println!(
                "Profit too low: {} < {}",
                signal.expected_profit_bps, context.min_profit_bps
            );
            return Ok(false);
        }

        // Check inventory limits if inventory-based
        let inventory_based = self
            .config
            .custom_params
            .get("inventory_based")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        println!("Inventory based: {}", inventory_based);

        if inventory_based {
            let base_asset = &signal.symbol.base;
            let quote_asset = &signal.symbol.quote;

            println!(
                "Checking inventory for base: {}, quote: {}",
                base_asset, quote_asset
            );

            // Check if we can sell base asset on sell exchange
            if !context.can_sell(sell_leg.exchange, base_asset, sell_leg.quantity) {
                println!(
                    "Cannot sell {} {} on {}",
                    sell_leg.quantity, base_asset, sell_leg.exchange
                );
                return Ok(false);
            }

            // Check if we can buy with quote asset on buy exchange
            let required_quote = buy_leg.price * buy_leg.quantity;
            if !context.can_sell(buy_leg.exchange, quote_asset, required_quote) {
                println!(
                    "Cannot buy with {} {} on {}",
                    required_quote, quote_asset, buy_leg.exchange
                );
                return Ok(false);
            }
        }

        // Check maximum exposure
        let total_notional = signal.total_notional();
        if total_notional > context.max_exposure {
            return Ok(false);
        }

        // Check latency constraints
        let max_latency_ms =
            self.get_custom_param_u64("max_latency_ms", CexArbitrageDefaults::MAX_LATENCY_MS);
        if context.max_latency_ms > max_latency_ms {
            return Ok(false);
        }

        // Check minimum notional
        let min_notional = self.get_custom_param_decimal(
            "min_notional_usd",
            Decimal::try_from(CexArbitrageDefaults::MIN_NOTIONAL_USD)
                .unwrap_or_else(|_| Decimal::from(10)),
        );
        if total_notional < min_notional {
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
