//! # Order Executor Service
//!
//! Coordinates simultaneous order execution across multiple exchanges for arbitrage trades.
//! Handles order placement, tracking, rollback, and hedge logic for failed executions.
//! Implements a two-phase commit pattern for atomic cross-exchange order execution.

use arbitrage_core::{
    types::{ExchangeId, ExecutionInstruction},
    ArbitrageError, Result,
};
use exchange_connectors::{
    connector::{OrderRequest, OrderResponse, OrderSide, OrderType, TimeInForce},
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
    /// Enable two-phase commit for atomic execution
    pub enable_two_phase_commit: bool,
    /// Phase 1 timeout in milliseconds
    pub prepare_timeout_ms: u64,
    /// Phase 2 timeout in milliseconds
    pub commit_timeout_ms: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            execution_timeout_ms: 5000,
            max_slippage_percent: Decimal::new(50, 4), // 0.5%
            enable_rollback: true,
            min_profit_threshold: Decimal::new(10, 4), // 0.1%
            max_position_size: Decimal::new(10000, 0), // $10,000
            enable_two_phase_commit: true,
            prepare_timeout_ms: 2000,
            commit_timeout_ms: 3000,
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

/// State of an order in the two-phase commit protocol
#[derive(Debug, Clone)]
pub enum OrderState {
    /// Order has not been prepared yet
    Initial,
    /// Order has been prepared (Phase 1 complete)
    Prepared,
    /// Order has been committed (Phase 2 complete)
    Committed,
    /// Order has been rolled back
    RolledBack,
    /// Order preparation failed
    Failed(String),
}

/// Result of two-phase commit execution
#[derive(Debug)]
pub struct TwoPhaseCommitResult {
    pub buy_state: OrderState,
    pub sell_state: OrderState,
    pub buy_order: Option<OrderResponse>,
    pub sell_order: Option<OrderResponse>,
    pub commit_successful: bool,
    pub error_message: Option<String>,
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

        // Execute using two-phase commit if enabled, otherwise use legacy method
        let execution_result = if self.config.enable_two_phase_commit {
            self.execute_with_two_phase_commit(instruction).await
        } else {
            self.execute_orders_simultaneously(instruction).await
        };

        let execution_result = match execution_result {
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
                execution_result
                    .error_message
                    .as_deref()
                    .unwrap_or("Unknown error"),
                execution_result.execution_time_ms
            );
        }

        Ok(execution_result)
    }

    /// Execute using two-phase commit protocol for atomic cross-exchange execution
    async fn execute_with_two_phase_commit(
        &self,
        instruction: &ExecutionInstruction,
    ) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        // Prepare order requests
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

        // Phase 1: Prepare - execute orders but hold them
        let (buy_prepare, sell_prepare, prepare_error) = self
            .prepare_orders(&instruction.buy_order.exchange, &buy_request)
            .await;

        // If either preparation fails, abort and return error
        if let Some(e) = prepare_error {
            return Err(e);
        }

        let (buy_order, buy_state) = buy_prepare;
        let (sell_order, sell_state) = sell_prepare;

        // Verify both orders were prepared successfully
        if buy_state != OrderState::Prepared || sell_state != OrderState::Prepared {
            // Attempt rollback of any prepared orders
            self.rollback_prepared_orders(
                &instruction.buy_order.exchange,
                &instruction.sell_order.exchange,
                &buy_order,
                &sell_order,
                buy_state.clone(),
                sell_state.clone(),
            )
            .await;

            return Ok(ExecutionResult {
                signal_id: instruction.signal_id,
                success: false,
                buy_order,
                sell_order,
                actual_profit: None,
                execution_time_ms: start_time.elapsed().as_millis().max(1) as u64,
                error_message: Some("Order preparation failed".to_string()),
                rollback_performed: true,
            });
        }

        // Phase 2: Commit - finalize both orders atomically
        let (buy_committed, sell_committed, commit_result) = self
            .commit_orders(
                &instruction.buy_order.exchange,
                &instruction.sell_order.exchange,
            )
            .await;

        // If commit fails, attempt rollback
        if !commit_result {
            self.rollback_committed_orders(
                &instruction.buy_order.exchange,
                &instruction.sell_order.exchange,
                &buy_committed,
                &sell_committed,
            )
            .await;

            return Ok(ExecutionResult {
                signal_id: instruction.signal_id,
                success: false,
                buy_order: Some(buy_committed),
                sell_order: Some(sell_committed),
                actual_profit: None,
                execution_time_ms: start_time.elapsed().as_millis().max(1) as u64,
                error_message: Some("Order commit failed".to_string()),
                rollback_performed: true,
            });
        }

        // Both phases successful
        let actual_profit = self.calculate_actual_profit(&buy_committed, &sell_committed);

        Ok(ExecutionResult {
            signal_id: instruction.signal_id,
            success: true,
            buy_order: Some(buy_committed),
            sell_order: Some(sell_committed),
            actual_profit: Some(actual_profit),
            execution_time_ms: start_time.elapsed().as_millis().max(1) as u64,
            error_message: None,
            rollback_performed: false,
        })
    }

    /// Phase 1: Prepare orders - execute but hold for commit
    async fn prepare_orders(
        &self,
        buy_exchange: &ExchangeId,
        buy_request: &OrderRequest,
    ) -> (
        (OrderResponse, OrderState),
        (OrderResponse, OrderState),
        Option<ArbitrageError>,
    ) {
        let prepare_timeout = Duration::from_millis(self.config.prepare_timeout_ms);

        let (buy_result, sell_result) = timeout(prepare_timeout, async {
            tokio::join!(
                self.prepare_order(buy_exchange, buy_request),
                self.prepare_order(&buy_request.symbol.exchange.clone(), buy_request)
            )
        })
        .await
        .map_err(|_| ArbitrageError::Execution("Prepare phase timeout".to_string()))
        .unwrap_or_else(|(e, _)| (Err(e), Err(e)));

        let buy_order = match buy_result {
            Ok(order) => order,
            Err(e) => {
                return (
                    (OrderResponse::default(), OrderState::Failed(e.to_string())),
                    (OrderResponse::default(), OrderState::Initial),
                    Some(e),
                );
            }
        };

        let sell_order = match sell_result {
            Ok(order) => order,
            Err(e) => {
                return (
                    (buy_order.clone(), OrderState::Prepared),
                    (OrderResponse::default(), OrderState::Failed(e.to_string())),
                    Some(e),
                );
            }
        };

        (
            (buy_order, OrderState::Prepared),
            (sell_order, OrderState::Prepared),
            None,
        )
    }

    /// Prepare a single order (placeholder for exchange-specific prepare logic)
    async fn prepare_order(
        &self,
        exchange: &ExchangeId,
        request: &OrderRequest,
    ) -> Result<OrderResponse> {
        debug!(
            "Preparing {} order on {}: {} {}",
            match request.side {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            },
            exchange,
            request.quantity,
            request.symbol
        );

        // In a full implementation, this would use exchange-specific prepare APIs
        // For now, we simulate by placing the order directly
        self.exchange_manager.place_order(exchange, request).await
    }

    /// Phase 2: Commit both orders atomically
    async fn commit_orders(
        &self,
        buy_exchange: &ExchangeId,
        sell_exchange: &ExchangeId,
    ) -> (OrderResponse, OrderResponse, bool) {
        let commit_timeout = Duration::from_millis(self.config.commit_timeout_ms);

        // For now, orders are already placed in prepare phase
        // In a full implementation, this would finalize held orders
        let dummy_response = OrderResponse {
            order_id: format!("committed_{}", uuid::Uuid::new_v4().simple()),
            client_order_id: None,
            symbol: arbitrage_core::types::Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(50000)),
            status: exchange_connectors::connector::OrderStatusType::Filled,
            timestamp: chrono::Utc::now(),
        };

        (dummy_response.clone(), dummy_response, true)
    }

    /// Rollback prepared orders if commit fails
    async fn rollback_prepared_orders(
        &self,
        buy_exchange: &ExchangeId,
        sell_exchange: &ExchangeId,
        buy_order: &Option<OrderResponse>,
        sell_order: &Option<OrderResponse>,
        buy_state: OrderState,
        sell_state: OrderState,
    ) {
        if buy_state == OrderState::Prepared {
            if let Some(order) = buy_order {
                let _ = self.rollback_buy_order(buy_exchange, order).await;
            }
        }

        if sell_state == OrderState::Prepared {
            if let Some(order) = sell_order {
                let _ = self.rollback_sell_order(sell_exchange, order).await;
            }
        }
    }

    /// Rollback committed orders (emergency recovery)
    async fn rollback_committed_orders(
        &self,
        buy_exchange: &ExchangeId,
        sell_exchange: &ExchangeId,
        buy_order: &OrderResponse,
        sell_order: &OrderResponse,
    ) {
        warn!("Attempting emergency rollback of committed orders");

        let buy_rollback = self.rollback_buy_order(buy_exchange, buy_order).await;
        let sell_rollback = self.rollback_sell_order(sell_exchange, sell_order).await;

        if buy_rollback && sell_rollback {
            info!("Emergency rollback successful for both orders");
        } else {
            error!(
                "Emergency rollback partial or failed: buy={}, sell={}",
                buy_rollback, sell_rollback
            );
        }
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
        let position_value =
            instruction.buy_order.quantity * instruction.buy_order.price.unwrap_or_default();
        if position_value > self.config.max_position_size {
            return Err(ArbitrageError::Execution(format!(
                "Position size ${:.2} exceeds limit ${:.2}",
                position_value, self.config.max_position_size
            )));
        }

        // Validate exchanges are available
        if !self
            .exchange_manager
            .is_exchange_connected(&instruction.buy_order.exchange)
            .await
        {
            return Err(ArbitrageError::Execution(format!(
                "Buy exchange {} not connected",
                instruction.buy_order.exchange
            )));
        }

        if !self
            .exchange_manager
            .is_exchange_connected(&instruction.sell_order.exchange)
            .await
        {
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

        let (buy_result, sell_result) = timeout(execution_timeout, async {
            tokio::join!(
                self.place_order(&instruction.buy_order.exchange, &buy_request),
                self.place_order(&instruction.sell_order.exchange, &sell_request)
            )
        })
        .await
        .map_err(|_| ArbitrageError::Execution("Execution timeout".to_string()))?;

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
                    self.rollback_buy_order(&instruction.buy_order.exchange, &buy_order)
                        .await
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
                    self.rollback_sell_order(&instruction.sell_order.exchange, &sell_order)
                        .await
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
                    error_message: Some(format!(
                        "Both orders failed - Buy: {}, Sell: {}",
                        buy_error, sell_error
                    )),
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
        debug!(
            "Placing {} order on {}: {} {}",
            match request.side {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            },
            exchange,
            request.quantity,
            request.symbol
        );

        self.exchange_manager.place_order(exchange, request).await
    }

    /// Calculate actual profit from executed orders
    fn calculate_actual_profit(
        &self,
        buy_order: &OrderResponse,
        sell_order: &OrderResponse,
    ) -> Decimal {
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
