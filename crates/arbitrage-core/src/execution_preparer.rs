//! Execution instruction preparation and validation.
//!
//! This module handles the conversion of arbitrage signals into executable
//! order instructions, including slippage buffers, fee calculations, and
//! comprehensive validation.
//!
//! # Execution Preparation Flow
//!
//! 1. **Price Adjustment**: Apply slippage buffer to signal prices
//! 2. **Order Creation**: Generate buy and sell orders
//! 3. **Outcome Calculation**: Calculate expected and worst-case profits
//! 4. **Validation**: Verify all constraints are met
//! 5. **Preview**: Generate human-readable execution preview
//!
//! # Example
//!
//! ```rust
//! use arbitrage_core::execution_preparer::{ExecutionPreparer, ExecutionConfig};
//! use arbitrage_core::types::{ExchangeId, OrderType, Signal, Symbol, TimeInForce};
//! use rust_decimal::Decimal;
//!
//! let config = ExecutionConfig::default();
//! let preparer = ExecutionPreparer::new(config);
//!
//! // Prepare execution from a signal
//! ```

use crate::{
    confidence_scorer::FeeSchedule,
    types::{ExecutionInstruction, Order, OrderType, Side, Signal, TimeInForce},
    Result,
};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Configuration for execution preparation.
///
/// Controls how orders are generated and validated.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Slippage buffer percentage (e.g., 0.0005 = 0.05%)
    pub slippage_buffer_percent: Decimal,
    /// Default time in force for orders
    pub default_time_in_force: TimeInForce,
    /// Order type to use
    pub order_type: OrderType,
    /// Allow execution even with negative worst-case profit
    pub enable_force_execute: bool,
}

impl ExecutionConfig {
    /// Creates a new ExecutionConfig.
    ///
    /// # Arguments
    ///
    /// * `slippage_buffer_percent` - Buffer for adverse slippage
    /// * `default_time_in_force` - Default TIF for orders
    /// * `order_type` - Order type to use
    /// * `enable_force_execute` - Allow negative worst-case execution
    pub fn new(
        slippage_buffer_percent: Decimal,
        default_time_in_force: TimeInForce,
        order_type: OrderType,
        enable_force_execute: bool,
    ) -> Self {
        if enable_force_execute && Self::is_production() {
            tracing::error!("force_execute cannot be enabled in production mode - ignoring force_execute setting");
        }
        Self {
            slippage_buffer_percent,
            default_time_in_force,
            order_type,
            enable_force_execute: enable_force_execute && !Self::is_production(),
        }
    }

    /// Checks if running in production mode.
    fn is_production() -> bool {
        std::env::var("ARBITRAGE_ENV")
            .map(|v| v.eq_ignore_ascii_case("production"))
            .unwrap_or(false)
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self::new(
            Decimal::new(5, 4),
            TimeInForce::IOC,
            OrderType::Limit,
            false,
        )
    }
}

/// Prepares execution instructions from signals.
///
/// Takes a validated signal and quantity, then generates a complete
/// execution plan with buy and sell orders, fee estimates, and
/// profit projections.
///
/// # Process
///
/// 1. Applies slippage buffer to signal prices
/// 2. Creates buy and sell orders
/// 3. Calculates expected and worst-case outcomes
/// 4. Validates against constraints
/// 5. Returns instruction ready for execution
///
/// # Example
///
/// ```rust
/// use arbitrage_core::execution_preparer::{ExecutionPreparer, ExecutionConfig};
/// use arbitrage_core::types::{ExchangeId, Signal, Symbol};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let preparer = ExecutionPreparer::default();
///
/// let signal = Signal::new(
///     Symbol::new("BTC", "USDT"),
///     ExchangeId::Binance,
///     ExchangeId::ByBit,
///     Decimal::from(50000),
///     Decimal::from(50100),
///     Utc::now(),
/// );
///
/// let instruction = preparer.prepare_execution(&signal, Decimal::from(1));
/// ```
#[derive(Debug, Clone)]
pub struct ExecutionPreparer {
    /// Configuration
    config: ExecutionConfig,
    /// Fee schedules by exchange
    fee_schedules: HashMap<crate::types::ExchangeId, FeeSchedule>,
}

impl ExecutionPreparer {
    /// Creates a new ExecutionPreparer with default configuration.
    pub fn new(config: ExecutionConfig) -> Self {
        Self {
            config,
            fee_schedules: HashMap::new(),
        }
    }

    /// Updates fee schedule for an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `schedule` - The fee schedule
    pub fn update_fee_schedule(
        &mut self,
        exchange: crate::types::ExchangeId,
        schedule: FeeSchedule,
    ) {
        self.fee_schedules.insert(exchange, schedule);
    }

    /// Gets fee rate for an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `is_maker` - True for maker fee, false for taker
    ///
    /// # Returns
    ///
    /// The fee rate, or 0.1% default if not configured.
    fn get_fee_rate(&self, exchange: crate::types::ExchangeId, is_maker: bool) -> Decimal {
        self.fee_schedules
            .get(&exchange)
            .map(|f| f.get_fee_rate(is_maker))
            .unwrap_or_else(|| Decimal::new(1, 3))
    }

    /// Prepares execution instruction from a signal.
    ///
    /// Generates a complete execution plan with:
    /// - Slippage-buffered orders
    /// - Fee estimates
    /// - Expected and worst-case profit calculations
    /// - Comprehensive validation
    ///
    /// # Arguments
    ///
    /// * `signal` - The arbitrage signal
    /// * `quantity` - The quantity to trade
    ///
    /// # Returns
    ///
    /// `Result<ExecutionInstruction>` ready for execution
    ///
    /// # Preconditions
    ///
    /// - Signal must have valid buy/sell prices
    /// - Quantity must be positive
    ///
    /// # Postconditions
    ///
    /// - Returns instruction with orders, fees, and profit projections
    /// - Instruction validation_errors will be empty if valid
    pub fn prepare_execution(
        &self,
        signal: &Signal,
        quantity: Decimal,
    ) -> Result<ExecutionInstruction> {
        let slippage_multiplier = self.config.slippage_buffer_percent / Decimal::from(100);

        let buy_price_with_buffer = signal.buy_price * (Decimal::ONE + slippage_multiplier);
        let sell_price_with_buffer = signal.sell_price * (Decimal::ONE - slippage_multiplier);

        let buy_order = Order::new(
            signal.buy_exchange,
            signal.symbol.clone(),
            Side::Buy,
            self.config.order_type,
            quantity,
            Some(buy_price_with_buffer),
        );

        let sell_order = Order::new(
            signal.sell_exchange,
            signal.symbol.clone(),
            Side::Sell,
            self.config.order_type,
            quantity,
            Some(sell_price_with_buffer),
        );

        let mut instruction = ExecutionInstruction::new(signal.id, buy_order, sell_order);

        instruction.buy_order.time_in_force = self.config.default_time_in_force;
        instruction.sell_order.time_in_force = self.config.default_time_in_force;

        self.calculate_expected_outcomes(&mut instruction, signal)?;

        self.validate_instruction(&mut instruction)?;

        Ok(instruction)
    }

    /// Calculates expected and worst-case profit scenarios.
    ///
    /// # Arguments
    ///
    /// * `instruction` - The instruction to update
    /// * `signal` - The source signal
    fn calculate_expected_outcomes(
        &self,
        instruction: &mut ExecutionInstruction,
        signal: &Signal,
    ) -> Result<()> {
        let quantity = instruction.buy_order.quantity;

        let expected_buy_cost = signal.buy_price * quantity;
        let expected_sell_revenue = signal.sell_price * quantity;
        instruction.expected_profit = expected_sell_revenue - expected_buy_cost;

        let worst_buy_cost = instruction.buy_order.price.unwrap_or(signal.buy_price) * quantity;
        let worst_sell_revenue =
            instruction.sell_order.price.unwrap_or(signal.sell_price) * quantity;
        instruction.worst_case_profit = worst_sell_revenue - worst_buy_cost;

        let buy_fee_rate = self.get_fee_rate(signal.buy_exchange, false);
        let sell_fee_rate = self.get_fee_rate(signal.sell_exchange, false);

        instruction.buy_order.expected_fee = expected_buy_cost * buy_fee_rate;
        instruction.sell_order.expected_fee = expected_sell_revenue * sell_fee_rate;
        instruction.total_fees =
            instruction.buy_order.expected_fee + instruction.sell_order.expected_fee;

        instruction.expected_profit -= instruction.total_fees;
        instruction.worst_case_profit -= instruction.total_fees;

        instruction.slippage_buffer = self.config.slippage_buffer_percent;

        Ok(())
    }

    /// Validates execution instruction.
    ///
    /// Performs comprehensive validation including:
    /// - Order completeness (prices for limit orders)
    /// - Quantity validity (positive, matching)
    /// - Symbol matching
    /// - Profitability (unless force_execute enabled)
    /// - Minimum notional values
    ///
    /// # Arguments
    ///
    /// * `instruction` - The instruction to validate
    fn validate_instruction(&self, instruction: &mut ExecutionInstruction) -> Result<()> {
        instruction.validation_errors.clear();

        if instruction.buy_order.order_type == OrderType::Limit
            && instruction.buy_order.price.is_none()
        {
            instruction
                .validation_errors
                .push("Buy order missing price for limit order".to_string());
        }

        if instruction.sell_order.order_type == OrderType::Limit
            && instruction.sell_order.price.is_none()
        {
            instruction
                .validation_errors
                .push("Sell order missing price for limit order".to_string());
        }

        if instruction.buy_order.quantity <= Decimal::ZERO {
            instruction
                .validation_errors
                .push("Buy order quantity must be positive".to_string());
        }

        if instruction.sell_order.quantity <= Decimal::ZERO {
            instruction
                .validation_errors
                .push("Sell order quantity must be positive".to_string());
        }

        if instruction.buy_order.quantity != instruction.sell_order.quantity {
            instruction
                .validation_errors
                .push("Buy and sell quantities must match".to_string());
        }

        if instruction.buy_order.symbol != instruction.sell_order.symbol {
            instruction
                .validation_errors
                .push("Buy and sell symbols must match".to_string());
        }

        if instruction.expected_profit <= Decimal::ZERO && !self.config.enable_force_execute {
            instruction.validation_errors.push(format!(
                "Expected profit {} is not positive",
                instruction.expected_profit
            ));
        } else if instruction.expected_profit <= Decimal::ZERO && self.config.enable_force_execute {
            tracing::warn!(
                signal_id = %instruction.signal_id,
                expected_profit = %instruction.expected_profit,
                "Force executing instruction with negative expected profit"
            );
        }

        if instruction.worst_case_profit <= Decimal::ZERO && !self.config.enable_force_execute {
            instruction.validation_errors.push(format!(
                "Worst case profit {} is not positive (use force_execute to override)",
                instruction.worst_case_profit
            ));
        } else if instruction.worst_case_profit <= Decimal::ZERO && self.config.enable_force_execute
        {
            tracing::warn!(
                signal_id = %instruction.signal_id,
                worst_case_profit = %instruction.worst_case_profit,
                "Force executing instruction with negative worst-case profit"
            );
        }

        let min_notional = Decimal::from(10);

        if let Some(buy_price) = instruction.buy_order.price {
            let buy_notional = buy_price * instruction.buy_order.quantity;
            if buy_notional < min_notional {
                instruction.validation_errors.push(format!(
                    "Buy order notional {} below minimum {}",
                    buy_notional, min_notional
                ));
            }
        }

        if let Some(sell_price) = instruction.sell_order.price {
            let sell_notional = sell_price * instruction.sell_order.quantity;
            if sell_notional < min_notional {
                instruction.validation_errors.push(format!(
                    "Sell order notional {} below minimum {}",
                    sell_notional, min_notional
                ));
            }
        }

        Ok(())
    }

    /// Formats quantity to exchange precision.
    ///
    /// # Arguments
    ///
    /// * `quantity` - The quantity to format
    /// * `precision` - Number of decimal places
    ///
    /// # Returns
    ///
    /// Quantity rounded to the specified precision.
    pub fn format_quantity(&self, quantity: Decimal, precision: u32) -> Decimal {
        let scale = 10_u64.pow(precision);
        let scaled = quantity * Decimal::from(scale);
        let rounded = scaled.round();
        rounded / Decimal::from(scale)
    }

    /// Generates a preview of the execution.
    ///
    /// Creates a human-readable summary of the execution instruction
    /// including order details and profit projections.
    ///
    /// # Arguments
    ///
    /// * `instruction` - The instruction to preview
    ///
    /// # Returns
    ///
    /// An `ExecutionPreview` with formatted summary.
    pub fn generate_preview(&self, instruction: &ExecutionInstruction) -> ExecutionPreview {
        ExecutionPreview {
            buy_order_summary: OrderSummary {
                exchange: instruction.buy_order.exchange,
                symbol: instruction.buy_order.symbol.clone(),
                side: instruction.buy_order.side,
                quantity: instruction.buy_order.quantity,
                price: instruction.buy_order.price,
                estimated_cost: instruction
                    .buy_order
                    .price
                    .map(|p| p * instruction.buy_order.quantity)
                    .unwrap_or(Decimal::ZERO),
                estimated_fee: instruction.buy_order.expected_fee,
            },
            sell_order_summary: OrderSummary {
                exchange: instruction.sell_order.exchange,
                symbol: instruction.sell_order.symbol.clone(),
                side: instruction.sell_order.side,
                quantity: instruction.sell_order.quantity,
                price: instruction.sell_order.price,
                estimated_cost: instruction
                    .sell_order
                    .price
                    .map(|p| p * instruction.sell_order.quantity)
                    .unwrap_or(Decimal::ZERO),
                estimated_fee: instruction.sell_order.expected_fee,
            },
            expected_profit: instruction.expected_profit,
            worst_case_profit: instruction.worst_case_profit,
            total_fees: instruction.total_fees,
            is_valid: instruction.is_valid(),
            validation_errors: instruction.validation_errors.clone(),
        }
    }

    /// Gets the current configuration.
    pub fn get_config(&self) -> &ExecutionConfig {
        &self.config
    }

    /// Updates the configuration.
    pub fn update_config(&mut self, config: ExecutionConfig) {
        self.config = config;
    }
}

/// Summary of an order for preview purposes.
#[derive(Debug, Clone)]
pub struct OrderSummary {
    /// Exchange for the order
    pub exchange: crate::types::ExchangeId,
    /// Trading symbol
    pub symbol: crate::types::Symbol,
    /// Buy or Sell
    pub side: Side,
    /// Order quantity
    pub quantity: Decimal,
    /// Order price (None for market orders)
    pub price: Option<Decimal>,
    /// Estimated cost (price * quantity)
    pub estimated_cost: Decimal,
    /// Estimated fee
    pub estimated_fee: Decimal,
}

/// Preview of an execution instruction.
///
/// Provides a human-readable summary of what an execution will do,
/// including profit projections and validation status.
#[derive(Debug, Clone)]
pub struct ExecutionPreview {
    /// Buy order summary
    pub buy_order_summary: OrderSummary,
    /// Sell order summary
    pub sell_order_summary: OrderSummary,
    /// Expected profit after fees
    pub expected_profit: Decimal,
    /// Worst-case profit with slippage
    pub worst_case_profit: Decimal,
    /// Total fees for both orders
    pub total_fees: Decimal,
    /// Whether the instruction is valid
    pub is_valid: bool,
    /// Validation errors, if any
    pub validation_errors: Vec<String>,
}

impl Default for ExecutionPreparer {
    fn default() -> Self {
        Self::new(ExecutionConfig::default())
    }
}
