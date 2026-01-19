/// Strategy-specific configurations, constants, and utilities
///
/// This module contains strategy-specific parameters, default configurations,
// and helper functions that are used across different strategies.
use crate::ExchangeId;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Default configuration parameters for CEX arbitrage strategy
pub struct CexArbitrageDefaults;

impl CexArbitrageDefaults {
    /// Maximum allowed latency in milliseconds
    pub const MAX_LATENCY_MS: u64 = 500;

    /// Minimum notional value in USD
    pub const MIN_NOTIONAL_USD: f64 = 10.0;

    /// Maximum position size as percentage of portfolio
    pub const MAX_POSITION_PCT: f64 = 0.05; // 5%

    /// VWAP calculation quantity in USD
    pub const VWAP_QUANTITY_USD: f64 = 1000.0;

    /// Whether to use inventory-based arbitrage
    pub const INVENTORY_BASED: bool = true;

    /// Default allowed symbols for CEX arbitrage
    pub const ALLOWED_SYMBOLS: &'static [&'static str] = &[
        "BTC/USDT",
        "ETH/USDT",
        "BNB/USDT",
        "ADA/USDT",
        "SOL/USDT",
        "DOT/USDT",
        "MATIC/USDT",
        "AVAX/USDT",
        "LINK/USDT",
        "UNI/USDT",
    ];

    /// Get default custom parameters for CEX arbitrage
    pub fn get_custom_params() -> HashMap<String, Value> {
        let mut params = HashMap::new();
        params.insert("max_latency_ms".to_string(), json!(Self::MAX_LATENCY_MS));
        params.insert(
            "min_notional_usd".to_string(),
            json!(Self::MIN_NOTIONAL_USD),
        );
        params.insert(
            "max_position_pct".to_string(),
            json!(Self::MAX_POSITION_PCT),
        );
        params.insert(
            "vwap_quantity_usd".to_string(),
            json!(Self::VWAP_QUANTITY_USD),
        );
        params.insert("inventory_based".to_string(), json!(Self::INVENTORY_BASED));
        params.insert("allowed_symbols".to_string(), json!(Self::ALLOWED_SYMBOLS));
        params
    }
}

/// Default configuration parameters for funding rate arbitrage strategy
pub struct FundingRateDefaults;

impl FundingRateDefaults {
    /// Minimum funding rate to consider (in percentage)
    pub const MIN_FUNDING_RATE_PCT: f64 = 0.01; // 0.01% = 1 bps

    /// Maximum funding rate to consider (in percentage)
    pub const MAX_FUNDING_RATE_PCT: f64 = 1.0; // 1%

    /// Minimum time to next funding (in hours)
    pub const MIN_TIME_TO_FUNDING_HOURS: f64 = 0.5; // 30 minutes

    /// Maximum position size for funding arbitrage
    pub const MAX_FUNDING_POSITION_USD: f64 = 50000.0;

    /// Get default custom parameters for funding rate arbitrage
    pub fn get_custom_params() -> HashMap<String, Value> {
        let mut params = HashMap::new();
        params.insert(
            "min_funding_rate_pct".to_string(),
            json!(Self::MIN_FUNDING_RATE_PCT),
        );
        params.insert(
            "max_funding_rate_pct".to_string(),
            json!(Self::MAX_FUNDING_RATE_PCT),
        );
        params.insert(
            "min_time_to_funding_hours".to_string(),
            json!(Self::MIN_TIME_TO_FUNDING_HOURS),
        );
        params.insert(
            "max_funding_position_usd".to_string(),
            json!(Self::MAX_FUNDING_POSITION_USD),
        );
        params
    }
}

/// Default configuration parameters for stablecoin peg arbitrage
pub struct StablecoinDefaults;

impl StablecoinDefaults {
    /// Minimum peg deviation to consider (in percentage)
    pub const MIN_PEG_DEVIATION_PCT: f64 = 0.1; // 0.1% = 10 bps

    /// Maximum peg deviation to consider (in percentage)
    pub const MAX_PEG_DEVIATION_PCT: f64 = 2.0; // 2%

    /// Stablecoin peg targets
    pub const PEG_TARGETS: &'static [(&'static str, f64)] = &[
        ("USDT", 1.0),
        ("USDC", 1.0),
        ("BUSD", 1.0),
        ("DAI", 1.0),
        ("TUSD", 1.0),
    ];

    /// Get default custom parameters for stablecoin arbitrage
    pub fn get_custom_params() -> HashMap<String, Value> {
        let mut params = HashMap::new();
        params.insert(
            "min_peg_deviation_pct".to_string(),
            json!(Self::MIN_PEG_DEVIATION_PCT),
        );
        params.insert(
            "max_peg_deviation_pct".to_string(),
            json!(Self::MAX_PEG_DEVIATION_PCT),
        );

        let peg_targets: HashMap<String, f64> = Self::PEG_TARGETS
            .iter()
            .map(|(coin, target)| (coin.to_string(), *target))
            .collect();
        params.insert("peg_targets".to_string(), json!(peg_targets));

        params
    }
}

/// Exchange-specific configuration and capabilities
pub struct ExchangeCapabilities;

impl ExchangeCapabilities {
    /// Get supported exchanges for CEX arbitrage
    pub fn get_cex_arbitrage_exchanges() -> Vec<ExchangeId> {
        vec![
            ExchangeId::OKX,
            ExchangeId::ByBit,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
            ExchangeId::Bitstamp,
            ExchangeId::Kraken,
        ]
    }

    /// Get supported exchanges for funding rate arbitrage
    pub fn get_funding_rate_exchanges() -> Vec<ExchangeId> {
        vec![ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC]
    }

    /// Get supported exchanges for spot/perpetual arbitrage
    pub fn get_spot_perp_exchanges() -> Vec<ExchangeId> {
        vec![ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC]
    }

    /// Check if exchange supports perpetual contracts
    pub fn supports_perpetuals(exchange: ExchangeId) -> bool {
        matches!(
            exchange,
            ExchangeId::OKX | ExchangeId::ByBit | ExchangeId::MEXC
        )
    }

    /// Check if exchange supports funding rates
    pub fn supports_funding_rates(exchange: ExchangeId) -> bool {
        matches!(
            exchange,
            ExchangeId::OKX | ExchangeId::ByBit | ExchangeId::MEXC
        )
    }

    /// Get typical maker/taker fees for an exchange (in percentage)
    pub fn get_typical_fees(exchange: ExchangeId) -> (f64, f64) {
        match exchange {
            ExchangeId::OKX => (0.08, 0.10),      // 0.08% maker, 0.10% taker
            ExchangeId::ByBit => (0.10, 0.10),    // 0.10% maker, 0.10% taker
            ExchangeId::MEXC => (0.20, 0.20),     // 0.20% maker, 0.20% taker
            ExchangeId::GateIo => (0.20, 0.20),   // 0.20% maker, 0.20% taker
            ExchangeId::Bitstamp => (0.50, 0.50), // 0.50% maker, 0.50% taker
            ExchangeId::Kraken => (0.16, 0.26),   // 0.16% maker, 0.26% taker
            _ => (0.25, 0.25),                    // Default fallback
        }
    }
}

/// Strategy performance thresholds and limits
pub struct StrategyLimits;

impl StrategyLimits {
    /// Minimum profit threshold in basis points for different strategy types
    pub const CEX_ARBITRAGE_MIN_PROFIT_BPS: i32 = 10; // 0.10%
    pub const FUNDING_RATE_MIN_PROFIT_BPS: i32 = 5; // 0.05%
    pub const STABLECOIN_MIN_PROFIT_BPS: i32 = 5; // 0.05%
    pub const SPOT_PERP_MIN_PROFIT_BPS: i32 = 15; // 0.15%
    pub const LATENCY_MIN_PROFIT_BPS: i32 = 3; // 0.03%

    /// Maximum exposure limits in USD for different strategies
    pub const CEX_ARBITRAGE_MAX_EXPOSURE: f64 = 100000.0; // $100k
    pub const FUNDING_RATE_MAX_EXPOSURE: f64 = 200000.0; // $200k
    pub const STABLECOIN_MAX_EXPOSURE: f64 = 50000.0; // $50k
    pub const SPOT_PERP_MAX_EXPOSURE: f64 = 150000.0; // $150k
    pub const LATENCY_MAX_EXPOSURE: f64 = 25000.0; // $25k

    /// Get minimum profit threshold for a strategy
    pub fn get_min_profit_bps(strategy_id: &str) -> i32 {
        match strategy_id {
            "cex_arbitrage" => Self::CEX_ARBITRAGE_MIN_PROFIT_BPS,
            "funding_rate_arbitrage" => Self::FUNDING_RATE_MIN_PROFIT_BPS,
            "stablecoin_arbitrage" => Self::STABLECOIN_MIN_PROFIT_BPS,
            "spot_perp_arbitrage" => Self::SPOT_PERP_MIN_PROFIT_BPS,
            "latency_arbitrage" => Self::LATENCY_MIN_PROFIT_BPS,
            _ => Self::CEX_ARBITRAGE_MIN_PROFIT_BPS, // Default
        }
    }

    /// Get maximum exposure for a strategy
    pub fn get_max_exposure(strategy_id: &str) -> Decimal {
        let exposure = match strategy_id {
            "cex_arbitrage" => Self::CEX_ARBITRAGE_MAX_EXPOSURE,
            "funding_rate_arbitrage" => Self::FUNDING_RATE_MAX_EXPOSURE,
            "stablecoin_arbitrage" => Self::STABLECOIN_MAX_EXPOSURE,
            "spot_perp_arbitrage" => Self::SPOT_PERP_MAX_EXPOSURE,
            "latency_arbitrage" => Self::LATENCY_MAX_EXPOSURE,
            _ => Self::CEX_ARBITRAGE_MAX_EXPOSURE, // Default
        };

        Decimal::try_from(exposure).unwrap_or_else(|_| Decimal::from(10000))
    }
}

/// Utility functions for strategy configuration
pub struct StrategyUtils;

impl StrategyUtils {
    /// Create a custom parameter map with common defaults
    pub fn create_base_custom_params() -> HashMap<String, Value> {
        let mut params = HashMap::new();
        params.insert("enabled".to_string(), json!(true));
        params.insert("debug_mode".to_string(), json!(false));
        params.insert("max_signals_per_minute".to_string(), json!(60));
        params.insert("signal_cooldown_ms".to_string(), json!(1000));
        params
    }

    /// Merge custom parameters with defaults
    pub fn merge_custom_params(
        base: HashMap<String, Value>,
        overrides: HashMap<String, Value>,
    ) -> HashMap<String, Value> {
        let mut merged = base;
        for (key, value) in overrides {
            merged.insert(key, value);
        }
        merged
    }

    /// Validate custom parameter value
    pub fn validate_custom_param(key: &str, value: &Value) -> Result<(), String> {
        match key {
            "max_latency_ms" => {
                if let Some(val) = value.as_u64() {
                    if val > 5000 {
                        return Err("max_latency_ms cannot exceed 5000ms".to_string());
                    }
                } else {
                    return Err("max_latency_ms must be a positive integer".to_string());
                }
            }
            "min_notional_usd" => {
                if let Some(val) = value.as_f64() {
                    if !(1.0..=100000.0).contains(&val) {
                        return Err("min_notional_usd must be between 1 and 100000".to_string());
                    }
                } else {
                    return Err("min_notional_usd must be a positive number".to_string());
                }
            }
            "max_position_pct" => {
                if let Some(val) = value.as_f64() {
                    if !(0.001..=1.0).contains(&val) {
                        return Err("max_position_pct must be between 0.001 and 1.0".to_string());
                    }
                } else {
                    return Err("max_position_pct must be a number between 0 and 1".to_string());
                }
            }
            _ => {} // Unknown parameters are allowed
        }

        Ok(())
    }
}
