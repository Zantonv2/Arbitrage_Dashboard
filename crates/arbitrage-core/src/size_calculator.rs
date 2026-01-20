use crate::{
    types::{ExchangeId, OrderBook, OrderBookLevel, Signal, Symbol},
    ArbitrageError, Result,
};
use rust_decimal::Decimal;

/// Size calculation configuration
#[derive(Debug, Clone)]
pub struct SizeConfig {
    pub max_slippage_percent: Decimal,
    pub conservative_multiplier: Decimal,
    pub min_order_size_usd: Decimal,
    pub max_order_size_usd: Decimal,
    pub max_position_size_usd: Decimal,
    pub slippage_tiers: Vec<Decimal>, // Configurable slippage levels
    pub fee_aware_sizing: bool,       // Account for fees in size calculation
}

impl Default for SizeConfig {
    fn default() -> Self {
        Self {
            max_slippage_percent: Decimal::new(1, 3),    // 0.001 = 0.1%
            conservative_multiplier: Decimal::new(8, 1), // 0.8 = 80%
            min_order_size_usd: Decimal::from(10),
            max_order_size_usd: Decimal::from(50000),
            max_position_size_usd: Decimal::from(10000),
            slippage_tiers: vec![
                Decimal::new(5, 4), // 0.0005 = 0.05%
                Decimal::new(1, 3), // 0.001 = 0.1%
                Decimal::new(2, 3), // 0.002 = 0.2%
                Decimal::new(5, 3), // 0.005 = 0.5%
            ],
            fee_aware_sizing: true,
        }
    }
}

/// Size calculation result with multiple slippage tiers
#[derive(Debug, Clone)]
pub struct SizeRecommendation {
    pub recommended_size: Decimal,
    pub max_size: Decimal,
    pub expected_slippage: Decimal,
    pub size_tiers: Vec<SizeTier>,
    pub limiting_factor: LimitingFactor,
}

/// Size tier for different slippage tolerances
#[derive(Debug, Clone)]
pub struct SizeTier {
    pub slippage_percent: Decimal,
    pub max_size: Decimal,
    pub expected_fill_price_buy: Decimal,
    pub expected_fill_price_sell: Decimal,
}

/// What factor limited the position size
#[derive(Debug, Clone, PartialEq)]
pub enum LimitingFactor {
    OrderBookDepth,
    ExchangeMinimum,
    ExchangeMaximum,
    UserPositionLimit,
    SlippageTolerance,
    InsufficientDepth,
    InventoryLimit,
    FeeImpact,
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

/// Calculator for optimal trade sizes
#[derive(Debug, Clone)]
pub struct SizeCalculator {
    config: SizeConfig,
}

impl SizeCalculator {
    pub fn new(config: SizeConfig) -> Self {
        Self { config }
    }

    /// Calculate recommended size for a signal with real signal prices
    pub fn calculate_size(
        &self,
        signal: &Signal,
        buy_order_book: &OrderBook,
        sell_order_book: &OrderBook,
    ) -> Result<SizeRecommendation> {
        // Use configurable slippage tiers instead of hardcoded values
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
                    // Return zero-size tier instead of breaking the flow
                    size_tiers.push(SizeTier {
                        slippage_percent: *slippage,
                        max_size: Decimal::ZERO,
                        expected_fill_price_buy: signal.buy_price,
                        expected_fill_price_sell: signal.sell_price,
                    });
                }
            }
        }

        // Use the most conservative tier (lowest slippage) as recommended
        let recommended_size = if let Some(first_tier) = size_tiers.first() {
            if first_tier.max_size > Decimal::ZERO {
                first_tier.max_size * self.config.conservative_multiplier
            } else {
                Decimal::ZERO
            }
        } else {
            Decimal::ZERO
        };

        // Determine limiting factor using real signal prices
        let limiting_factor = self.determine_limiting_factor(
            recommended_size,
            signal.buy_price.max(signal.sell_price), // Use higher price for USD calculation
        );

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

    /// Calculate size for a specific slippage tolerance
    fn calculate_size_for_slippage(
        &self,
        signal: &Signal,
        buy_order_book: &OrderBook,
        sell_order_book: &OrderBook,
        max_slippage: Decimal,
    ) -> Result<SizeTier> {
        // Calculate maximum size we can trade on buy side
        let (buy_size, buy_avg_price) = self.calculate_max_size_for_slippage(
            &buy_order_book.asks,
            signal.buy_price,
            max_slippage,
            true, // buying (price goes up)
        )?;

        // Calculate maximum size we can trade on sell side
        let (sell_size, sell_avg_price) = self.calculate_max_size_for_slippage(
            &sell_order_book.bids,
            signal.sell_price,
            max_slippage,
            false, // selling (price goes down)
        )?;

        // Use the smaller of the two sizes
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

    /// Calculate maximum size for given slippage on one side
    fn calculate_max_size_for_slippage(
        &self,
        levels: &[OrderBookLevel],
        start_price: Decimal,
        max_slippage: Decimal,
        is_buying: bool,
    ) -> Result<(Decimal, Decimal)> {
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

    /// Determine what factor is limiting the position size
    fn determine_limiting_factor(
        &self,
        size: Decimal,
        price: Decimal, // Use actual price instead of hardcoded value
    ) -> LimitingFactor {
        // Check against configured limits
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

    /// Validate that size meets exchange requirements
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

    /// Calculate target quantity for VWAP analysis based on USD amount
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

        // Apply minimum order size constraints
        let min_quantity = self.get_min_order_quantity(symbol);
        Ok(base_quantity.max(min_quantity))
    }

    /// Get minimum order quantity for a symbol (simplified)
    fn get_min_order_quantity(&self, _symbol: &Symbol) -> Decimal {
        // In real implementation, this would come from exchange specs
        Decimal::new(1, 4) // 0.0001 as default minimum
    }

    /// Get current configuration
    pub fn get_config(&self) -> &SizeConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: SizeConfig) {
        self.config = config;
    }
}

impl Default for SizeCalculator {
    fn default() -> Self {
        Self::new(SizeConfig::default())
    }
}
