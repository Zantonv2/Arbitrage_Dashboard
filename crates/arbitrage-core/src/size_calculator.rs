//! Trade size calculation and recommendation engine.
//!
//! This module provides utilities for calculating optimal trade sizes based on
//! order book depth, slippage tolerance, and risk constraints.
//!
//! # Size Calculation Process
//!
//! 1. Analyzes order book depth on both sides of the trade
//! 2. Calculates maximum fillable quantity at each slippage tier
//! 3. Applies configurable limits (min/max order size, position limits)
//! 4. Returns recommended size with limiting factor analysis
//!
//! # Example
//!
//! ```rust
//! use arbitrage_core::size_calculator::{SizeCalculator, SizeConfig};
//! use arbitrage_core::types::{ExchangeId, OrderBook, OrderBookLevel, Signal, Symbol};
//! use rust_decimal::Decimal;
//!
//! let config = SizeConfig::default();
//! let calculator = SizeCalculator::new(config);
//!
//! // Size recommendation would be calculated based on signal and order books
//! ```

use crate::{
    types::{ExchangeId, OrderBook, OrderBookLevel, Signal, Symbol},
    ArbitrageError, Result,
};
use rust_decimal::Decimal;

/// Configuration for size calculation parameters.
///
/// Controls how trade sizes are calculated based on slippage tolerance,
/// depth requirements, and risk limits.
#[derive(Debug, Clone)]
pub struct SizeConfig {
    /// Maximum acceptable slippage percentage (e.g., 0.001 = 0.1%)
    pub max_slippage_percent: Decimal,
    /// Conservative multiplier for recommended size (e.g., 0.8 = 80% of max)
    pub conservative_multiplier: Decimal,
    /// Minimum order size in USD
    pub min_order_size_usd: Decimal,
    /// Maximum order size in USD
    pub max_order_size_usd: Decimal,
    /// Maximum position size in USD
    pub max_position_size_usd: Decimal,
    /// Configurable slippage tiers for tiered analysis
    pub slippage_tiers: Vec<Decimal>,
    /// Whether to account for fees in size calculation
    pub fee_aware_sizing: bool,
}

impl Default for SizeConfig {
    fn default() -> Self {
        Self {
            max_slippage_percent: Decimal::new(1, 3),
            conservative_multiplier: Decimal::new(8, 1),
            min_order_size_usd: Decimal::from(10),
            max_order_size_usd: Decimal::from(50000),
            max_position_size_usd: Decimal::from(10000),
            slippage_tiers: vec![
                Decimal::new(5, 4),
                Decimal::new(1, 3),
                Decimal::new(2, 3),
                Decimal::new(5, 3),
            ],
            fee_aware_sizing: true,
        }
    }
}

/// Size calculation result with multiple slippage tiers.
///
/// Contains the recommended size along with detailed analysis at different
/// slippage tolerance levels.
#[derive(Debug, Clone)]
pub struct SizeRecommendation {
    /// Conservative recommended size (uses conservative_multiplier)
    pub recommended_size: Decimal,
    /// Maximum possible size at tightest slippage tier
    pub max_size: Decimal,
    /// Expected slippage at recommended size
    pub expected_slippage: Decimal,
    /// Size tiers at different slippage levels
    pub size_tiers: Vec<SizeTier>,
    /// What factor limited the position size
    pub limiting_factor: LimitingFactor,
}

/// Size tier for different slippage tolerances.
///
/// Represents the maximum fillable size at a specific slippage tolerance
/// with expected fill prices.
#[derive(Debug, Clone)]
pub struct SizeTier {
    /// Slippage percentage for this tier
    pub slippage_percent: Decimal,
    /// Maximum fillable quantity at this slippage
    pub max_size: Decimal,
    /// Expected fill price for buys
    pub expected_fill_price_buy: Decimal,
    /// Expected fill price for sells
    pub expected_fill_price_sell: Decimal,
}

/// What factor limited the position size.
///
/// Indicates which constraint was the limiting factor in size calculation.
#[derive(Debug, Clone, PartialEq)]
pub enum LimitingFactor {
    /// Limited by order book depth
    OrderBookDepth,
    /// Below exchange minimum order size
    ExchangeMinimum,
    /// Above exchange maximum order size
    ExchangeMaximum,
    /// Hit user-defined position limit
    UserPositionLimit,
    /// Hit slippage tolerance
    SlippageTolerance,
    /// Insufficient depth in order book
    InsufficientDepth,
    /// Hit inventory limit
    InventoryLimit,
    /// Fee impact reduced profitability
    FeeImpact,
    /// Hit exchange rate limit
    RateLimit,
}

impl std::fmt::Display for LimitingFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LimitingFactor::OrderBookDepth => write!(f, "OrderBookDepth"),
            LimitingFactor::ExchangeMinimum => write!(f, "ExchangeMinimum"),
            LimitingFactor::ExchangeMaximum => write!(f, "ExchangeMaximum"),
            LimitingFactor::UserPositionLimit => write!(f, "UserPositionLimit"),
            LimitingFactor::SlippageTolerance => write!(f, "SlippageTolerance"),
            LimitingFactor::InsufficientDepth => write!(f, "InsufficientDepth"),
            LimitingFactor::InventoryLimit => write!(f, "InventoryLimit"),
            LimitingFactor::FeeImpact => write!(f, "FeeImpact"),
            LimitingFactor::RateLimit => write!(f, "RateLimit"),
        }
    }
}

/// Calculator for optimal trade sizes.
///
/// Analyzes order book depth and applies risk constraints to determine
/// the optimal position size for arbitrage opportunities.
///
/// # Example
///
/// ```rust
/// use arbitrage_core::size_calculator::{SizeCalculator, SizeConfig};
/// use arbitrage_core::types::{ExchangeId, OrderBook, OrderBookLevel, Signal, Symbol};
/// use rust_decimal::Decimal;
///
/// let config = SizeConfig::default();
/// let calculator = SizeCalculator::new(config);
///
/// // Would use with signal and order books
/// ```
#[derive(Debug, Clone)]
pub struct SizeCalculator {
    /// Calculation configuration
    config: SizeConfig,
}

impl SizeCalculator {
    /// Creates a new SizeCalculator with the given configuration.
    pub fn new(config: SizeConfig) -> Self {
        Self { config }
    }

    /// Calculates recommended size for a signal.
    ///
    /// Analyzes order books on both sides and calculates the optimal
    /// position size based on configurable slippage tiers and limits.
    ///
    /// # Arguments
    ///
    /// * `signal` - The arbitrage signal with pricing
    /// * `buy_order_book` - Order book from the buy exchange
    /// * `sell_order_book` - Order book from the sell exchange
    ///
    /// # Returns
    ///
    /// `Result<SizeRecommendation>` with size tiers and limiting factor
    ///
    /// # Process
    ///
    /// 1. Calculate max size at each slippage tier
    /// 2. Determine limiting factor based on constraints
    /// 3. Apply conservative multiplier for recommended size
    pub fn calculate_size(
        &self,
        signal: &Signal,
        buy_order_book: &OrderBook,
        sell_order_book: &OrderBook,
    ) -> Result<SizeRecommendation> {
        // Validate signal prices - reject negative prices
        if signal.buy_price < Decimal::ZERO {
            return Err(ArbitrageError::Validation(format!(
                "Buy price must be non-negative, got {}",
                signal.buy_price
            )));
        }

        if signal.sell_price < Decimal::ZERO {
            return Err(ArbitrageError::Validation(format!(
                "Sell price must be non-negative, got {}",
                signal.sell_price
            )));
        }

        let mut size_tiers = Vec::new();
        let mut max_size = Decimal::ZERO;

        for slippage in &self.config.slippage_tiers {
            match self.calculate_size_for_slippage(
                signal,
                buy_order_book,
                sell_order_book,
                *slippage,
            ) {
                Ok(tier) => {
                    if tier.max_size > max_size {
                        max_size = tier.max_size;
                    }
                    size_tiers.push(tier);
                }
                Err(_) => {
                    size_tiers.push(SizeTier {
                        slippage_percent: *slippage,
                        max_size: Decimal::ZERO,
                        expected_fill_price_buy: signal.buy_price,
                        expected_fill_price_sell: signal.sell_price,
                    });
                }
            }
        }

        let recommended_size = if let Some(first_tier) = size_tiers.first() {
            if first_tier.max_size > Decimal::ZERO {
                first_tier.max_size * self.config.conservative_multiplier
            } else {
                Decimal::ZERO
            }
        } else {
            Decimal::ZERO
        };

        let limiting_factor = self
            .determine_limiting_factor(recommended_size, signal.buy_price.max(signal.sell_price));

        Ok(SizeRecommendation {
            recommended_size,
            max_size,
            expected_slippage: self
                .config
                .slippage_tiers
                .first()
                .copied()
                .unwrap_or(Decimal::ZERO),
            size_tiers,
            limiting_factor,
        })
    }

    /// Calculates size for a specific slippage tolerance.
    ///
    /// Internal method that analyzes both order books to find the maximum
    /// fillable quantity within the slippage constraint.
    ///
    /// # Arguments
    ///
    /// * `signal` - The arbitrage signal
    /// * `buy_order_book` - Order book from buy exchange
    /// * `sell_order_book` - Order book from sell exchange
    /// * `max_slippage` - Maximum acceptable slippage percentage
    ///
    /// # Returns
    ///
    /// `Result<SizeTier>` with max size and fill prices
    fn calculate_size_for_slippage(
        &self,
        signal: &Signal,
        buy_order_book: &OrderBook,
        sell_order_book: &OrderBook,
        max_slippage: Decimal,
    ) -> Result<SizeTier> {
        let (buy_size, buy_avg_price) = self.calculate_max_size_for_slippage(
            &buy_order_book.asks,
            signal.buy_price,
            max_slippage,
            true,
        )?;

        let (sell_size, sell_avg_price) = self.calculate_max_size_for_slippage(
            &sell_order_book.bids,
            signal.sell_price,
            max_slippage,
            false,
        )?;

        let max_size = buy_size.min(sell_size);

        if max_size <= Decimal::ZERO {
            return Err(ArbitrageError::Calculation(
                "Insufficient depth for any size".to_string(),
            ));
        }

        Ok(SizeTier {
            slippage_percent: max_slippage,
            max_size,
            expected_fill_price_buy: buy_avg_price,
            expected_fill_price_sell: sell_avg_price,
        })
    }

    /// Calculates maximum size for given slippage on one side.
    ///
    /// Traverses order book levels until the price moves beyond the
    /// slippage tolerance.
    ///
    /// # Arguments
    ///
    /// * `levels` - Order book levels (asks for buy, bids for sell)
    /// * `start_price` - Starting price (signal price)
    /// * `max_slippage` - Maximum acceptable slippage percentage
    /// * `is_buying` - True if calculating for buy side
    ///
    /// # Returns
    ///
    /// Tuple of (max quantity, average fill price)
    ///
    /// # Errors
    ///
    /// Returns `ArbitrageError::Validation` if start_price is zero or negative,
    /// or if max_slippage is negative.
    fn calculate_max_size_for_slippage(
        &self,
        levels: &[OrderBookLevel],
        start_price: Decimal,
        max_slippage: Decimal,
        is_buying: bool,
    ) -> Result<(Decimal, Decimal)> {
        if start_price <= Decimal::ZERO {
            return Err(ArbitrageError::Validation(format!(
                "Start price must be positive, got {}",
                start_price
            )));
        }

        if max_slippage < Decimal::ZERO {
            return Err(ArbitrageError::Validation(format!(
                "Max slippage must be non-negative, got {}",
                max_slippage
            )));
        }

        if levels.is_empty() {
            return Ok((Decimal::ZERO, start_price));
        }

        let max_price_change = start_price * max_slippage / Decimal::from(100);
        let price_limit = if is_buying {
            start_price + max_price_change
        } else {
            start_price - max_price_change
        };

        let mut total_quantity = Decimal::ZERO;
        let mut total_cost = Decimal::ZERO;

        for level in levels {
            let can_use_level = if is_buying {
                level.price <= price_limit
            } else {
                level.price >= price_limit
            };

            if !can_use_level {
                break;
            }

            if level.quantity <= Decimal::ZERO {
                continue;
            }

            if level.price <= Decimal::ZERO {
                continue;
            }

            total_quantity += level.quantity;
            total_cost += level.price * level.quantity;
        }

        let avg_price = if total_quantity > Decimal::ZERO {
            total_cost / total_quantity
        } else {
            start_price
        };

        Ok((total_quantity, avg_price))
    }

    /// Determines what factor is limiting the position size.
    ///
    /// Analyzes the calculated size against configured limits to identify
    /// the primary constraint.
    ///
    /// # Arguments
    ///
    /// * `size` - The calculated position size
    /// * `price` - The reference price for USD calculation
    ///
    /// # Returns
    ///
    /// The `LimitingFactor` enum indicating the primary constraint
    fn determine_limiting_factor(&self, size: Decimal, price: Decimal) -> LimitingFactor {
        let size_usd = size * price;

        if size_usd < self.config.min_order_size_usd {
            LimitingFactor::ExchangeMinimum
        } else if size_usd > self.config.max_order_size_usd {
            LimitingFactor::ExchangeMaximum
        } else if size_usd > self.config.max_position_size_usd {
            LimitingFactor::UserPositionLimit
        } else if size <= Decimal::ZERO {
            LimitingFactor::InsufficientDepth
        } else {
            LimitingFactor::OrderBookDepth
        }
    }

    /// Validates that size meets exchange requirements.
    ///
    /// Checks the calculated size against minimum and maximum order size
    /// constraints.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `exchange` - The exchange
    /// * `size` - The quantity to validate
    /// * `price` - The price for notional calculation
    ///
    /// # Returns
    ///
    /// `Ok(())` if valid, error with explanation if not
    pub fn validate_size(
        &self,
        _symbol: &Symbol,
        _exchange: ExchangeId,
        size: Decimal,
        price: Decimal,
    ) -> Result<()> {
        let notional = size * price;

        if notional < self.config.min_order_size_usd {
            return Err(ArbitrageError::Validation(format!(
                "Order size {} USD below minimum {} USD",
                notional, self.config.min_order_size_usd
            )));
        }

        if notional > self.config.max_order_size_usd {
            return Err(ArbitrageError::Validation(format!(
                "Order size {} USD above maximum {} USD",
                notional, self.config.max_order_size_usd
            )));
        }

        Ok(())
    }

    /// Calculates target quantity for VWAP analysis.
    ///
    /// Converts a target USD amount to a quantity based on the mid price.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `mid_price` - Current mid price
    /// * `target_usd` - Target USD amount
    ///
    /// # Returns
    ///
    /// The calculated quantity, at minimum the exchange minimum
    ///
    /// # Errors
    ///
    /// Returns error if mid_price is zero.
    pub fn calculate_vwap_quantity(
        &self,
        symbol: &Symbol,
        mid_price: Decimal,
        target_usd: Decimal,
    ) -> Result<Decimal> {
        if mid_price.is_zero() {
            return Err(ArbitrageError::Calculation(
                "Zero mid price for VWAP quantity calculation".to_string(),
            ));
        }

        let base_quantity = target_usd.checked_div(mid_price).ok_or_else(|| {
            ArbitrageError::Calculation("Division by zero in VWAP quantity calculation".to_string())
        })?;

        let min_quantity = self.get_min_order_quantity(symbol);
        Ok(base_quantity.max(min_quantity))
    }

    /// Gets minimum order quantity for a symbol.
    ///
    /// In a production system, this would query exchange specifications.
    /// Currently returns a conservative default.
    fn get_min_order_quantity(&self, _symbol: &Symbol) -> Decimal {
        Decimal::new(1, 4)
    }

    /// Gets the current configuration.
    pub fn get_config(&self) -> &SizeConfig {
        &self.config
    }

    /// Updates the configuration.
    pub fn update_config(&mut self, config: SizeConfig) {
        self.config = config;
    }
}

impl Default for SizeCalculator {
    fn default() -> Self {
        Self::new(SizeConfig::default())
    }
}
