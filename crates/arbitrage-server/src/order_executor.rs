//! # Order Executor Service
//!
//! Coordinates simultaneous order execution across multiple exchanges for arbitrage trades.
//! Handles order placement, tracking, rollback, and hedge logic for failed executions.

use arbitrage_core::{
    types::{ExecutionInstruction, ExchangeId},
    ArbitrageError, Result,
};
use exchange_connectors::{
    connector::{
        ExchangeConnector, OrderRequest, OrderResponse, OrderSide, OrderType, 
        TimeInForce
    },
    exchange_manager::ExchangeManager,
};
use rust_decimal::Decimal;
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for order execution
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum time to wait for order fills
    pub execution_timeout_ms: u64,
    /// Maximum slippage tolerance (as percentage)
    pub max_slippage_percent: Decimal,
    /// Whether to enable rollback on partial failures
    pub enable_rollback: bool,
    /// Minimum profit threshold to proceed with execution
    pub min_profit_threshold: Decimal,
    /// Maximum position size per trade
    pub max_position_size: Decimal,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            execution_timeout_ms: 5000,
            max_slippage_percent: Decimal::new(50, 4), // 0.5%
            enable_rollback: true,
            min_profit_threshold: Decimal::new(10, 4), // 0.1%
            max_position_size: Decimal::new(10000, 0), // $10,000
        }
    }
}

/// Result of an arbitrage execution attempt
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub signal_id: Uuid,
    pub success: bool,
    pub buy_order: Option<OrderResponse>,
    pub sell_order: Option<OrderResponse>,
    pub actual_profit: Option<Decimal>,
    pub execution_time_ms: u64,
    pub error_message: Option<String>,
    pub rollback_performed: bool,
}

/// Order execution service for coordinated arbitrage trades
pub struct OrderExecutor {
    config: ExecutorConfig,
    exchange_manager: Arc<ExchangeManager>,
}

impl OrderExecutor {
    /// Create new order executor
    pub fn new(config: ExecutorConfig, exchange_manager: Arc<ExchangeManager>) -> Self {
        Self {
            config,
            exchange_manager,
        }
    }

    /// Execute an arbitrage opportunity
    pub async fn execute_arbitrage(
        &self,
        instruction: &ExecutionInstruction,
    ) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();
        
        info!(
            "🎯 Executing arbitrage: {} {} → {} (${:.2})",
            instruction.buy_order.symbol,
            instruction.buy_order.exchange,
            instruction.sell_order.exchange,
            instruction.expected_profit
        );

        // Validate instruction
        self.validate_instruction(instruction).await?;

        // Execute orders simultaneously
        let execution_result = match self.execute_orders_simultaneously(instruction).await {
            Ok(result) => result,
            Err(e) => {
                error!("Execution failed: {}", e);
                ExecutionResult {
                    signal_id: instruction.signal_id,
                    success: false,
                    buy_order: None,
                    sell_order: None,
                    actual_profit: None,
                    execution_time_ms: start_time.elapsed().as_millis().max(1) as u64,
                    error_message: Some(e.to_string()),
                    rollback_performed: false,
                }
            }
        };

        // Log result
        if execution_result.success {
            info!(
                "✅ Arbitrage executed successfully: {:.4}% profit in {}ms",
                execution_result.actual_profit.unwrap_or_default() * Decimal::from(100),
                execution_result.execution_time_ms
            );
        } else {
            warn!(
                "❌ Arbitrage execution failed: {} ({}ms)",
                execution_result.error_message.as_deref().unwrap_or("Unknown error"),
                execution_result.execution_time_ms
            );
        }

        Ok(execution_result)
    }

    /// Validate execution instruction
    async fn validate_instruction(&self, instruction: &ExecutionInstruction) -> Result<()> {
        // Check minimum profit threshold
        if instruction.expected_profit < self.config.min_profit_threshold {
            return Err(ArbitrageError::Execution(format!(
                "Profit {:.4}% below threshold {:.4}%",
                instruction.expected_profit * Decimal::from(100),
                self.config.min_profit_threshold * Decimal::from(100)
            )));
        }

        // Check position size limits
        let position_value = instruction.buy_order.quantity * instruction.buy_order.price.unwrap_or_default();
        if position_value > self.config.max_position_size {
            return Err(ArbitrageError::Execution(format!(
                "Position size ${:.2} exceeds limit ${:.2}",
                position_value,
                self.config.max_position_size
            )));
        }

        // Validate exchanges are available
        if !self.exchange_manager.is_exchange_connected(&instruction.buy_order.exchange).await {
            return Err(ArbitrageError::Execution(format!(
                "Buy exchange {} not connected",
                instruction.buy_order.exchange
            )));
        }

        if !self.exchange_manager.is_exchange_connected(&instruction.sell_order.exchange).await {
            return Err(ArbitrageError::Execution(format!(
                "Sell exchange {} not connected",
                instruction.sell_order.exchange
            )));
        }

        Ok(())
    }

    /// Execute buy and sell orders simultaneously
    async fn execute_orders_simultaneously(
        &self,
        instruction: &ExecutionInstruction,
    ) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        // Prepare orders
        let buy_request = OrderRequest {
            symbol: instruction.buy_order.symbol.clone(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: instruction.buy_order.quantity,
            price: instruction.buy_order.price,
            time_in_force: TimeInForce::IOC,
            client_order_id: Some(format!("arb_buy_{}", instruction.signal_id)),
        };

        let sell_request = OrderRequest {
            symbol: instruction.sell_order.symbol.clone(),
            side: OrderSide::Sell,
            order_type: OrderType::Market,
            quantity: instruction.sell_order.quantity,
            price: instruction.sell_order.price,
            time_in_force: TimeInForce::IOC,
            client_order_id: Some(format!("arb_sell_{}", instruction.signal_id)),
        };

        // Execute orders with timeout
        let execution_timeout = Duration::from_millis(self.config.execution_timeout_ms);
        
        let (buy_result, sell_result) = timeout(
            execution_timeout,
            async {
                tokio::join!(
                    self.place_order(&instruction.buy_order.exchange, &buy_request),
                    self.place_order(&instruction.sell_order.exchange, &sell_request)
                )
            },
        ).await.map_err(|_| ArbitrageError::Execution("Execution timeout".to_string()))?;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        // Handle results
        match (buy_result, sell_result) {
            (Ok(buy_order), Ok(sell_order)) => {
                // Both orders successful
                let actual_profit = self.calculate_actual_profit(&buy_order, &sell_order);
                
                Ok(ExecutionResult {
                    signal_id: instruction.signal_id,
                    success: true,
                    buy_order: Some(buy_order),
                    sell_order: Some(sell_order),
                    actual_profit: Some(actual_profit),
                    execution_time_ms: execution_time_ms.max(1), // Ensure it's at least 1ms
                    error_message: None,
                    rollback_performed: false,
                })
            }
            (Ok(buy_order), Err(sell_error)) => {
                // Buy succeeded, sell failed - need rollback
                let rollback_performed = if self.config.enable_rollback {
                    self.rollback_buy_order(&instruction.buy_order.exchange, &buy_order).await
                } else {
                    false
                };

                Ok(ExecutionResult {
                    signal_id: instruction.signal_id,
                    success: false,
                    buy_order: Some(buy_order),
                    sell_order: None,
                    actual_profit: None,
                    execution_time_ms: execution_time_ms.max(1),
                    error_message: Some(format!("Sell order failed: {}", sell_error)),
                    rollback_performed,
                })
            }
            (Err(buy_error), Ok(sell_order)) => {
                // Sell succeeded, buy failed - need rollback
                let rollback_performed = if self.config.enable_rollback {
                    self.rollback_sell_order(&instruction.sell_order.exchange, &sell_order).await
                } else {
                    false
                };

                Ok(ExecutionResult {
                    signal_id: instruction.signal_id,
                    success: false,
                    buy_order: None,
                    sell_order: Some(sell_order),
                    actual_profit: None,
                    execution_time_ms: execution_time_ms.max(1),
                    error_message: Some(format!("Buy order failed: {}", buy_error)),
                    rollback_performed,
                })
            }
            (Err(buy_error), Err(sell_error)) => {
                // Both orders failed
                Ok(ExecutionResult {
                    signal_id: instruction.signal_id,
                    success: false,
                    buy_order: None,
                    sell_order: None,
                    actual_profit: None,
                    execution_time_ms: execution_time_ms.max(1),
                    error_message: Some(format!("Both orders failed - Buy: {}, Sell: {}", buy_error, sell_error)),
                    rollback_performed: false,
                })
            }
        }
    }

    /// Place order on specific exchange
    async fn place_order(
        &self,
        exchange: &ExchangeId,
        request: &OrderRequest,
    ) -> Result<OrderResponse> {
        debug!("Placing {} order on {}: {} {}", 
            match request.side { OrderSide::Buy => "BUY", OrderSide::Sell => "SELL" },
            exchange,
            request.quantity,
            request.symbol
        );

        self.exchange_manager.place_order(exchange, request).await
    }

    /// Calculate actual profit from executed orders
    fn calculate_actual_profit(&self, buy_order: &OrderResponse, sell_order: &OrderResponse) -> Decimal {
        // For now, use simple calculation - in production would need to account for fees, slippage, etc.
        let buy_price = buy_order.price.unwrap_or_default();
        let sell_price = sell_order.price.unwrap_or_default();
        let quantity = buy_order.quantity.min(sell_order.quantity);
        
        if buy_price > Decimal::ZERO && quantity > Decimal::ZERO {
            ((sell_price - buy_price) / buy_price) * quantity
        } else {
            Decimal::ZERO
        }
    }

    /// Rollback buy order by placing offsetting sell
    async fn rollback_buy_order(&self, exchange: &ExchangeId, buy_order: &OrderResponse) -> bool {
        warn!("🔄 Rolling back buy order: {}", buy_order.order_id);

        let rollback_request = OrderRequest {
            symbol: buy_order.symbol.clone(),
            side: OrderSide::Sell,
            order_type: OrderType::Market,
            quantity: buy_order.quantity,
            price: None, // Market order
            time_in_force: TimeInForce::IOC,
            client_order_id: Some(format!("rollback_{}", buy_order.order_id)),
        };

        match self.place_order(exchange, &rollback_request).await {
            Ok(_) => {
                info!("✅ Buy order rollback successful");
                true
            }
            Err(e) => {
                error!("❌ Buy order rollback failed: {}", e);
                false
            }
        }
    }

    /// Rollback sell order by placing offsetting buy
    async fn rollback_sell_order(&self, exchange: &ExchangeId, sell_order: &OrderResponse) -> bool {
        warn!("🔄 Rolling back sell order: {}", sell_order.order_id);

        let rollback_request = OrderRequest {
            symbol: sell_order.symbol.clone(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: sell_order.quantity,
            price: None, // Market order
            time_in_force: TimeInForce::IOC,
            client_order_id: Some(format!("rollback_{}", sell_order.order_id)),
        };

        match self.place_order(exchange, &rollback_request).await {
            Ok(_) => {
                info!("✅ Sell order rollback successful");
                true
            }
            Err(e) => {
                error!("❌ Sell order rollback failed: {}", e);
                false
            }
        }
    }

    /// Get current configuration
    pub fn get_config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: ExecutorConfig) {
        self.config = config;
    }
}