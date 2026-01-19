//! # Order Executor Tests
//!
//! Comprehensive tests for the order execution service including simultaneous execution,
//! rollback logic, and error handling.

use arbitrage_core::{
    types::{ExchangeId, ExecutionInstruction, Order, OrderType, Side, Symbol, TimeInForce},
    ArbitrageError,
};
use arbitrage_server::order_executor::{ExecutorConfig, OrderExecutor};
use exchange_connectors::{
    connector::{
        AssetBalance, Balance, CancelResponse, ConnectorStats, ExchangeConnector, FundingRate,
        HealthStatus, OrderRequest, OrderResponse, OrderSide, OrderStatus, OrderStatusType,
        TickerData,
    },
    exchange_manager::{ExchangeManager, ExchangeManagerConfig},
};
use rust_decimal::Decimal;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// Mock exchange connector for testing
struct MockExchangeConnector {
    exchange_id: ExchangeId,
    should_fail_orders: bool,
    order_delay_ms: u64,
}

impl MockExchangeConnector {
    fn new(exchange_id: ExchangeId) -> Self {
        Self {
            exchange_id,
            should_fail_orders: false,
            order_delay_ms: 0,
        }
    }

    fn with_failure(mut self, should_fail: bool) -> Self {
        self.should_fail_orders = should_fail;
        self
    }

    fn with_delay(mut self, delay_ms: u64) -> Self {
        self.order_delay_ms = delay_ms;
        self
    }
}

#[async_trait::async_trait]
impl ExchangeConnector for MockExchangeConnector {
    fn exchange_id(&self) -> ExchangeId {
        self.exchange_id
    }

    fn status(&self) -> arbitrage_core::types::ConnectionStatus {
        arbitrage_core::types::ConnectionStatus::Connected
    }

    fn event_receiver(
        &self,
    ) -> tokio::sync::broadcast::Receiver<exchange_connectors::events::ConnectionEvent> {
        let (tx, rx) = tokio::sync::broadcast::channel(1);
        rx
    }

    async fn fetch_order_book(
        &self,
        _symbol: &Symbol,
    ) -> arbitrage_core::Result<arbitrage_core::types::OrderBook> {
        unimplemented!("Not needed for executor tests")
    }

    async fn fetch_symbols(&self) -> arbitrage_core::Result<Vec<Symbol>> {
        unimplemented!("Not needed for executor tests")
    }

    async fn fetch_tickers(
        &self,
        _symbols: &[Symbol],
    ) -> arbitrage_core::Result<HashMap<Symbol, TickerData>> {
        unimplemented!("Not needed for executor tests")
    }

    async fn fetch_funding_rates(
        &self,
        _symbols: &[Symbol],
    ) -> arbitrage_core::Result<HashMap<Symbol, FundingRate>> {
        unimplemented!("Not needed for executor tests")
    }

    async fn connect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn disconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn subscribe_symbols(&mut self, _symbols: &[Symbol]) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn unsubscribe_symbols(&mut self, _symbols: &[Symbol]) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn subscribe_tickers(&mut self, _symbols: &[Symbol]) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn subscribe_order_books(&mut self, _symbols: &[Symbol]) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn subscribe_trades(&mut self, _symbols: &[Symbol]) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn subscribe_funding_rates(&mut self, _symbols: &[Symbol]) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> arbitrage_core::Result<HealthStatus> {
        Ok(HealthStatus {
            is_connected: true,
            last_message_time: Some(chrono::Utc::now()),
            websocket_status: arbitrage_core::types::ConnectionStatus::Connected,
            rest_api_status: arbitrage_core::types::ConnectionStatus::Connected,
            error_count: 0,
            reconnect_count: 0,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        ConnectorStats::default()
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    // Trading methods
    async fn place_order(&self, order: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        if self.order_delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.order_delay_ms)).await;
        }

        if self.should_fail_orders {
            return Err(ArbitrageError::ExchangeConnection(
                "Mock order failure".to_string(),
            ));
        }

        Ok(OrderResponse {
            order_id: format!("mock_order_{}", Uuid::new_v4()),
            client_order_id: order.client_order_id.clone(),
            symbol: order.symbol.clone(),
            side: order.side.clone(),
            order_type: order.order_type.clone(),
            quantity: order.quantity,
            price: order.price,
            status: OrderStatusType::Filled,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn cancel_order(&self, order_id: &str) -> arbitrage_core::Result<CancelResponse> {
        Ok(CancelResponse {
            order_id: order_id.to_string(),
            client_order_id: None,
            status: OrderStatusType::Cancelled,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: None,
            symbol: Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: exchange_connectors::connector::OrderType::Market,
            quantity: Decimal::new(1, 1),
            price: Some(Decimal::new(50000, 0)),
            filled_quantity: Decimal::new(1, 1),
            remaining_quantity: Decimal::ZERO,
            average_price: Some(Decimal::new(50000, 0)),
            status: OrderStatusType::Filled,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
        let mut balances = HashMap::new();
        balances.insert(
            "BTC".to_string(),
            AssetBalance {
                asset: "BTC".to_string(),
                free: Decimal::new(10, 0),
                locked: Decimal::ZERO,
                total: Decimal::new(10, 0),
            },
        );
        balances.insert(
            "USDT".to_string(),
            AssetBalance {
                asset: "USDT".to_string(),
                free: Decimal::new(100000, 0),
                locked: Decimal::ZERO,
                total: Decimal::new(100000, 0),
            },
        );

        Ok(Balance {
            exchange: self.exchange_id,
            balances,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_open_orders(
        &self,
        _symbol: Option<&Symbol>,
    ) -> arbitrage_core::Result<Vec<OrderStatus>> {
        Ok(Vec::new())
    }
}

/// Helper function to create test execution instruction
fn create_test_execution() -> ExecutionInstruction {
    let signal_id = Uuid::new_v4();
    let symbol = Symbol::new("BTC", "USDT");

    let buy_order = Order::new(
        ExchangeId::OKX,
        symbol.clone(),
        Side::Buy,
        OrderType::Market,
        Decimal::new(1, 1), // 0.1 BTC
        Some(Decimal::new(50000, 0)),
    );

    let sell_order = Order::new(
        ExchangeId::ByBit,
        symbol.clone(),
        Side::Sell,
        OrderType::Market,
        Decimal::new(1, 1), // 0.1 BTC
        Some(Decimal::new(50100, 0)),
    );

    let mut instruction = ExecutionInstruction::new(signal_id, buy_order, sell_order);
    instruction.expected_profit = Decimal::new(200, 4); // 2%
    instruction.slippage_buffer = Decimal::new(50, 4); // 0.5%

    instruction
}

/// Helper function to create mock exchange manager
async fn create_mock_exchange_manager() -> Arc<ExchangeManager> {
    let config = ExchangeManagerConfig::default();
    let mut manager = ExchangeManager::new(config);

    // Add mock connectors
    manager
        .add_connector(Box::new(MockExchangeConnector::new(ExchangeId::OKX)))
        .await
        .unwrap();
    manager
        .add_connector(Box::new(MockExchangeConnector::new(ExchangeId::ByBit)))
        .await
        .unwrap();

    Arc::new(manager)
}

#[tokio::test]
async fn test_successful_arbitrage_execution() {
    let exchange_manager = create_mock_exchange_manager().await;
    let config = ExecutorConfig::default();
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution();

    let result = executor
        .execute_arbitrage(&instruction)
        .await
        .expect("Execution failed");

    assert!(result.success);
    assert!(result.buy_order.is_some());
    assert!(result.sell_order.is_some());
    assert!(result.actual_profit.is_some());
    assert!(result.execution_time_ms > 0);
    assert!(!result.rollback_performed);
    assert!(result.error_message.is_none());
}

#[tokio::test]
async fn test_buy_order_failure_with_rollback() {
    let config = ExchangeManagerConfig::default();
    let mut manager = ExchangeManager::new(config);

    // Add failing buy exchange and successful sell exchange
    manager
        .add_connector(Box::new(
            MockExchangeConnector::new(ExchangeId::OKX).with_failure(true),
        ))
        .await
        .unwrap();
    manager
        .add_connector(Box::new(MockExchangeConnector::new(ExchangeId::ByBit)))
        .await
        .unwrap();

    let exchange_manager = Arc::new(manager);
    let config = ExecutorConfig {
        enable_rollback: true,
        ..Default::default()
    };
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution();

    let result = executor
        .execute_arbitrage(&instruction)
        .await
        .expect("Execution failed");

    assert!(!result.success);
    assert!(result.buy_order.is_none());
    assert!(result.sell_order.is_some());
    assert!(result.actual_profit.is_none());
    assert!(result.error_message.is_some());
    assert!(result.rollback_performed);
}

#[tokio::test]
async fn test_sell_order_failure_with_rollback() {
    let config = ExchangeManagerConfig::default();
    let mut manager = ExchangeManager::new(config);

    // Add successful buy exchange and failing sell exchange
    manager
        .add_connector(Box::new(MockExchangeConnector::new(ExchangeId::OKX)))
        .await
        .unwrap();
    manager
        .add_connector(Box::new(
            MockExchangeConnector::new(ExchangeId::ByBit).with_failure(true),
        ))
        .await
        .unwrap();

    let exchange_manager = Arc::new(manager);
    let config = ExecutorConfig {
        enable_rollback: true,
        ..Default::default()
    };
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution();

    let result = executor
        .execute_arbitrage(&instruction)
        .await
        .expect("Execution failed");

    assert!(!result.success);
    assert!(result.buy_order.is_some());
    assert!(result.sell_order.is_none());
    assert!(result.actual_profit.is_none());
    assert!(result.error_message.is_some());
    assert!(result.rollback_performed);
}

#[tokio::test]
async fn test_both_orders_failure() {
    let config = ExchangeManagerConfig::default();
    let mut manager = ExchangeManager::new(config);

    // Add failing exchanges
    manager
        .add_connector(Box::new(
            MockExchangeConnector::new(ExchangeId::OKX).with_failure(true),
        ))
        .await
        .unwrap();
    manager
        .add_connector(Box::new(
            MockExchangeConnector::new(ExchangeId::ByBit).with_failure(true),
        ))
        .await
        .unwrap();

    let exchange_manager = Arc::new(manager);
    let config = ExecutorConfig::default();
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution();

    let result = executor
        .execute_arbitrage(&instruction)
        .await
        .expect("Execution failed");

    assert!(!result.success);
    assert!(result.buy_order.is_none());
    assert!(result.sell_order.is_none());
    assert!(result.actual_profit.is_none());
    assert!(result.error_message.is_some());
    assert!(!result.rollback_performed);
}

#[tokio::test]
async fn test_execution_timeout() {
    let config = ExchangeManagerConfig::default();
    let mut manager = ExchangeManager::new(config);

    // Add slow exchanges
    manager
        .add_connector(Box::new(
            MockExchangeConnector::new(ExchangeId::OKX).with_delay(3000),
        ))
        .await
        .unwrap();
    manager
        .add_connector(Box::new(
            MockExchangeConnector::new(ExchangeId::ByBit).with_delay(3000),
        ))
        .await
        .unwrap();

    let exchange_manager = Arc::new(manager);
    let config = ExecutorConfig {
        execution_timeout_ms: 1000, // Short timeout
        ..Default::default()
    };
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution();

    let result = executor
        .execute_arbitrage(&instruction)
        .await
        .expect("Execution failed");

    assert!(!result.success);
    assert!(result.error_message.is_some());
    assert!(result.error_message.as_ref().unwrap().contains("timeout"));
}

#[tokio::test]
async fn test_profit_threshold_validation() {
    let exchange_manager = create_mock_exchange_manager().await;
    let config = ExecutorConfig {
        min_profit_threshold: Decimal::new(500, 4), // 5% minimum
        ..Default::default()
    };
    let executor = OrderExecutor::new(config, exchange_manager);

    let mut instruction = create_test_execution();
    instruction.expected_profit = Decimal::new(100, 4); // Only 1% profit

    let result = executor.execute_arbitrage(&instruction).await;

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("below threshold"));
}

#[tokio::test]
async fn test_position_size_validation() {
    let exchange_manager = create_mock_exchange_manager().await;
    let config = ExecutorConfig {
        max_position_size: Decimal::new(1000, 0), // $1000 max
        ..Default::default()
    };
    let executor = OrderExecutor::new(config, exchange_manager);

    let mut instruction = create_test_execution();
    instruction.buy_order.quantity = Decimal::new(1, 0); // 1 BTC = $50,000

    let result = executor.execute_arbitrage(&instruction).await;

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("exceeds limit"));
}

#[tokio::test]
async fn test_exchange_connectivity_validation() {
    // Create manager with only one exchange
    let config = ExchangeManagerConfig::default();
    let mut manager = ExchangeManager::new(config);
    manager
        .add_connector(Box::new(MockExchangeConnector::new(ExchangeId::OKX)))
        .await
        .unwrap();

    let exchange_manager = Arc::new(manager);
    let config = ExecutorConfig::default();
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution(); // Requires both OKX and ByBit

    let result = executor.execute_arbitrage(&instruction).await;

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("not connected"));
}

#[tokio::test]
async fn test_rollback_disabled() {
    let config = ExchangeManagerConfig::default();
    let mut manager = ExchangeManager::new(config);

    // Add failing buy exchange and successful sell exchange
    manager
        .add_connector(Box::new(
            MockExchangeConnector::new(ExchangeId::OKX).with_failure(true),
        ))
        .await
        .unwrap();
    manager
        .add_connector(Box::new(MockExchangeConnector::new(ExchangeId::ByBit)))
        .await
        .unwrap();

    let exchange_manager = Arc::new(manager);
    let config = ExecutorConfig {
        enable_rollback: false, // Disable rollback
        ..Default::default()
    };
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution();

    let result = executor
        .execute_arbitrage(&instruction)
        .await
        .expect("Execution failed");

    assert!(!result.success);
    assert!(!result.rollback_performed); // Should not perform rollback
}

#[tokio::test]
async fn test_concurrent_executions() {
    let exchange_manager = create_mock_exchange_manager().await;
    let config = ExecutorConfig::default();
    let executor = Arc::new(OrderExecutor::new(config, exchange_manager));

    // Execute multiple arbitrage opportunities concurrently
    let mut handles = Vec::new();

    for i in 0..5 {
        let executor_clone = executor.clone();
        let handle = tokio::spawn(async move {
            let mut instruction = create_test_execution();
            instruction.signal_id = Uuid::new_v4();
            instruction.buy_order.quantity = Decimal::new(i + 1, 2); // 0.01, 0.02, 0.03, 0.04, 0.05 BTC
            instruction.sell_order.quantity = Decimal::new(i + 1, 2);

            executor_clone.execute_arbitrage(&instruction).await
        });
        handles.push(handle);
    }

    // Wait for all executions to complete
    let mut successful_executions = 0;
    for handle in handles {
        let result = handle
            .await
            .expect("Task panicked")
            .expect("Execution failed");
        if result.success {
            successful_executions += 1;
        }
    }

    assert_eq!(successful_executions, 5);
}

#[tokio::test]
async fn test_actual_profit_calculation() {
    let exchange_manager = create_mock_exchange_manager().await;
    let config = ExecutorConfig::default();
    let executor = OrderExecutor::new(config, exchange_manager);

    let instruction = create_test_execution();

    let result = executor
        .execute_arbitrage(&instruction)
        .await
        .expect("Execution failed");

    assert!(result.success);
    assert!(result.actual_profit.is_some());

    let actual_profit = result.actual_profit.unwrap();
    assert!(actual_profit > Decimal::ZERO);

    // Verify profit calculation makes sense
    // Buy at 50000, sell at 50100, quantity 0.1 BTC
    // Expected profit: (50100 - 50000) / 50000 * 0.1 = 0.002 (0.2%)
    let expected_profit = Decimal::new(2, 4); // 0.002
    assert!((actual_profit - expected_profit).abs() < Decimal::new(1, 5)); // Within 0.001% tolerance
}

#[tokio::test]
async fn test_config_updates() {
    let exchange_manager = create_mock_exchange_manager().await;
    let config = ExecutorConfig::default();
    let mut executor = OrderExecutor::new(config, exchange_manager);

    // Update configuration
    let new_config = ExecutorConfig {
        execution_timeout_ms: 10000,
        max_slippage_percent: Decimal::new(100, 4), // 1%
        enable_rollback: false,
        min_profit_threshold: Decimal::new(50, 4), // 0.5%
        max_position_size: Decimal::new(50000, 0), // $50,000
    };

    executor.update_config(new_config.clone());

    // Verify config was updated
    let current_config = executor.get_config();
    assert_eq!(
        current_config.execution_timeout_ms,
        new_config.execution_timeout_ms
    );
    assert_eq!(
        current_config.max_slippage_percent,
        new_config.max_slippage_percent
    );
    assert_eq!(current_config.enable_rollback, new_config.enable_rollback);
    assert_eq!(
        current_config.min_profit_threshold,
        new_config.min_profit_threshold
    );
    assert_eq!(
        current_config.max_position_size,
        new_config.max_position_size
    );
}
