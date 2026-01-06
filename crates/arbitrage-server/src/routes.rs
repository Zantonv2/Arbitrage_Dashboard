use crate::server::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, warn};

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

/// Get active signals
pub async fn get_signals(
    State(state): State<AppState>,
    Query(params): Query<SignalsQuery>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/signals with params: {:?}", params);

    // TODO: Implement actual signal retrieval from engine
    // For now, return empty array
    let signals = json!({
        "signals": [],
        "total": 0,
        "limit": params.limit.unwrap_or(100),
        "offset": params.offset.unwrap_or(0)
    });

    Ok(Json(signals))
}

/// Get specific signal by ID
pub async fn get_signal(
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/signals/{}", signal_id);

    // TODO: Implement signal lookup
    warn!("Signal lookup not implemented: {}", signal_id);
    Err(StatusCode::NOT_FOUND)
}

/// Get order book for exchange/symbol
pub async fn get_orderbook(
    State(state): State<AppState>,
    Path((exchange, symbol)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/orderbooks/{}/{}", exchange, symbol);

    // TODO: Parse exchange and symbol, lookup order book
    let orderbook = json!({
        "exchange": exchange,
        "symbol": symbol,
        "bids": [],
        "asks": [],
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(Json(orderbook))
}

/// Prepare execution instruction
pub async fn prepare_execution(
    State(state): State<AppState>,
    Json(request): Json<PrepareExecutionRequest>,
) -> Result<Json<PrepareExecutionResponse>, StatusCode> {
    debug!("POST /api/executions/prepare: {:?}", request);

    // TODO: Implement execution preparation
    // For now, return placeholder response
    let response = PrepareExecutionResponse {
        instruction_id: uuid::Uuid::new_v4().to_string(),
        buy_order: OrderPreview {
            exchange: "bybit".to_string(),
            symbol: "BTC/USDT".to_string(),
            side: "buy".to_string(),
            quantity: request.quantity.unwrap_or(1.0),
            price: Some(50000.0),
            estimated_cost: 50000.0,
            estimated_fee: 50.0,
        },
        sell_order: OrderPreview {
            exchange: "bingx".to_string(),
            symbol: "BTC/USDT".to_string(),
            side: "sell".to_string(),
            quantity: request.quantity.unwrap_or(1.0),
            price: Some(50100.0),
            estimated_cost: 50100.0,
            estimated_fee: 50.1,
        },
        expected_profit: 100.0,
        worst_case_profit: 50.0,
        total_fees: 100.1,
        is_valid: true,
        validation_errors: vec![],
    };

    Ok(Json(response))
}

/// Confirm execution
pub async fn confirm_execution(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    debug!("POST /api/executions/confirm: {:?}", request);

    // TODO: Implement execution confirmation
    let response = json!({
        "success": true,
        "message": "Execution confirmed (placeholder)"
    });

    Ok(Json(response))
}

/// Get analytics data
pub async fn get_analytics(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/analytics with params: {:?}", params);

    // TODO: Implement analytics calculation
    let analytics = json!({
        "total_signals": 0,
        "total_profit": 0.0,
        "win_rate": 0.0,
        "avg_profit_per_trade": 0.0,
        "period": params.get("period").unwrap_or(&"24h".to_string())
    });

    Ok(Json(analytics))
}

/// Get exchange connection status
pub async fn get_exchange_status(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/status/exchanges");

    // TODO: Get actual exchange status from connectors
    let status = json!({
        "exchanges": [
            {
                "id": "bybit",
                "status": "disconnected",
                "last_heartbeat": null,
                "uptime_percent": 0.0,
                "error_count": 0,
                "subscribed_symbols": []
            },
            {
                "id": "bingx",
                "status": "disconnected",
                "last_heartbeat": null,
                "uptime_percent": 0.0,
                "error_count": 0,
                "subscribed_symbols": []
            }
        ]
    });

    Ok(Json(status))
}

/// Get current configuration
pub async fn get_config(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    debug!("GET /api/config");

    // Return sanitized config (no sensitive data)
    let config = json!({
        "server": {
            "host": state.config.server.host,
            "port": state.config.server.port
        },
        "trading": {
            "min_profit_threshold_percent": state.config.trading.min_profit_threshold_percent,
            "min_confidence_threshold": state.config.trading.min_confidence_threshold,
            "max_signal_age_seconds": state.config.trading.max_signal_age_seconds
        },
        "risk": {
            "max_position_size_usd": state.config.risk.max_position_size_usd,
            "max_slippage_percent": state.config.risk.max_slippage_percent
        }
    });

    Ok(Json(config))
}

/// Update configuration
pub async fn update_config(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    debug!("POST /api/config: {:?}", request);

    // TODO: Implement config update
    let response = json!({
        "success": true,
        "message": "Configuration updated (placeholder)"
    });

    Ok(Json(response))
}

// ============================================================================
// RUSSIAN FINANCIAL ADVISOR STRATEGIES (8-22% DAILY RETURNS)
// ============================================================================

/// P2P RUB Strategy Endpoints
/// Expected: 20-50k RUB/day (4-10% daily)

#[derive(Debug, Serialize)]
pub struct P2PStatusResponse {
    pub bybit_connected: bool,
    pub htx_connected: bool,
    pub latest_bybit_rates: Option<P2PRatesResponse>,
    pub latest_htx_rates: Option<P2PRatesResponse>,
    pub active_opportunities: Vec<P2POpportunityResponse>,
    pub daily_potential: P2PDailyPotentialResponse,
}

#[derive(Debug, Serialize)]
pub struct P2PRatesResponse {
    pub exchange: String,
    pub sell_rate: Option<P2PRateResponse>,
    pub buy_rate: Option<P2PRateResponse>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct P2PRateResponse {
    pub rate: f64,
    pub volume: f64,
    pub min_order: f64,
    pub max_order: f64,
}

#[derive(Debug, Serialize)]
pub struct P2POpportunityResponse {
    pub id: String,
    pub sell_exchange: String,
    pub buy_exchange: String,
    pub sell_rate: f64,
    pub buy_rate: f64,
    pub spread_percent: f64,
    pub volume_usdt: f64,
    pub execution_time_min: u32,
    pub profit_rub_per_1000_usdt: f64,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct P2PDailyPotentialResponse {
    pub opportunities_count: usize,
    pub avg_spread_percent: f64,
    pub cycles_per_day: u32,
    pub daily_return_percent: f64,
    pub daily_profit_rub: f64,
    pub capital_rub: f64,
}

/// Get P2P RUB arbitrage status and opportunities
pub async fn get_p2p_status(
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<P2PStatusResponse>, StatusCode> {
    debug!("GET /api/strategies/p2p-rub");
    
    let capital_rub = params.get("capital_rub")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(500000.0); // Default 500k RUB
    
    // TODO: Get actual P2P status from P2PManager
    // For now, return mock data showing the potential
    let response = P2PStatusResponse {
        bybit_connected: true,
        htx_connected: true,
        latest_bybit_rates: Some(P2PRatesResponse {
            exchange: "bybit".to_string(),
            sell_rate: Some(P2PRateResponse {
                rate: 97.8,
                volume: 50000.0,
                min_order: 100.0,
                max_order: 50000.0,
            }),
            buy_rate: Some(P2PRateResponse {
                rate: 96.5,
                volume: 45000.0,
                min_order: 100.0,
                max_order: 50000.0,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }),
        latest_htx_rates: Some(P2PRatesResponse {
            exchange: "htx".to_string(),
            sell_rate: Some(P2PRateResponse {
                rate: 96.2,
                volume: 42000.0,
                min_order: 100.0,
                max_order: 50000.0,
            }),
            buy_rate: Some(P2PRateResponse {
                rate: 95.8,
                volume: 38000.0,
                min_order: 100.0,
                max_order: 50000.0,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }),
        active_opportunities: vec![
            P2POpportunityResponse {
                id: "p2p_bybit_htx_1".to_string(),
                sell_exchange: "bybit".to_string(),
                buy_exchange: "htx".to_string(),
                sell_rate: 97.8,
                buy_rate: 96.2,
                spread_percent: 1.66,
                volume_usdt: 42000.0,
                execution_time_min: 12,
                profit_rub_per_1000_usdt: 1600.0,
                created_at: chrono::Utc::now().to_rfc3339(),
                expires_at: (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339(),
            }
        ],
        daily_potential: P2PDailyPotentialResponse {
            opportunities_count: 1,
            avg_spread_percent: 1.66,
            cycles_per_day: 120, // 12 min per cycle = 120 cycles/day
            daily_return_percent: 6.64, // 1.66% * 4 cycles (conservative)
            daily_profit_rub: capital_rub * 0.0664,
            capital_rub,
        },
    };
    
    Ok(Json(response))
}

/// New Listings Strategy Endpoints
/// Expected: 15-40k RUB/day (3-8% daily)

#[derive(Debug, Serialize)]
pub struct NewListingsStatusResponse {
    pub gateio_connected: bool,
    pub bybit_connected: bool,
    pub active_opportunities: Vec<NewListingOpportunityResponse>,
    pub daily_potential: NewListingDailyPotentialResponse,
    pub recent_listings: Vec<RecentListingResponse>,
}

#[derive(Debug, Serialize)]
pub struct NewListingOpportunityResponse {
    pub id: String,
    pub symbol: String,
    pub listing_exchange: String,
    pub secondary_exchange: String,
    pub listing_price: f64,
    pub secondary_price: f64,
    pub spread_percent: f64,
    pub volume_available: f64,
    pub time_since_listing_min: u32,
    pub profit_usdt_per_1000: f64,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct NewListingDailyPotentialResponse {
    pub opportunities_count: usize,
    pub avg_spread_percent: f64,
    pub opportunities_per_day: f64,
    pub daily_return_percent: f64,
    pub daily_profit_usdt: f64,
    pub capital_usdt: f64,
}

#[derive(Debug, Serialize)]
pub struct RecentListingResponse {
    pub symbol: String,
    pub exchange: String,
    pub listing_time: String,
    pub initial_price: f64,
    pub current_price: f64,
    pub price_change_percent: f64,
}

/// Get new listings arbitrage status
pub async fn get_new_listings_status(
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<NewListingsStatusResponse>, StatusCode> {
    debug!("GET /api/strategies/new-listings");
    
    let capital_usdt = params.get("capital_usdt")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(50000.0); // Default 50k USDT equivalent
    
    // TODO: Get actual new listings data
    let response = NewListingsStatusResponse {
        gateio_connected: true,
        bybit_connected: true,
        active_opportunities: vec![
            NewListingOpportunityResponse {
                id: "listing_gate_bybit_1".to_string(),
                symbol: "NEWCOIN/USDT".to_string(),
                listing_exchange: "gateio".to_string(),
                secondary_exchange: "bybit".to_string(),
                listing_price: 0.50,
                secondary_price: 0.58,
                spread_percent: 16.0,
                volume_available: 25000.0,
                time_since_listing_min: 15,
                profit_usdt_per_1000: 160.0,
                created_at: chrono::Utc::now().to_rfc3339(),
                expires_at: (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339(),
            }
        ],
        daily_potential: NewListingDailyPotentialResponse {
            opportunities_count: 1,
            avg_spread_percent: 16.0,
            opportunities_per_day: 2.5,
            daily_return_percent: 5.0, // Conservative estimate
            daily_profit_usdt: capital_usdt * 0.05,
            capital_usdt,
        },
        recent_listings: vec![
            RecentListingResponse {
                symbol: "NEWCOIN/USDT".to_string(),
                exchange: "gateio".to_string(),
                listing_time: (chrono::Utc::now() - chrono::Duration::minutes(15)).to_rfc3339(),
                initial_price: 0.50,
                current_price: 0.58,
                price_change_percent: 16.0,
            }
        ],
    };
    
    Ok(Json(response))
}

/// Multi-Hop Strategy Endpoints
/// Expected: 5-15k RUB/day (1-3% daily)

#[derive(Debug, Serialize)]
pub struct MultiHopStatusResponse {
    pub gateio_connected: bool,
    pub active_opportunities: Vec<MultiHopOpportunityResponse>,
    pub daily_potential: MultiHopDailyPotentialResponse,
    pub orderbook_pairs: usize,
}

#[derive(Debug, Serialize)]
pub struct MultiHopOpportunityResponse {
    pub id: String,
    pub exchange: String,
    pub path_description: String,
    pub start_amount: f64,
    pub final_amount: f64,
    pub profit_percent: f64,
    pub execution_steps: Vec<MultiHopStepResponse>,
    pub estimated_execution_time_ms: u64,
    pub risk_score: f64,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct MultiHopStepResponse {
    pub step_number: u32,
    pub symbol: String,
    pub side: String,
    pub price: f64,
    pub quantity: f64,
    pub expected_result: f64,
}

#[derive(Debug, Serialize)]
pub struct MultiHopDailyPotentialResponse {
    pub opportunities_count: usize,
    pub avg_profit_percent: f64,
    pub cycles_per_day: u32,
    pub daily_return_percent: f64,
    pub daily_profit_usdt: f64,
    pub capital_usdt: f64,
}

/// Get multi-hop arbitrage status
pub async fn get_multi_hop_status(
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<MultiHopStatusResponse>, StatusCode> {
    debug!("GET /api/strategies/multi-hop");
    
    let capital_usdt = params.get("capital_usdt")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(50000.0);
    
    // TODO: Get actual multi-hop data from MultiHopEngine
    let response = MultiHopStatusResponse {
        gateio_connected: true,
        active_opportunities: vec![
            MultiHopOpportunityResponse {
                id: "multihop_gate_1".to_string(),
                exchange: "gateio".to_string(),
                path_description: "BTC→USDT→ETH→BTC".to_string(),
                start_amount: 1.0,
                final_amount: 1.005,
                profit_percent: 0.5,
                execution_steps: vec![
                    MultiHopStepResponse {
                        step_number: 1,
                        symbol: "BTC/USDT".to_string(),
                        side: "sell".to_string(),
                        price: 50000.0,
                        quantity: 1.0,
                        expected_result: 50000.0,
                    },
                    MultiHopStepResponse {
                        step_number: 2,
                        symbol: "ETH/USDT".to_string(),
                        side: "buy".to_string(),
                        price: 3000.0,
                        quantity: 16.67,
                        expected_result: 16.67,
                    },
                    MultiHopStepResponse {
                        step_number: 3,
                        symbol: "ETH/BTC".to_string(),
                        side: "sell".to_string(),
                        price: 0.0603,
                        quantity: 16.67,
                        expected_result: 1.005,
                    },
                ],
                estimated_execution_time_ms: 2000,
                risk_score: 0.3,
                created_at: chrono::Utc::now().to_rfc3339(),
                expires_at: (chrono::Utc::now() + chrono::Duration::seconds(30)).to_rfc3339(),
            }
        ],
        daily_potential: MultiHopDailyPotentialResponse {
            opportunities_count: 1,
            avg_profit_percent: 0.5,
            cycles_per_day: 100,
            daily_return_percent: 2.0, // Conservative estimate
            daily_profit_usdt: capital_usdt * 0.02,
            capital_usdt,
        },
        orderbook_pairs: 25,
    };
    
    Ok(Json(response))
}

/// Funding Rate Strategy Endpoints
/// Expected: 2-5k RUB/day (0.3-0.5% daily, passive)

#[derive(Debug, Serialize)]
pub struct FundingRateStatusResponse {
    pub bybit_connected: bool,
    pub okx_connected: bool,
    pub active_opportunities: Vec<FundingRateOpportunityResponse>,
    pub daily_potential: FundingRateDailyPotentialResponse,
    pub next_funding_times: Vec<NextFundingResponse>,
}

#[derive(Debug, Serialize)]
pub struct FundingRateOpportunityResponse {
    pub id: String,
    pub symbol: String,
    pub long_exchange: String,
    pub short_exchange: String,
    pub long_funding_rate: f64,
    pub short_funding_rate: f64,
    pub net_funding_rate: f64,
    pub daily_return_percent: f64,
    pub position_size: f64,
    pub next_funding_time: String,
    pub time_to_funding_hours: f64,
}

#[derive(Debug, Serialize)]
pub struct FundingRateDailyPotentialResponse {
    pub opportunities_count: usize,
    pub avg_daily_return_percent: f64,
    pub total_position_size: f64,
    pub total_daily_profit_usdt: f64,
    pub capital_utilization_percent: f64,
    pub capital_usdt: f64,
}

#[derive(Debug, Serialize)]
pub struct NextFundingResponse {
    pub exchange: String,
    pub symbol: String,
    pub funding_rate: f64,
    pub next_funding_time: String,
    pub hours_remaining: f64,
}

/// Get funding rate arbitrage status
pub async fn get_funding_rate_status(
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<FundingRateStatusResponse>, StatusCode> {
    debug!("GET /api/strategies/funding-rate");
    
    let capital_usdt = params.get("capital_usdt")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(50000.0);
    
    // TODO: Get actual funding rate data
    let next_funding = chrono::Utc::now() + chrono::Duration::hours(4);
    let response = FundingRateStatusResponse {
        bybit_connected: true,
        okx_connected: true,
        active_opportunities: vec![
            FundingRateOpportunityResponse {
                id: "funding_btc_bybit_okx".to_string(),
                symbol: "BTC-PERP".to_string(),
                long_exchange: "bybit".to_string(),
                short_exchange: "okx".to_string(),
                long_funding_rate: 0.08,
                short_funding_rate: -0.06,
                net_funding_rate: 0.14,
                daily_return_percent: 0.42, // 0.14% * 3 times per day
                position_size: 10000.0, // 20% of capital
                next_funding_time: next_funding.to_rfc3339(),
                time_to_funding_hours: 4.0,
            }
        ],
        daily_potential: FundingRateDailyPotentialResponse {
            opportunities_count: 1,
            avg_daily_return_percent: 0.42,
            total_position_size: 10000.0,
            total_daily_profit_usdt: 42.0,
            capital_utilization_percent: 20.0,
            capital_usdt,
        },
        next_funding_times: vec![
            NextFundingResponse {
                exchange: "bybit".to_string(),
                symbol: "BTC-PERP".to_string(),
                funding_rate: 0.08,
                next_funding_time: next_funding.to_rfc3339(),
                hours_remaining: 4.0,
            },
            NextFundingResponse {
                exchange: "okx".to_string(),
                symbol: "BTC-USDT-SWAP".to_string(),
                funding_rate: -0.06,
                next_funding_time: next_funding.to_rfc3339(),
                hours_remaining: 4.0,
            },
        ],
    };
    
    Ok(Json(response))
}

/// Combined Strategy Overview
/// Total expected: 8-22% daily returns

#[derive(Debug, Serialize)]
pub struct StrategySummaryResponse {
    pub total_daily_return_percent: f64,
    pub total_daily_profit_rub: f64,
    pub total_daily_profit_usdt: f64,
    pub capital_rub: f64,
    pub capital_usdt: f64,
    pub strategies: Vec<StrategyPerformanceResponse>,
    pub risk_assessment: RiskAssessmentResponse,
}

#[derive(Debug, Serialize)]
pub struct StrategyPerformanceResponse {
    pub name: String,
    pub daily_return_percent: f64,
    pub daily_profit_rub: f64,
    pub daily_profit_usdt: f64,
    pub opportunities_count: usize,
    pub status: String,
    pub priority: u32,
}

#[derive(Debug, Serialize)]
pub struct RiskAssessmentResponse {
    pub overall_risk_level: String,
    pub p2p_risk: String,
    pub listings_risk: String,
    pub multihop_risk: String,
    pub funding_risk: String,
    pub mitigation_strategies: Vec<String>,
}

/// Get combined strategy summary
pub async fn get_strategy_summary(
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<StrategySummaryResponse>, StatusCode> {
    debug!("GET /api/strategies/summary");
    
    let capital_rub = params.get("capital_rub")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(500000.0);
    
    let capital_usdt = capital_rub / 97.0; // Approximate conversion
    
    // Russian Financial Advisor's projections
    let strategies = vec![
        StrategyPerformanceResponse {
            name: "P2P RUB Arbitrage".to_string(),
            daily_return_percent: 7.0, // 4-10% range, use middle
            daily_profit_rub: capital_rub * 0.07,
            daily_profit_usdt: (capital_rub * 0.07) / 97.0,
            opportunities_count: 75, // 50-100 per hour
            status: "active".to_string(),
            priority: 1,
        },
        StrategyPerformanceResponse {
            name: "New Listings Arbitrage".to_string(),
            daily_return_percent: 5.5, // 3-8% range
            daily_profit_rub: capital_rub * 0.055,
            daily_profit_usdt: (capital_rub * 0.055) / 97.0,
            opportunities_count: 3, // 2-3 per day
            status: "active".to_string(),
            priority: 2,
        },
        StrategyPerformanceResponse {
            name: "Multi-Hop Triangular".to_string(),
            daily_return_percent: 2.0, // 1-3% range
            daily_profit_rub: capital_rub * 0.02,
            daily_profit_usdt: (capital_rub * 0.02) / 97.0,
            opportunities_count: 100, // Continuous
            status: "active".to_string(),
            priority: 3,
        },
        StrategyPerformanceResponse {
            name: "Funding Rate Arbitrage".to_string(),
            daily_return_percent: 0.4, // 0.3-0.5% range
            daily_profit_rub: capital_rub * 0.004,
            daily_profit_usdt: (capital_rub * 0.004) / 97.0,
            opportunities_count: 3, // 3 cycles per day
            status: "passive".to_string(),
            priority: 4,
        },
    ];
    
    let total_daily_return = strategies.iter().map(|s| s.daily_return_percent).sum::<f64>();
    let total_daily_profit_rub = strategies.iter().map(|s| s.daily_profit_rub).sum::<f64>();
    let total_daily_profit_usdt = strategies.iter().map(|s| s.daily_profit_usdt).sum::<f64>();
    
    let response = StrategySummaryResponse {
        total_daily_return_percent: total_daily_return,
        total_daily_profit_rub,
        total_daily_profit_usdt,
        capital_rub,
        capital_usdt,
        strategies,
        risk_assessment: RiskAssessmentResponse {
            overall_risk_level: "Medium".to_string(),
            p2p_risk: "Bank restrictions".to_string(),
            listings_risk: "Execution slippage".to_string(),
            multihop_risk: "Path convergence".to_string(),
            funding_risk: "Basis risk".to_string(),
            mitigation_strategies: vec![
                "3-5 bank cards rotation".to_string(),
                "Volume confirmation before execution".to_string(),
                "Real-time orderbook depth monitoring".to_string(),
                "Position sizing <20% capital".to_string(),
            ],
        },
    };
    
    Ok(Json(response))
}