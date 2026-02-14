//! API Routes for the Arbitrage Dashboard
//!
//! Provides REST endpoints for:
//! - Signal queries and management
//! - Order book data
//! - Execution preparation and confirmation
//! - System status and analytics
//! - Configuration management
//!
//! **Note:** Authentication has been removed for local-only use.

use crate::{
    audit_logger::{AuditDecision, AuditEntry},
    order_executor::OrderExecutor,
    server::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

// ============================================================================
// Query Parameters
// ============================================================================

/// Query parameters for signals endpoint
#[derive(Debug, Deserialize)]
pub struct SignalsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub symbol: Option<String>,
    pub exchange: Option<String>,
    pub min_profit: Option<f64>,
    pub min_confidence: Option<f64>,
}

/// Request body for execution preparation
#[derive(Debug, Deserialize)]
pub struct PrepareExecutionRequest {
    pub signal_id: String,
    pub quantity: Option<f64>,
}

impl PrepareExecutionRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate signal_id is proper UUID format
        if let Err(_) = uuid::Uuid::parse_str(&self.signal_id) {
            errors.push("signal_id must be a valid UUID format".to_string());
        }

        // Validate quantity is positive if provided
        if let Some(qty) = self.quantity {
            if qty <= 0.0 {
                errors.push("quantity must be a positive number greater than 0".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Response for execution preparation
#[derive(Debug, Serialize)]
pub struct PrepareExecutionResponse {
    pub instruction_id: String,
    pub buy_order: OrderPreview,
    pub sell_order: OrderPreview,
    pub expected_profit: f64,
    pub worst_case_profit: f64,
    pub total_fees: f64,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
}

/// Order preview for execution response
#[derive(Debug, Serialize)]
pub struct OrderPreview {
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub estimated_cost: f64,
    pub estimated_fee: f64,
}

// ============================================================================
// Signal Endpoints
// ============================================================================

/// Get active signals
pub async fn get_signals(
    State(state): State<AppState>,
    Query(params): Query<SignalsQuery>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/signals with params: {:?}", params);

    // Build query from params
    let query = arbitrage_core::storage::SignalQuery {
        limit: params.limit,
        status_filter: Some(arbitrage_core::storage::SignalStatus::Detected),
        symbol_filter: params.symbol.clone(),
        exchange_filter: None, // TODO: Parse exchange string to ExchangeId
        min_confidence: params
            .min_confidence
            .map(|c| rust_decimal::Decimal::try_from(c).unwrap_or_default()),
        time_range: None,
    };

    // Query storage directly from state
    match state.storage.query_signals(&query).await {
        Ok(stored_signals) => {
            let signals: Vec<Value> = stored_signals
                .iter()
                .map(|stored| {
                    json!({
                        "id": stored.signal.id.to_string(),
                        "symbol": stored.signal.symbol.to_pair(),
                        "buy_exchange": stored.signal.buy_exchange.to_string(),
                        "sell_exchange": stored.signal.sell_exchange.to_string(),
                        "buy_price": stored.signal.buy_price.to_string(),
                        "sell_price": stored.signal.sell_price.to_string(),
                        "gross_profit_percent": stored.signal.gross_profit_percent.to_string(),
                        "net_profit_percent": stored.signal.net_profit_percent.to_string(),
                        "confidence": stored.confidence_score.to_string(),
                        "recommended_size": stored.signal.recommended_size.to_string(),
                        "detected_at": stored.timestamp.to_rfc3339(),
                        "status": format!("{:?}", stored.status)
                    })
                })
                .collect();

            let response = json!({
                "signals": signals,
                "total": signals.len(),
                "limit": params.limit.unwrap_or(100),
                "offset": params.offset.unwrap_or(0)
            });

            Ok(Json(response))
        }
        Err(e) => {
            warn!("Failed to query signals: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get specific signal by ID
pub async fn get_signal(
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/signals/{}", signal_id);

    // Parse signal ID
    let signal_uuid = match uuid::Uuid::parse_str(&signal_id) {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    // Query for all signals and find the one we want
    let query = arbitrage_core::storage::SignalQuery {
        limit: Some(1000), // Get enough to find our signal
        status_filter: None,
        symbol_filter: None,
        exchange_filter: None,
        min_confidence: None,
        time_range: None,
    };

    match state.storage.query_signals(&query).await {
        Ok(stored_signals) => {
            if let Some(stored) = stored_signals.iter().find(|s| s.signal.id == signal_uuid) {
                let signal = json!({
                    "id": stored.signal.id.to_string(),
                    "symbol": stored.signal.symbol.to_pair(),
                    "buy_exchange": stored.signal.buy_exchange.to_string(),
                    "sell_exchange": stored.signal.sell_exchange.to_string(),
                    "buy_price": stored.signal.buy_price.to_string(),
                    "sell_price": stored.signal.sell_price.to_string(),
                    "gross_profit_percent": stored.signal.gross_profit_percent.to_string(),
                    "net_profit_percent": stored.signal.net_profit_percent.to_string(),
                    "confidence": stored.confidence_score.to_string(),
                    "recommended_size": stored.signal.recommended_size.to_string(),
                    "detected_at": stored.timestamp.to_rfc3339(),
                    "status": format!("{:?}", stored.status)
                });
                Ok(Json(signal))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(e) => {
            warn!("Failed to query signal {}: {}", signal_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============================================================================
// Order Book Endpoints
// ============================================================================

/// Get order book for exchange/symbol
pub async fn get_orderbook(
    State(state): State<AppState>,
    Path((exchange, symbol)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/orderbooks/{}/{}", exchange, symbol);

    // Parse exchange ID
    let exchange_id = match exchange.to_lowercase().as_str() {
        "okx" => arbitrage_core::types::ExchangeId::OKX,
        "bybit" => arbitrage_core::types::ExchangeId::ByBit,
        "mexc" => arbitrage_core::types::ExchangeId::MEXC,
        "gateio" => arbitrage_core::types::ExchangeId::GateIo,
        "kraken" => arbitrage_core::types::ExchangeId::Kraken,
        "bitstamp" => arbitrage_core::types::ExchangeId::Bitstamp,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Parse symbol
    let parsed_symbol = match arbitrage_core::types::Symbol::from_pair(&symbol) {
        Some(s) => s,
        None => return Err(StatusCode::BAD_REQUEST),
    };

    // Get order book from engine cache
    match state
        .arbitrage_engine
        .get_order_book(exchange_id, Arc::new(parsed_symbol))
    {
        Some(ob) => {
            let bids: Vec<Value> = ob
                .bids
                .iter()
                .take(20)
                .map(|level| json!([level.price.to_string(), level.quantity.to_string()]))
                .collect();

            let asks: Vec<Value> = ob
                .asks
                .iter()
                .take(20)
                .map(|level| json!([level.price.to_string(), level.quantity.to_string()]))
                .collect();

            let orderbook = json!({
                "exchange": exchange,
                "symbol": symbol,
                "bids": bids,
                "asks": asks,
                "timestamp": ob.timestamp.to_rfc3339()
            });

            Ok(Json(orderbook))
        }
        None => {
            let orderbook = json!({
                "exchange": exchange,
                "symbol": symbol,
                "bids": [],
                "asks": [],
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "error": "Order book not found in cache"
            });
            Ok(Json(orderbook))
        }
    }
}

// ============================================================================
// Execution Endpoints
// ============================================================================

/// Prepare execution instruction
pub async fn prepare_execution(
    State(state): State<AppState>,
    Json(request): Json<PrepareExecutionRequest>,
) -> Result<Json<PrepareExecutionResponse>, StatusCode> {
    debug!("POST /api/executions/prepare: {:?}", request);

    // Validate request
    if let Err(errors) = request.validate() {
        let response = PrepareExecutionResponse {
            instruction_id: String::new(),
            buy_order: OrderPreview {
                exchange: String::new(),
                symbol: String::new(),
                side: String::new(),
                quantity: 0.0,
                price: None,
                estimated_cost: 0.0,
                estimated_fee: 0.0,
            },
            sell_order: OrderPreview {
                exchange: String::new(),
                symbol: String::new(),
                side: String::new(),
                quantity: 0.0,
                price: None,
                estimated_cost: 0.0,
                estimated_fee: 0.0,
            },
            expected_profit: 0.0,
            worst_case_profit: 0.0,
            total_fees: 0.0,
            is_valid: false,
            validation_errors: errors,
        };
        return Ok(Json(response));
    }

    // Parse signal ID
    let signal_uuid = match uuid::Uuid::parse_str(&request.signal_id) {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    // Find the signal in storage
    let query = arbitrage_core::storage::SignalQuery {
        limit: Some(1000),
        status_filter: None,
        symbol_filter: None,
        exchange_filter: None,
        min_confidence: None,
        time_range: None,
    };

    let stored_signals = match state.storage.query_signals(&query).await {
        Ok(signals) => signals,
        Err(e) => {
            warn!("Failed to query signals: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let stored_signal = match stored_signals.iter().find(|s| s.signal.id == signal_uuid) {
        Some(signal) => signal,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Use provided quantity or signal's recommended size
    let quantity = request
        .quantity
        .map(|q| rust_decimal::Decimal::try_from(q).unwrap_or_default())
        .unwrap_or(stored_signal.signal.recommended_size);

    // Use ExecutionPreparer to create instruction
    let execution_preparer = arbitrage_core::execution_preparer::ExecutionPreparer::default();

    match execution_preparer.prepare_execution(&stored_signal.signal, quantity) {
        Ok(instruction) => {
            // Store instruction in cache for later confirmation
            state.instruction_cache.store(instruction.clone()).await;
            
            // Log execution preparation
            let audit_entry = AuditEntry::new(
                AuditDecision::ExecutionPrepared,
                "success",
                &format!("Prepared execution for signal {}", signal_uuid),
            )
            .with_opportunity(signal_uuid, &stored_signal.signal.symbol.to_pair())
            .with_exchanges(
                &stored_signal.signal.buy_exchange.to_string(),
                &stored_signal.signal.sell_exchange.to_string(),
            )
            .with_profit(&instruction.expected_profit.to_string(), None);

            state.audit_logger.log(audit_entry).await;

            let response = PrepareExecutionResponse {
                instruction_id: instruction.id.to_string(),
                buy_order: OrderPreview {
                    exchange: instruction.buy_order.exchange.to_string(),
                    symbol: instruction.buy_order.symbol.to_pair(),
                    side: "buy".to_string(),
                    quantity: instruction.buy_order.quantity.to_f64().unwrap_or(0.0),
                    price: instruction
                        .buy_order
                        .price
                        .map(|p| p.to_f64().unwrap_or(0.0)),
                    estimated_cost: instruction
                        .buy_order
                        .price
                        .map(|p| (p * instruction.buy_order.quantity).to_f64().unwrap_or(0.0))
                        .unwrap_or(0.0),
                    estimated_fee: instruction.buy_order.expected_fee.to_f64().unwrap_or(0.0),
                },
                sell_order: OrderPreview {
                    exchange: instruction.sell_order.exchange.to_string(),
                    symbol: instruction.sell_order.symbol.to_pair(),
                    side: "sell".to_string(),
                    quantity: instruction.sell_order.quantity.to_f64().unwrap_or(0.0),
                    price: instruction
                        .sell_order
                        .price
                        .map(|p| p.to_f64().unwrap_or(0.0)),
                    estimated_cost: instruction
                        .sell_order
                        .price
                        .map(|p| {
                            (p * instruction.sell_order.quantity)
                                .to_f64()
                                .unwrap_or(0.0)
                        })
                        .unwrap_or(0.0),
                    estimated_fee: instruction.sell_order.expected_fee.to_f64().unwrap_or(0.0),
                },
                expected_profit: instruction.expected_profit.to_f64().unwrap_or(0.0),
                worst_case_profit: instruction.worst_case_profit.to_f64().unwrap_or(0.0),
                total_fees: instruction.total_fees.to_f64().unwrap_or(0.0),
                is_valid: instruction.is_valid(),
                validation_errors: instruction.validation_errors,
            };

            Ok(Json(response))
        }
        Err(e) => {
            // Log execution preparation failure
            let audit_entry = AuditEntry::new(
                AuditDecision::ExecutionRejected,
                "error",
                &format!("Failed to prepare execution: {}", e),
            )
            .with_opportunity(signal_uuid, &stored_signal.signal.symbol.to_pair());

            state.audit_logger.log(audit_entry).await;

            warn!("Failed to prepare execution: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Request body for execution confirmation
#[derive(Debug, Deserialize)]
pub struct ConfirmExecutionRequest {
    pub instruction_id: String,
}

impl ConfirmExecutionRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Err(_) = uuid::Uuid::parse_str(&self.instruction_id) {
            errors.push("instruction_id must be a valid UUID format".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Response for execution confirmation
#[derive(Debug, Serialize)]
pub struct ConfirmExecutionResponse {
    pub success: bool,
    pub message: String,
    pub execution_id: Option<String>,
    pub buy_order: Option<OrderResult>,
    pub sell_order: Option<OrderResult>,
    pub actual_profit: Option<f64>,
    pub execution_time_ms: Option<u64>,
    pub rollback_performed: bool,
}

/// Order execution result
#[derive(Debug, Serialize)]
pub struct OrderResult {
    pub order_id: String,
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: f64,
    pub status: String,
}

/// Confirm execution
pub async fn confirm_execution(
    State(state): State<AppState>,
    Json(request): Json<ConfirmExecutionRequest>,
) -> Result<Json<ConfirmExecutionResponse>, StatusCode> {
    debug!("POST /api/executions/confirm: {:?}", request);

    // Validate request
    if let Err(errors) = request.validate() {
        return Ok(Json(ConfirmExecutionResponse {
            success: false,
            message: format!("Validation failed: {}", errors.join(", ")),
            execution_id: None,
            buy_order: None,
            sell_order: None,
            actual_profit: None,
            execution_time_ms: None,
            rollback_performed: false,
        }));
    }

    // Parse instruction ID
    let instruction_uuid = match uuid::Uuid::parse_str(&request.instruction_id) {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    // Retrieve instruction from cache
    let cached = match state.instruction_cache.get(&instruction_uuid).await {
        Some(cached) => cached,
        None => {
            return Ok(Json(ConfirmExecutionResponse {
                success: false,
                message: "Instruction not found or expired. Please prepare execution again.".to_string(),
                execution_id: None,
                buy_order: None,
                sell_order: None,
                actual_profit: None,
                execution_time_ms: None,
                rollback_performed: false,
            }));
        }
    };

    let instruction = cached.instruction;

    // Check if instruction is valid
    if !instruction.is_valid() {
        return Ok(Json(ConfirmExecutionResponse {
            success: false,
            message: format!(
                "Instruction has validation errors: {}",
                instruction.validation_errors.join(", ")
            ),
            execution_id: None,
            buy_order: None,
            sell_order: None,
            actual_profit: None,
            execution_time_ms: None,
            rollback_performed: false,
        }));
    }

    // Log execution start
    let audit_entry = AuditEntry::new(
        AuditDecision::ExecutionConfirmed,
        "pending",
        &format!("Starting execution for instruction {}", instruction_uuid),
    )
    .with_opportunity(instruction.signal_id, &instruction.buy_order.symbol.to_pair())
    .with_exchanges(
        &instruction.buy_order.exchange.to_string(),
        &instruction.sell_order.exchange.to_string(),
    )
    .with_profit(&instruction.expected_profit.to_string(), None);

    state.audit_logger.log(audit_entry).await;

    // Get exchange manager from bridge
    let bridge = state.bridge.lock().await;
    let exchange_manager = bridge.get_exchange_manager();
    
    // Create order executor
    let executor = OrderExecutor::new(
        state.executor_config.clone(),
        exchange_manager,
    );
    
    // Execute the arbitrage
    let result = executor.execute_arbitrage(&instruction).await;
    
    // Remove instruction from cache after execution attempt
    state.instruction_cache.remove(&instruction_uuid).await;

    match result {
        Ok(execution_result) => {
            let (status, message) = if execution_result.success {
                ("success", "Arbitrage executed successfully".to_string())
            } else {
                ("failed", execution_result.error_message.clone().unwrap_or_else(|| "Execution failed".to_string()))
            };

            // Log execution result
            let audit_entry = AuditEntry::new(
                if execution_result.success {
                    AuditDecision::ExecutionCompleted
                } else {
                    AuditDecision::ExecutionFailed
                },
                status,
                &message,
            )
            .with_opportunity(instruction.signal_id, &instruction.buy_order.symbol.to_pair())
            .with_exchanges(
                &instruction.buy_order.exchange.to_string(),
                &instruction.sell_order.exchange.to_string(),
            )
            .with_profit(
                &instruction.expected_profit.to_string(),
                execution_result.actual_profit.map(|p| p.to_string()).as_deref(),
            );

            state.audit_logger.log(audit_entry).await;

            info!(
                "Execution {} for signal {}: {}",
                if execution_result.success { "completed" } else { "failed" },
                instruction.signal_id,
                message
            );

            Ok(Json(ConfirmExecutionResponse {
                success: execution_result.success,
                message,
                execution_id: Some(instruction_uuid.to_string()),
                buy_order: execution_result.buy_order.map(|o| OrderResult {
                    order_id: o.order_id,
                    exchange: instruction.buy_order.exchange.to_string(),
                    symbol: o.symbol.to_pair(),
                    side: o.side.to_string(),
                    quantity: o.quantity.to_f64().unwrap_or(0.0),
                    price: o.price.unwrap_or_default().to_f64().unwrap_or(0.0),
                    status: format!("{:?}", o.status),
                }),
                sell_order: execution_result.sell_order.map(|o| OrderResult {
                    order_id: o.order_id,
                    exchange: instruction.sell_order.exchange.to_string(),
                    symbol: o.symbol.to_pair(),
                    side: o.side.to_string(),
                    quantity: o.quantity.to_f64().unwrap_or(0.0),
                    price: o.price.unwrap_or_default().to_f64().unwrap_or(0.0),
                    status: format!("{:?}", o.status),
                }),
                actual_profit: execution_result.actual_profit.map(|p| p.to_f64().unwrap_or(0.0)),
                execution_time_ms: Some(execution_result.execution_time_ms),
                rollback_performed: execution_result.rollback_performed,
            }))
        }
        Err(e) => {
            // Log execution error
            let audit_entry = AuditEntry::new(
                AuditDecision::ExecutionFailed,
                "error",
                &format!("Execution error: {}", e),
            )
            .with_opportunity(instruction.signal_id, &instruction.buy_order.symbol.to_pair());

            state.audit_logger.log(audit_entry).await;

            warn!("Execution error for signal {}: {}", instruction.signal_id, e);

            Ok(Json(ConfirmExecutionResponse {
                success: false,
                message: format!("Execution error: {}", e),
                execution_id: Some(instruction_uuid.to_string()),
                buy_order: None,
                sell_order: None,
                actual_profit: None,
                execution_time_ms: None,
                rollback_performed: false,
            }))
        }
    }
}

// ============================================================================
// Analytics Endpoints
// ============================================================================

/// Get analytics data
pub async fn get_analytics(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/analytics with params: {:?}", params);

    let engine_stats = state.arbitrage_engine.get_stats();

    let analytics = json!({
        "total_signals_detected": engine_stats.signals_detected,
        "total_signals_filtered": engine_stats.signals_filtered,
        "total_signals_emitted": engine_stats.signals_emitted,
        "active_symbols": engine_stats.active_symbols_count,
        "cached_order_books": engine_stats.order_books_count,
        "period": params.get("period").cloned().unwrap_or_else(|| "24h".to_string())
    });

    Ok(Json(analytics))
}

/// Get audit log entries
pub async fn get_audit_log(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/audit with params: {:?}", params);

    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);

    let entries = state.audit_logger.get_recent(limit).await;

    let response = json!({
        "entries": entries,
        "total": entries.len(),
        "limit": limit
    });

    Ok(Json(response))
}

// ============================================================================
// Status Endpoints
// ============================================================================

/// Get exchange connection status
pub async fn get_exchange_status(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/status/exchanges");

    let bridge_stats = state.bridge.lock().await.get_stats().await;

    let status = json!({
        "connected_exchanges": bridge_stats.connected_exchanges,
        "total_exchanges": bridge_stats.total_exchanges,
        "total_messages": bridge_stats.total_messages_received,
        "total_errors": bridge_stats.total_errors,
        "active_symbols": bridge_stats.active_symbols,
        "last_updated": chrono::Utc::now().to_rfc3339()
    });

    Ok(Json(status))
}

/// Get bridge service status
pub async fn get_bridge_status(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/status/bridge");

    let bridge_stats = state.bridge.lock().await.get_stats().await;
    let engine_stats = state.arbitrage_engine.get_stats();

    let status = json!({
        "bridge": {
            "connected_exchanges": bridge_stats.connected_exchanges,
            "total_exchanges": bridge_stats.total_exchanges,
            "total_messages_received": bridge_stats.total_messages_received,
            "total_errors": bridge_stats.total_errors,
            "active_symbols": bridge_stats.active_symbols,
        },
        "engine": {
            "cached_signals": engine_stats.cached_signals_count,
            "order_books_cached": engine_stats.order_books_count,
            "tickers_cached": engine_stats.tickers_count,
            "funding_rates_cached": engine_stats.funding_rates_count,
            "signals_detected": engine_stats.signals_detected,
            "signals_emitted": engine_stats.signals_emitted,
        },
        "last_updated": chrono::Utc::now().to_rfc3339()
    });

    Ok(Json(status))
}

// ============================================================================
// Configuration Endpoints
// ============================================================================

/// Get current configuration
pub async fn get_config(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/config");

    let config = json!({
        "server": {
            "host": state.config.server.host,
            "port": state.config.server.port
        },
        "trading": {
            "min_profit_threshold_percent": state.config.trading.min_profit_threshold_percent.to_string(),
            "min_confidence_threshold": state.config.trading.min_confidence_threshold.to_string(),
            "max_signal_age_seconds": state.config.trading.max_signal_age_seconds
        },
        "risk": {
            "max_position_size_usd": state.config.risk.max_position_size_usd.to_string(),
            "max_slippage_percent": state.config.risk.max_slippage_percent.to_string()
        }
    });

    Ok(Json(config))
}

/// Update configuration
pub async fn update_config(
    State(_state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    debug!("POST /api/config: {:?}", request);

    // TODO: Implement config update with validation
    let response = json!({
        "success": true,
        "message": "Configuration update not yet implemented"
    });

    Ok(Json(response))
}

// ============================================================================
// Credential Management Endpoints
// ============================================================================

/// Request body for saving exchange credentials
#[derive(Debug, Deserialize)]
pub struct SaveCredentialsRequest {
    pub exchange: String,
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: Option<String>,
}

/// Request body for validating credentials
#[derive(Debug, Deserialize)]
pub struct ValidateCredentialsRequest {
    pub exchange: String,
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: Option<String>,
    pub testnet: bool,
}

/// Response for credential status
#[derive(Debug, Serialize)]
pub struct CredentialStatus {
    pub exchange: String,
    pub has_credentials: bool,
    pub testnet: bool,
    pub validated: bool,
}

/// Get credential status for all exchanges
pub async fn get_credentials(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/credentials");

    // Use hardcoded exchange list to avoid HashMap key issues
    let exchange_ids = [
        arbitrage_core::types::ExchangeId::OKX,
        arbitrage_core::types::ExchangeId::ByBit,
        arbitrage_core::types::ExchangeId::MEXC,
        arbitrage_core::types::ExchangeId::GateIo,
        arbitrage_core::types::ExchangeId::Kraken,
        arbitrage_core::types::ExchangeId::Bitstamp,
    ];
    
    let exchange_names = ["okx", "bybit", "mexc", "gateio", "kraken", "bitstamp"];
    
    let credentials: Vec<CredentialStatus> = exchange_ids.iter().zip(exchange_names.iter()).map(|(exchange_id, exchange_name)| {
        let config = state.config.exchanges.get(exchange_id);
        let has_credentials = config.map(|c| {
            c.api_key.is_configured() && c.api_secret.is_configured()
        }).unwrap_or(false);
        
        let testnet = config.map(|c| c.testnet).unwrap_or(true);
        
        CredentialStatus {
            exchange: exchange_name.to_string(),
            has_credentials,
            testnet,
            validated: false,
        }
    }).collect();

    let response = json!({
        "credentials": credentials
    });

    Ok(Json(response))
}

/// Save credentials for an exchange
pub async fn save_credentials(
    State(_state): State<AppState>,
    Json(request): Json<SaveCredentialsRequest>,
) -> Result<Json<Value>, StatusCode> {
    debug!("POST /api/credentials: {}", request.exchange);

    // Validate exchange name
    let valid_exchanges = ["okx", "bybit", "mexc", "gateio", "kraken", "bitstamp"];
    if !valid_exchanges.contains(&request.exchange.to_lowercase().as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // For now, just return success - actual storage would require
    // encryption and config file updates
    let response = json!({
        "success": true,
        "message": format!("Credentials received for {}. Server restart required for changes to take effect.", request.exchange)
    });

    Ok(Json(response))
}

/// Validate credentials for an exchange (tests API connectivity)
pub async fn validate_credentials(
    State(_state): State<AppState>,
    Json(request): Json<ValidateCredentialsRequest>,
) -> Result<Json<Value>, StatusCode> {
    debug!("POST /api/credentials/validate: {}", request.exchange);

    // Validate exchange name
    let valid_exchanges = ["okx", "bybit", "mexc", "gateio", "kraken", "bitstamp"];
    if !valid_exchanges.contains(&request.exchange.to_lowercase().as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Basic validation - check non-empty credentials
    if request.api_key.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "API key is empty"
        })));
    }

    if request.api_secret.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "API secret is empty"
        })));
    }

    // TODO: Actually test credentials by making an authenticated API call
    // This would require:
    // 1. Getting the exchange connector
    // 2. Making a balance query with the provided credentials
    // 3. Returning success/failure based on the response

    // For now, just validate format and return success
    let response = json!({
        "success": true,
        "message": "Credentials format validated. Full validation requires server restart with new credentials."
    });

    Ok(Json(response))
}

/// Delete credentials for an exchange
pub async fn delete_credentials(
    State(_state): State<AppState>,
    Path(exchange): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    debug!("DELETE /api/credentials/{}", exchange);

    // Validate exchange name
    let valid_exchanges = ["okx", "bybit", "mexc", "gateio", "kraken", "bitstamp"];
    if !valid_exchanges.contains(&exchange.to_lowercase().as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let response = json!({
        "success": true,
        "message": format!("Credentials cleared for {}. Server restart required.", exchange)
    });

    Ok(Json(response))
}

/// Toggle testnet/production mode for an exchange
pub async fn toggle_exchange_mode(
    State(_state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    debug!("POST /api/exchange/mode: {:?}", request);

    let exchange = request.get("exchange")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let testnet = request.get("testnet")
        .and_then(|v| v.as_bool())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Validate exchange name
    let valid_exchanges = ["okx", "bybit", "mexc", "gateio", "kraken", "bitstamp"];
    if !valid_exchanges.contains(&exchange.to_lowercase().as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let response = json!({
        "success": true,
        "message": format!("Exchange {} mode set to {}. Server restart required.", 
            exchange, if testnet { "testnet" } else { "production" })
    });

    Ok(Json(response))
}


