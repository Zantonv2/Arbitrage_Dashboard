#![allow(clippy::type_complexity)]

use crate::strategies::strategies_specifics::{ExchangeCapabilities, StrategyLimits};
use crate::strategies::{
    FilterContext, MarketBundle, RawSignal, RiskLimits, Strategy, StrategyConfig, TradeLeg,
};
use crate::{ExchangeId, Result, Side, Symbol};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Temporal (Latency) Arbitrage Strategy
///
/// **Principle**: Exploit short-lived price differences caused by slower market data
/// propagation between exchanges. React faster than other market participants to
/// price movements.
///
/// **Focus**:
/// - Ultra-low latency signal detection (sub-100ms)
/// - Price movement prediction based on leading indicators
/// - Exploit data feed delays between exchanges
///
/// **Supported Exchanges**: All 6 exchanges (OKX, Bybit, MEXC, Gate.io, Bitstamp, Kraken)
///
/// **Data Sources**:
/// - Real-time price feeds from all exchanges
/// - Order book depth for execution validation
/// - Historical price movement patterns
pub struct LatencyArbitrageStrategy {
    config: StrategyConfig,
    #[allow(dead_code)]
    /// Track price history for staleness detection
    price_history: HashMap<(ExchangeId, Symbol), Vec<(DateTime<Utc>, Decimal)>>,
}

impl LatencyArbitrageStrategy {
    /// Create a new latency arbitrage strategy with default configuration
    pub fn new() -> Self {
        let custom_params = json!({
            "min_latency_spread_bps": 20,
            "max_staleness_ms": 500,
            "min_price_movement_bps": 5,
            "correlation_window_seconds": 10,
            "price_staleness_threshold_ms": 100
        });

        let custom_params: std::collections::HashMap<String, serde_json::Value> =
            if let Some(obj) = custom_params.as_object() {
                obj.clone().into_iter().collect()
            } else {
                std::collections::HashMap::new()
            };

        Self {
            config: StrategyConfig {
                enabled: true,
                min_profit_bps: StrategyLimits::get_min_profit_bps("latency_arbitrage"),
                max_exposure: StrategyLimits::get_max_exposure("latency_arbitrage"),
                confidence_threshold: Decimal::new(7, 1),
                risk_limits: RiskLimits::default(),
                custom_params,
            },
            price_history: HashMap::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: StrategyConfig) -> Self {
        Self {
            config,
            price_history: HashMap::new(),
        }
    }

    /// Get exchanges ordered by typical latency (fastest first)
    fn get_exchanges_by_latency(&self) -> Vec<ExchangeId> {
        // Order exchanges by typical latency characteristics
        vec![
            ExchangeId::OKX,      // Typically fastest
            ExchangeId::ByBit,    // Fast
            ExchangeId::MEXC,     // Medium
            ExchangeId::GateIo,   // Medium-slow
            ExchangeId::Kraken,   // Slower
            ExchangeId::Bitstamp, // Typically slowest
        ]
    }
}

impl Default for LatencyArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl Strategy for LatencyArbitrageStrategy {
    fn id(&self) -> &'static str {
        "latency_arbitrage"
    }

    fn name(&self) -> &'static str {
        "Latency Arbitrage"
    }

    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::new();

        // Get all unique symbols
        let symbols = market_data.get_all_symbols();

        for symbol in symbols {
            // Look for price discrepancies between exchanges that might indicate
            // latency-based opportunities
            let mut exchange_prices: Vec<(ExchangeId, Decimal)> = Vec::new();

            // Collect prices from all exchanges
            for exchange in self.get_exchanges_by_latency() {
                if let Some(ticker) = market_data.get_ticker(exchange, Arc::clone(&symbol)) {
                    let mid_price = (ticker.bid + ticker.ask) / Decimal::from(2);
                    exchange_prices.push((exchange, mid_price));
                }
            }

            if exchange_prices.len() < 2 {
                continue;
            }

            // Find the highest and lowest prices
            let (max_exchange, max_price) = exchange_prices
                .iter()
                .max_by_key(|(_, price)| *price)
                .copied()
                .unwrap();
            let (min_exchange, min_price) = exchange_prices
                .iter()
                .min_by_key(|(_, price)| *price)
                .copied()
                .unwrap();

            if max_exchange == min_exchange {
                continue;
            }

            // Calculate price difference
            let price_diff_bps = ((max_price - min_price) / min_price * Decimal::from(10000))
                .to_i32()
                .unwrap_or(0);

            // Check if the difference is significant enough for latency arbitrage
            let min_latency_spread_bps = self
                .config
                .custom_params
                .get("min_latency_spread_bps")
                .and_then(|v| v.as_i64())
                .unwrap_or(20) as i32; // 0.20% minimum spread

            if price_diff_bps >= min_latency_spread_bps {
                // Get order books for liquidity check
                let buy_book = market_data.get_order_book(min_exchange, &symbol);
                let sell_book = market_data.get_order_book(max_exchange, &symbol);

                if let (Some(buy_book), Some(sell_book)) = (buy_book, sell_book) {
                    if let (Some(ask_level), Some(bid_level)) =
                        (buy_book.best_ask(), sell_book.best_bid())
                    {
                        // Estimate fees
                        let (_, buy_fee) = ExchangeCapabilities::get_typical_fees(min_exchange);
                        let (_, sell_fee) = ExchangeCapabilities::get_typical_fees(max_exchange);
                        let total_fee_bps = ((buy_fee + sell_fee) * 100.0) as i32;

                        let net_profit_bps = price_diff_bps - total_fee_bps;

                        if net_profit_bps >= self.config.min_profit_bps {
                            let quantity = ask_level.quantity.min(bid_level.quantity);
                            let min_quantity = Decimal::from(100) / ask_level.price; // $100 minimum

                            if quantity >= min_quantity {
                                // Create latency arbitrage signal
                                let mut signal = RawSignal::new(self.id(), (*symbol).clone());

                                // Buy from slower/cheaper exchange
                                let buy_leg = TradeLeg::new(
                                    min_exchange,
                                    (*symbol).clone(),
                                    Side::Buy,
                                    ask_level.price,
                                    quantity.min(min_quantity * Decimal::from(5)), // Limit size
                                );
                                signal.add_leg(buy_leg);

                                // Sell to faster/expensive exchange
                                let sell_leg = TradeLeg::new(
                                    max_exchange,
                                    (*symbol).clone(),
                                    Side::Sell,
                                    bid_level.price,
                                    quantity.min(min_quantity * Decimal::from(5)), // Limit size
                                );
                                signal.add_leg(sell_leg);

                                signal.set_profit_bps(net_profit_bps);

                                // Add metadata
                                signal.add_metadata("arbitrage_type", json!("latency"));
                                signal
                                    .add_metadata("slow_exchange", json!(min_exchange.to_string()));
                                signal
                                    .add_metadata("fast_exchange", json!(max_exchange.to_string()));
                                signal.add_metadata("price_spread_bps", json!(price_diff_bps));
                                signal.add_metadata("estimated_fees_bps", json!(total_fee_bps));

                                signals.push(signal);
                            }
                        }
                    }
                }
            }
        }

        Ok(signals)
    }

    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Basic validation
        if !signal.is_valid() {
            return Ok(false);
        }

        // Must have exactly 2 legs (buy + sell)
        if signal.legs.len() != 2 {
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

        // Check inventory for both legs
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
