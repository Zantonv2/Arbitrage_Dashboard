use crate::{
    types::{ExecutionInstruction, Order, OrderType, Signal, Side, TimeInForce},
    confidence_scorer::FeeSchedule,
    Result,
};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Configuration for execution preparation
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub slippage_buffer_percent: Decimal, // e.g., 0.05 = 0.05% slippage buffer
    pub default_time_in_force: TimeInForce,
    pub order_type: OrderType,
    pub enable_force_execute: bool, // Allow negative worst-case profit if strategy requires
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            slippage_buffer_percent: Decimal::new(5, 4), // 0.0005 = 0.05% slippage buffer
            default_time_in_force: TimeInForce::IOC,
            order_type: OrderType::Limit,
            enable_force_execute: false, // Conservative default
        }
    }
}

/// Prepares execution instructions from signals
pub struct ExecutionPreparer {
    config: ExecutionConfig,
    fee_schedules: HashMap<crate::types::ExchangeId, FeeSchedule>, // Real fee data
}

impl ExecutionPreparer {
    pub fn new(config: ExecutionConfig) -> Self {
        Self { 
            config,
            fee_schedules: HashMap::new(),
        }
    }
    
    /// Update fee schedule for an exchange
    pub fn update_fee_schedule(&mut self, exchange: crate::types::ExchangeId, schedule: FeeSchedule) {
        self.fee_schedules.insert(exchange, schedule);
    }
    
    /// Get fee rate for a specific exchange
    fn get_fee_rate(&self, exchange: crate::types::ExchangeId, is_maker: bool) -> Decimal {
        self.fee_schedules.get(&exchange)
            .map(|f| f.get_fee_rate(is_maker))
            .unwrap_or_else(|| Decimal::new(1, 3)) // Default 0.1% if no fee schedule
    }

    /// Prepare execution instruction from signal
    pub fn prepare_execution(&self, signal: &Signal, quantity: Decimal) -> Result<ExecutionInstruction> {
        // Apply slippage buffer to prices
        let slippage_multiplier = self.config.slippage_buffer_percent / Decimal::from(100);
        
        let buy_price_with_buffer = signal.buy_price * (Decimal::ONE + slippage_multiplier);
        let sell_price_with_buffer = signal.sell_price * (Decimal::ONE - slippage_multiplier);

        // Create buy order
        let buy_order = Order::new(
            signal.buy_exchange,
            signal.symbol.clone(),
            Side::Buy,
            self.config.order_type,
            quantity,
            Some(buy_price_with_buffer),
        );

        // Create sell order
        let sell_order = Order::new(
            signal.sell_exchange,
            signal.symbol.clone(),
            Side::Sell,
            self.config.order_type,
            quantity,
            Some(sell_price_with_buffer),
        );

        // Create execution instruction
        let mut instruction = ExecutionInstruction::new(signal.id, buy_order, sell_order);
        
        // Set time in force
        instruction.buy_order.time_in_force = self.config.default_time_in_force;
        instruction.sell_order.time_in_force = self.config.default_time_in_force;

        // Calculate expected outcomes
        self.calculate_expected_outcomes(&mut instruction, signal)?;

        // Validate the instruction
        self.validate_instruction(&mut instruction)?;

        Ok(instruction)
    }

    /// Calculate expected and worst-case profit scenarios
    fn calculate_expected_outcomes(
        &self,
        instruction: &mut ExecutionInstruction,
        signal: &Signal,
    ) -> Result<()> {
        let quantity = instruction.buy_order.quantity;
        
        // Expected case: fill at signal prices
        let expected_buy_cost = signal.buy_price * quantity;
        let expected_sell_revenue = signal.sell_price * quantity;
        instruction.expected_profit = expected_sell_revenue - expected_buy_cost;

        // Worst case: fill at buffered prices
        let worst_buy_cost = instruction.buy_order.price.unwrap_or(signal.buy_price) * quantity;
        let worst_sell_revenue = instruction.sell_order.price.unwrap_or(signal.sell_price) * quantity;
        instruction.worst_case_profit = worst_sell_revenue - worst_buy_cost;

        // Calculate fees using real fee schedules (assume taker for conservative estimate)
        let buy_fee_rate = self.get_fee_rate(signal.buy_exchange, false);
        let sell_fee_rate = self.get_fee_rate(signal.sell_exchange, false);
        
        instruction.buy_order.expected_fee = expected_buy_cost * buy_fee_rate;
        instruction.sell_order.expected_fee = expected_sell_revenue * sell_fee_rate;
        instruction.total_fees = instruction.buy_order.expected_fee + instruction.sell_order.expected_fee;

        // Adjust profits for fees
        instruction.expected_profit -= instruction.total_fees;
        instruction.worst_case_profit -= instruction.total_fees;

        instruction.slippage_buffer = self.config.slippage_buffer_percent;

        Ok(())
    }

    /// Validate execution instruction
    fn validate_instruction(&self, instruction: &mut ExecutionInstruction) -> Result<()> {
        instruction.validation_errors.clear();

        // Check that we have prices for limit orders
        if instruction.buy_order.order_type == OrderType::Limit && instruction.buy_order.price.is_none() {
            instruction.validation_errors.push("Buy order missing price for limit order".to_string());
        }

        if instruction.sell_order.order_type == OrderType::Limit && instruction.sell_order.price.is_none() {
            instruction.validation_errors.push("Sell order missing price for limit order".to_string());
        }

        // Check quantities are positive
        if instruction.buy_order.quantity <= Decimal::ZERO {
            instruction.validation_errors.push("Buy order quantity must be positive".to_string());
        }

        if instruction.sell_order.quantity <= Decimal::ZERO {
            instruction.validation_errors.push("Sell order quantity must be positive".to_string());
        }

        // Check quantities match
        if instruction.buy_order.quantity != instruction.sell_order.quantity {
            instruction.validation_errors.push("Buy and sell quantities must match".to_string());
        }

        // Check symbols match
        if instruction.buy_order.symbol != instruction.sell_order.symbol {
            instruction.validation_errors.push("Buy and sell symbols must match".to_string());
        }

        // Check that worst case is still profitable (unless force execute is enabled)
        if instruction.worst_case_profit <= Decimal::ZERO && !self.config.enable_force_execute {
            instruction.validation_errors.push(
                format!("Worst case profit {} is not positive (use force_execute to override)", instruction.worst_case_profit)
            );
        } else if instruction.worst_case_profit <= Decimal::ZERO && self.config.enable_force_execute {
            // Log warning but allow execution
            tracing::warn!(
                signal_id = %instruction.signal_id,
                worst_case_profit = %instruction.worst_case_profit,
                "Force executing instruction with negative worst-case profit"
            );
        }

        // Validate minimum notional values (placeholder)
        let min_notional = Decimal::from(10); // $10 minimum
        
        if let Some(buy_price) = instruction.buy_order.price {
            let buy_notional = buy_price * instruction.buy_order.quantity;
            if buy_notional < min_notional {
                instruction.validation_errors.push(
                    format!("Buy order notional {} below minimum {}", buy_notional, min_notional)
                );
            }
        }

        if let Some(sell_price) = instruction.sell_order.price {
            let sell_notional = sell_price * instruction.sell_order.quantity;
            if sell_notional < min_notional {
                instruction.validation_errors.push(
                    format!("Sell order notional {} below minimum {}", sell_notional, min_notional)
                );
            }
        }

        Ok(())
    }

    /// Format quantity to exchange precision
    pub fn format_quantity(&self, quantity: Decimal, precision: u32) -> Decimal {
        let scale = 10_u64.pow(precision);
        let scaled = quantity * Decimal::from(scale);
        let rounded = scaled.round();
        rounded / Decimal::from(scale)
    }

    /// Generate preview of execution
    pub fn generate_preview(&self, instruction: &ExecutionInstruction) -> ExecutionPreview {
        ExecutionPreview {
            buy_order_summary: OrderSummary {
                exchange: instruction.buy_order.exchange,
                symbol: instruction.buy_order.symbol.clone(),
                side: instruction.buy_order.side,
                quantity: instruction.buy_order.quantity,
                price: instruction.buy_order.price,
                estimated_cost: instruction.buy_order.price
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
                estimated_cost: instruction.sell_order.price
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

    /// Get current configuration
    pub fn get_config(&self) -> &ExecutionConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: ExecutionConfig) {
        self.config = config;
    }
}

/// Summary of an order for preview
#[derive(Debug, Clone)]
pub struct OrderSummary {
    pub exchange: crate::types::ExchangeId,
    pub symbol: crate::types::Symbol,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub estimated_cost: Decimal,
    pub estimated_fee: Decimal,
}

/// Preview of execution instruction
#[derive(Debug, Clone)]
pub struct ExecutionPreview {
    pub buy_order_summary: OrderSummary,
    pub sell_order_summary: OrderSummary,
    pub expected_profit: Decimal,
    pub worst_case_profit: Decimal,
    pub total_fees: Decimal,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
}

impl Default for ExecutionPreparer {
    fn default() -> Self {
        Self::new(ExecutionConfig::default())
    }
}