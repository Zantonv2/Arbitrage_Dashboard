use crate::constants::BASIS_POINTS_DIVISOR;
use crate::market_utils;
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
///
/// # Example
///
/// ```rust
/// use arbitrage_core::strategies::StrategyConfig;
///
/// let strategy = CexArbitrageStrategy::new();
/// assert_eq!(strategy.id(), "cex_arbitrage");
/// assert_eq!(strategy.name(), "CEX ↔ CEX Price Arbitrage");
/// ```
pub struct CexArbitrageStrategy {
    config: StrategyConfig,
}

impl CexArbitrageStrategy {
    /// Creates a new CEX arbitrage strategy with default configuration.
    ///
    /// Initializes the strategy with default limits and parameters for
    /// CEX-to-CEX arbitrage trading. The strategy uses predefined minimum
    /// profit thresholds and maximum exposure limits.
    ///
    /// # Returns
    ///
    /// A new CexArbitrageStrategy instance with default configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// let strategy = CexArbitrageStrategy::new();
    /// assert!(strategy.config().enabled);
    /// ```
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

    /// Creates a CEX arbitrage strategy with custom configuration.
    ///
    /// Allows overriding the default configuration with custom parameters
    /// for fine-tuned control over strategy behavior, including custom
    /// min profit thresholds, exposure limits, and symbol filters.
    ///
    /// # Arguments
    ///
    /// * `config` - Custom strategy configuration
    ///
    /// # Returns
    ///
    /// A new CexArbitrageStrategy instance with the provided configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::strategies::{StrategyConfig, RiskLimits};
    /// use rust_decimal::Decimal;
    ///
    /// let config = StrategyConfig {
    ///     enabled: true,
    ///     min_profit_bps: 10,
    ///     max_exposure: Decimal::from(50000),
    ///     confidence_threshold: Decimal::new(9, 1),
    ///     risk_limits: RiskLimits::default(),
    ///     custom_params: serde_json::Map::new(),
    /// };
    ///
    /// let strategy = CexArbitrageStrategy::with_config(config);
    /// ```
    pub fn with_config(config: StrategyConfig) -> Self {
        Self { config }
    }

    /// Get supported exchanges for CEX arbitrage
    fn get_supported_exchanges(&self) -> Vec<ExchangeId> {
        ExchangeCapabilities::get_cex_arbitrage_exchanges()
    }

    /// Calculate gross profit percentage before fees
    fn calculate_gross_profit_bps(&self, buy_price: Decimal, sell_price: Decimal) -> Result<i32> {
        if buy_price <= Decimal::ZERO {
            return Err(ArbitrageError::Validation(format!(
                "Buy price must be positive, got {}",
                buy_price
            )));
        }

        if sell_price <= Decimal::ZERO {
            return Err(ArbitrageError::Validation(format!(
                "Sell price must be positive, got {}",
                sell_price
            )));
        }

        let profit_ratio = (sell_price - buy_price)
            .checked_div(buy_price)
            .ok_or_else(|| {
                ArbitrageError::Calculation("Division by zero in profit calculation".to_string())
            })?;

        // Convert to basis points (1% = 100 bps)
        let profit_decimal = profit_ratio * Decimal::from(BASIS_POINTS_DIVISOR);
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
    ///
    /// Calculates net profit by subtracting combined taker fees from gross profit.
    /// Fees are converted from percentage to basis points using checked arithmetic
    /// to prevent overflow for unusual fee values.
    ///
    /// # Arguments
    ///
    /// * `gross_profit_bps` - Gross profit in basis points
    /// * `buy_exchange` - Exchange for buy leg
    /// * `sell_exchange` - Exchange for sell leg
    ///
    /// # Returns
    ///
    /// Net profit in basis points after fees, or an error if fee calculation overflows.
    pub(crate) fn estimate_net_profit_bps(
        &self,
        gross_profit_bps: i32,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
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

        let net_profit = gross_profit_bps.checked_sub(total_fee_bps).ok_or_else(|| {
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

        Ok(net_profit)
    }
}

impl Default for CexArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for CexArbitrageStrategy {
    /// Returns the unique identifier for this strategy.
    ///
    /// # Returns
    ///
    /// A static string slice identifying the strategy type.
    fn id(&self) -> &'static str {
        "cex_arbitrage"
    }

    /// Returns the human-readable name for this strategy.
    ///
    /// # Returns
    ///
    /// A static string slice containing the strategy name.
    fn name(&self) -> &'static str {
        "CEX ↔ CEX Price Arbitrage"
    }

    /// Detects arbitrage opportunities across all supported CEX exchanges.
    ///
    /// Scans all available symbols across connected exchanges to identify
    /// price discrepancies where buying on one exchange and selling on
    /// another would be profitable after accounting for fees.
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

            // Find best bid and ask across all exchanges using market_utils
            let best_bid = match market_utils::find_best_bid(market_data, &symbol, &self.get_supported_exchanges()) {
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

            let best_ask = match market_utils::find_best_ask(market_data, &symbol, &self.get_supported_exchanges()) {
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
                match self.estimate_net_profit_bps(gross_profit_bps, buy_exchange, sell_exchange) {
                    Ok(net) => {
                        println!(
                            "Net profit after fees: {} bps (min required: {} bps)",
                            net, self.config.min_profit_bps
                        );
                        net
                    }
                    Err(e) => {
                        println!("Failed to calculate net profit: {}", e);
                        warn!("Failed to calculate net profit for {}: {}", symbol, e);
                        continue;
                    }
                };

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
            let mut signal = RawSignal::new(self.id(), (*symbol).clone().into());

            // Add buy leg
            let buy_leg = TradeLeg::new(
                buy_exchange,
                (*symbol).clone().into(),
                Side::Buy,
                buy_price,
                liquidity,
            );
            signal.add_leg(buy_leg);

            // Add sell leg
            let sell_leg = TradeLeg::new(
                sell_exchange,
                (*symbol).clone().into(),
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

    /// Filters a signal based on additional context and risk checks.
    ///
    /// Validates that a detected signal meets all requirements for execution,
    /// including exchange permissions, inventory constraints, exposure limits,
    /// and latency constraints.
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
        if signal.legs.len() < 2 {
            return Ok(false);
        }

        debug_assert!(
            signal.legs.len() >= 2,
            "Signal should have at least 2 legs for CEX arbitrage"
        );

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

            debug_assert!(
                !sell_leg.quantity.is_zero(),
                "Sell leg quantity should be non-zero for inventory check"
            );
            debug_assert!(
                !(buy_leg.price * buy_leg.quantity).is_zero(),
                "Required quote amount should be non-zero for inventory check"
            );

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
        debug_assert!(
            total_notional > Decimal::ZERO,
            "Total notional should be positive for a valid signal"
        );

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
    /// min profit threshold, max exposure, and symbol filters.
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
