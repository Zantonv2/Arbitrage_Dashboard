use crate::strategies::strategies_specifics::{
    ConfigExtensions, ExchangeCapabilities, StablecoinDefaults, StrategyLimits,
};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ArbitrageError, ExchangeId, Result, Side, Symbol};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;

pub const MIN_PEG_DEVIATION_PCT_STR: &str = "0.001";
pub const MAX_PEG_DEVIATION_PCT_STR: &str = "0.02";

/// Stablecoin Peg Arbitrage Strategy
///
/// **Principle**: Trade temporary deviations between stablecoins and their target peg.
/// Exploit price differences when stablecoins deviate from their $1.00 target.
///
/// **Focus**:
/// - Monitor stablecoin prices across all exchanges
/// - Detect deviations from $1.00 peg (both premium and discount)
/// - Account for trading fees and liquidity
/// - Focus on major stablecoins: USDT, USDC, BUSD, DAI, TUSD
///
/// **Supported Exchanges**: All 6 exchanges (OKX, Bybit, MEXC, Gate.io, Bitstamp, Kraken)
///
/// **Data Sources**:
/// - Stablecoin tickers from all exchanges
/// - Order book depth for liquidity validation
/// - Cross-stablecoin pairs (USDT/USDC, etc.)
#[derive(Debug, Clone)]
pub struct StablecoinArbitrageStrategy {
    config: StrategyConfig,
}

impl StablecoinArbitrageStrategy {
    /// Create a new stablecoin arbitrage strategy with default configuration
    pub fn new() -> Self {
        Self {
            config: StrategyConfig {
                min_profit_bps: StrategyLimits::get_min_profit_bps("stablecoin_arbitrage"),
                max_exposure: StrategyLimits::get_max_exposure("stablecoin_arbitrage"),
                custom_params: StablecoinDefaults::get_custom_params(),
                ..Default::default()
            },
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        Self { config }
    }

    /// Get supported exchanges for stablecoin arbitrage
    fn get_supported_exchanges(&self) -> Vec<ExchangeId> {
        ExchangeCapabilities::get_cex_arbitrage_exchanges()
    }

    /// Get configured peg targets
    fn get_peg_targets(&self) -> HashMap<String, Decimal> {
        let mut targets = HashMap::new();

        if let Some(peg_targets) = self.config.custom_params.get("peg_targets") {
            if let Some(targets_obj) = peg_targets.as_object() {
                for (coin, target_val) in targets_obj {
                    if let Some(target_str) = target_val.as_str() {
                        if let Ok(target_decimal) = Decimal::from_str_exact(target_str) {
                            targets.insert(coin.clone(), target_decimal);
                        }
                    }
                }
            }
        }

        // Default targets if not configured
        if targets.is_empty() {
            targets.insert("USDT".to_string(), Decimal::ONE);
            targets.insert("USDC".to_string(), Decimal::ONE);
            targets.insert("BUSD".to_string(), Decimal::ONE);
            targets.insert("DAI".to_string(), Decimal::ONE);
            targets.insert("TUSD".to_string(), Decimal::ONE);
        }

        targets
    }

    /// Check if a symbol is a stablecoin pair
    fn is_stablecoin_pair(&self, symbol: &Symbol) -> bool {
        let peg_targets = self.get_peg_targets();

        // Check if base is stablecoin and quote is USD/USDT/USDC
        let base_is_stable = peg_targets.contains_key(&symbol.base);
        let quote_is_reference = matches!(symbol.quote.as_str(), "USD" | "USDT" | "USDC");

        base_is_stable && quote_is_reference
    }

    /// Calculate peg deviation in basis points
    fn calculate_peg_deviation_bps(&self, symbol: &Symbol, price: Decimal) -> Result<i32> {
        let peg_targets = self.get_peg_targets();

        let target_price = peg_targets
            .get(&symbol.base)
            .copied()
            .unwrap_or(Decimal::ONE);

        if target_price.is_zero() {
            return Err(ArbitrageError::Calculation(
                "Target price cannot be zero".to_string(),
            ));
        }

        let deviation = (price - target_price)
            .checked_div(target_price)
            .ok_or_else(|| {
                ArbitrageError::Calculation("Division by zero in peg deviation".to_string())
            })?;

        let deviation_bps = deviation
            .checked_mul(Decimal::from(10000))
            .ok_or_else(|| ArbitrageError::Calculation("Peg deviation overflow".to_string()))?
            .to_i32()
            .ok_or_else(|| {
                ArbitrageError::Calculation("Peg deviation conversion failed".to_string())
            })?;

        Ok(deviation_bps)
    }

    /// Check if peg deviation is within acceptable range
    fn is_deviation_valid(&self, deviation_bps: i32) -> bool {
        let min_deviation_pct = self
            .config
            .custom_params
            .get_decimal("min_peg_deviation_pct")
            .unwrap_or_else(|| {
                Decimal::from_str_exact(MIN_PEG_DEVIATION_PCT_STR).unwrap_or(Decimal::new(1, 3))
            });

        let max_deviation_pct = self
            .config
            .custom_params
            .get_decimal("max_peg_deviation_pct")
            .unwrap_or_else(|| {
                Decimal::from_str_exact(MAX_PEG_DEVIATION_PCT_STR).unwrap_or(Decimal::new(2, 2))
            });

        let min_deviation_bps = min_deviation_pct
            .checked_mul(Decimal::from(100))
            .unwrap_or(Decimal::ZERO)
            .to_i32()
            .unwrap_or(0);
        let max_deviation_bps = max_deviation_pct
            .checked_mul(Decimal::from(100))
            .unwrap_or(Decimal::ZERO)
            .to_i32()
            .unwrap_or(0);

        let abs_deviation = deviation_bps.abs();
        abs_deviation >= min_deviation_bps && abs_deviation <= max_deviation_bps
    }

    /// Find arbitrage opportunities between stablecoin and peg
    fn find_peg_arbitrage(
        &self,
        market_data: &MarketBundle,
        symbol: &Symbol,
        exchange: ExchangeId,
    ) -> Option<RawSignal> {
        let ticker = market_data.get_ticker(exchange, symbol)?;

        // Use mid price for peg comparison
        let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);

        // Calculate peg deviation
        let deviation_bps = match self.calculate_peg_deviation_bps(symbol, mid_price) {
            Ok(dev) => dev,
            Err(_e) => {
                return None;
            }
        };

        // Check if deviation is significant enough
        if !self.is_deviation_valid(deviation_bps) {
            return None;
        }

        // Get order book for liquidity check
        let order_book = market_data.get_order_book(exchange, symbol)?;

        // Determine trade direction based on deviation
        let (side, price, expected_profit_bps) = if deviation_bps > 0 {
            // Stablecoin trading above peg - sell it
            let best_bid = order_book.best_bid()?;
            (Side::Sell, best_bid.price, deviation_bps)
        } else {
            // Stablecoin trading below peg - buy it
            let best_ask = order_book.best_ask()?;
            (Side::Buy, best_ask.price, deviation_bps.abs())
        };

        // Get available liquidity
        let liquidity = match side {
            Side::Buy => order_book.best_ask()?.quantity,
            Side::Sell => order_book.best_bid()?.quantity,
        };

        // Check minimum liquidity
        let min_notional = Decimal::from(100); // $100 minimum
        let min_quantity = min_notional.checked_div(price).unwrap_or(Decimal::ZERO);

        if liquidity < min_quantity {
            return None;
        }

        // Estimate net profit after fees
        let (_, taker_fee) = ExchangeCapabilities::get_typical_fees(exchange);
        let fee_bps = Decimal::try_from(taker_fee)
            .ok()
            .and_then(|fee| fee.checked_mul(Decimal::from(100)))
            .unwrap_or(Decimal::ZERO)
            .to_i32()
            .unwrap_or(0);
        let net_profit_bps = expected_profit_bps - fee_bps;

        if net_profit_bps < self.config.min_profit_bps {
            return None;
        }

        // Create signal
        let mut signal = RawSignal::new(self.id(), symbol.clone());

        let trade_leg = TradeLeg::new(
            exchange,
            symbol.clone(),
            side,
            price,
            liquidity.min(min_quantity * Decimal::from(10)), // Cap at 10x minimum
        );
        signal.add_leg(trade_leg);
        signal.set_profit_bps(net_profit_bps);

        // Add metadata
        signal.add_metadata("peg_deviation_bps", json!(deviation_bps));
        signal.add_metadata("mid_price", json!(mid_price.to_string()));
        signal.add_metadata("target_price", json!("1.0"));
        signal.add_metadata("trade_direction", json!(side.to_string()));
        signal.add_metadata("estimated_fees_bps", json!(fee_bps));

        Some(signal)
    }

    /// Find cross-stablecoin arbitrage opportunities
    fn find_cross_stablecoin_arbitrage(&self, market_data: &MarketBundle) -> Vec<RawSignal> {
        let mut signals = Vec::new();

        // Look for USDT/USDC, USDT/DAI, USDC/DAI pairs etc.
        let stablecoins = vec!["USDT", "USDC", "BUSD", "DAI"];

        for base in &stablecoins {
            for quote in &stablecoins {
                if base == quote {
                    continue;
                }

                let symbol = Symbol::new(*base, *quote);

                // Find best prices across exchanges
                let mut best_bid: Option<(ExchangeId, Decimal, Decimal)> = None;
                let mut best_ask: Option<(ExchangeId, Decimal, Decimal)> = None;

                for exchange in self.get_supported_exchanges() {
                    if let Some(order_book) = market_data.get_order_book(exchange, &symbol) {
                        if let Some(bid_level) = order_book.best_bid() {
                            match best_bid {
                                None => {
                                    best_bid = Some((exchange, bid_level.price, bid_level.quantity))
                                }
                                Some((_, current_price, _)) => {
                                    if bid_level.price > current_price {
                                        best_bid =
                                            Some((exchange, bid_level.price, bid_level.quantity));
                                    }
                                }
                            }
                        }

                        if let Some(ask_level) = order_book.best_ask() {
                            match best_ask {
                                None => {
                                    best_ask = Some((exchange, ask_level.price, ask_level.quantity))
                                }
                                Some((_, current_price, _)) => {
                                    if ask_level.price < current_price {
                                        best_ask =
                                            Some((exchange, ask_level.price, ask_level.quantity));
                                    }
                                }
                            }
                        }
                    }
                }

                // Check for arbitrage opportunity
                if let (
                    Some((sell_exchange, sell_price, sell_qty)),
                    Some((buy_exchange, buy_price, buy_qty)),
                ) = (best_bid, best_ask)
                {
                    if sell_exchange != buy_exchange && sell_price > buy_price {
                        // Calculate profit
                        let profit_ratio = (sell_price - buy_price)
                            .checked_div(buy_price)
                            .unwrap_or(Decimal::ZERO);
                        let profit_bps = profit_ratio
                            .checked_mul(Decimal::from(10000))
                            .unwrap_or(Decimal::ZERO)
                            .to_i32()
                            .unwrap_or(0);

                        // Estimate fees
                        let (_, buy_fee) = ExchangeCapabilities::get_typical_fees(buy_exchange);
                        let (_, sell_fee) = ExchangeCapabilities::get_typical_fees(sell_exchange);
                        let total_fee_bps = Decimal::try_from(buy_fee + sell_fee)
                            .ok()
                            .and_then(|f| f.checked_mul(Decimal::from(100)))
                            .unwrap_or(Decimal::ZERO)
                            .to_i32()
                            .unwrap_or(0);
                        let net_profit_bps = profit_bps - total_fee_bps;

                        if net_profit_bps >= self.config.min_profit_bps {
                            let mut signal = RawSignal::new(self.id(), symbol.clone());

                            let quantity = sell_qty.min(buy_qty);

                            // Buy leg
                            let buy_leg = TradeLeg::new(
                                buy_exchange,
                                symbol.clone(),
                                Side::Buy,
                                buy_price,
                                quantity,
                            );
                            signal.add_leg(buy_leg);

                            // Sell leg
                            let sell_leg = TradeLeg::new(
                                sell_exchange,
                                symbol.clone(),
                                Side::Sell,
                                sell_price,
                                quantity,
                            );
                            signal.add_leg(sell_leg);

                            signal.set_profit_bps(net_profit_bps);

                            // Add metadata
                            signal.add_metadata("arbitrage_type", json!("cross_stablecoin"));
                            signal.add_metadata("buy_exchange", json!(buy_exchange.to_string()));
                            signal.add_metadata("sell_exchange", json!(sell_exchange.to_string()));
                            signal.add_metadata("gross_profit_bps", json!(profit_bps));

                            signals.push(signal);
                        }
                    }
                }
            }
        }

        signals
    }
}

impl Default for StablecoinArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for StablecoinArbitrageStrategy {
    fn id(&self) -> &'static str {
        "stablecoin_arbitrage"
    }

    fn name(&self) -> &'static str {
        "Stablecoin Peg Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();

        // 1. Find peg deviation opportunities
        for (exchange, symbol) in market_data.tickers.keys() {
            if !self.is_stablecoin_pair(symbol) {
                continue;
            }

            if let Some(signal) = self.find_peg_arbitrage(market_data, symbol, *exchange) {
                signals.push(signal);
            }
        }

        // 2. Find cross-stablecoin arbitrage opportunities
        let cross_signals = self.find_cross_stablecoin_arbitrage(market_data);
        signals.extend(cross_signals);

        Ok(signals)
    }

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have 1 or 2 legs (peg arbitrage or cross-stablecoin)
        if signal.legs.is_empty() || signal.legs.len() > 2 {
            return Ok(false);
        }

        // Validate all exchanges are allowed
        for leg in &signal.legs {
            if !context.is_exchange_allowed(leg.exchange) {
                return Ok(false);
            }
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

        // Check inventory for all legs
        for leg in &signal.legs {
            match leg.side {
                Side::Sell => {
                    // For sell orders, check if we have enough of the base asset
                    let base_asset = &signal.symbol.base;
                    if !context.can_sell(leg.exchange, base_asset, leg.quantity) {
                        return Ok(false);
                    }
                }
                Side::Buy => {
                    // For buy orders, check if we have enough quote currency
                    let quote_asset = &signal.symbol.quote;
                    let required_quote =
                        leg.price.checked_mul(leg.quantity).unwrap_or(Decimal::ZERO);
                    if !context.can_sell(leg.exchange, quote_asset, required_quote) {
                        return Ok(false);
                    }
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
