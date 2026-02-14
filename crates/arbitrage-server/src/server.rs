//! # Arbitrage Server
//!
//! Main HTTP server for the arbitrage dashboard.
//! Provides REST API endpoints and WebSocket connections for real-time data.
//!
//! **Note:** Authentication has been removed for local-only use.
//! The server is intended for local access only.

use crate::{
    audit_logger::AuditLogger, bridge::ArbitrageBridge, config_manager::ConfigManager,
    instruction_cache::InstructionCache, order_executor::ExecutorConfig, routes, websocket,
};
use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
    config::Config,
    execution_preparer::{ExecutionConfig, ExecutionPreparer},
    normalizer::Normalizer,
    size_calculator::{SizeCalculator, SizeConfig},
    storage::{StorageConfig, StorageService},
    strategies::{
        CexArbitrageStrategy, ConvergenceArbitrageStrategy, CrossExchangeArbitrageStrategy,
        FundingRateArbitrageStrategy, HedgedFundingStrategy, LatencyArbitrageStrategy,
        NewListingArbitrageStrategy, SpotPerpArbitrageStrategy, SpreadCaptureStrategy,
        StablecoinArbitrageStrategy, StrategyRegistry,
    },
};
use axum::{
    body::Body,
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use http::{Request, StatusCode};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{error, info, warn};

const RATE_LIMIT_MAX_REQUESTS: u64 = 100;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

fn sanitize_path(path: &str) -> Result<std::path::PathBuf, String> {
    if path.contains("..") {
        return Err("Path traversal sequences not allowed".to_string());
    }

    std::fs::canonicalize(path).map_err(|e| format!("Failed to canonicalize path: {}", e))
}

/// Shared application state accessible by all route handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub arbitrage_engine: Arc<ArbitrageEngine>,
    pub storage: Arc<StorageService>,
    pub strategy_registry: Arc<StrategyRegistry>,
    pub bridge: Arc<Mutex<ArbitrageBridge>>,
    pub audit_logger: Arc<AuditLogger>,
    pub executor_config: ExecutorConfig,
    pub instruction_cache: Arc<InstructionCache>,
}

#[derive(Clone)]
pub struct RateLimitState {
    requests: Arc<std::sync::Mutex<HashMap<String, (Instant, u64)>>>,
}

impl RateLimitState {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn check_rate_limit(&self, key: &str, max_requests: u64, window_secs: u64) -> bool {
        let now = Instant::now();
        let mut requests = match self.requests.lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::error!("Mutex poisoned for rate limiting - allowing request");
                return true;
            }
        };

        let should_allow = match requests.get(key) {
            Some((first_request, count)) => {
                let elapsed = now.duration_since(*first_request);
                if elapsed < Duration::from_secs(window_secs) {
                    *count < max_requests
                } else {
                    true
                }
            }
            None => true,
        };

        if should_allow {
            requests.insert(key.to_string(), (now, 1));
        }
        should_allow
    }
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

fn get_client_ip(req: &Request<Body>) -> String {
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v: &http::HeaderValue| v.to_str().ok())
        .or(req
            .headers()
            .get("x-real-ip")
            .and_then(|v: &http::HeaderValue| v.to_str().ok()))
        .unwrap_or("unknown")
        .to_string()
}

fn create_error_response(status_code: StatusCode, message: &str) -> impl IntoResponse {
    // Use safe response building without .expect()
    let body = Body::from(message.to_string());
    match http::Response::builder().status(status_code).body(body) {
        Ok(response) => response,
        Err(e) => {
            // Log the error and return a minimal safe response
            error!(
                "Failed to create HTTP response: {}. Returning minimal error response.",
                e
            );
            http::Response::new(Body::from("Internal Server Error"))
        }
    }
}

/// Main server struct
pub struct ArbitrageServer {
    app: Router,
    listener: TcpListener,
}

impl ArbitrageServer {
    /// Create and initialize the server with all components
    pub async fn new(
        config_path: &str,
        host: &str,
        port: u16,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing Arbitrage Server...");

        // Load configuration
        let config_manager = ConfigManager::new(config_path)?;
        let config = Arc::new(config_manager.get_config().clone());

        // Initialize core components
        let (storage, normalizer, confidence_scorer, size_calculator, execution_preparer) =
            Self::init_core_components(&config).await?;

        // Create arbitrage engine
        let (arbitrage_engine, signal_receiver) = ArbitrageEngine::new(
            (*config).clone(),
            normalizer,
            confidence_scorer,
            size_calculator,
            execution_preparer,
            storage.clone(),
        )?;
        let arbitrage_engine = Arc::new(arbitrage_engine);

        // Initialize strategy registry with all 10 strategies
        let strategy_registry = Arc::new(Self::init_strategies()?);
        info!(
            "✅ Registered {} strategies",
            strategy_registry.get_enabled().len()
        );

        // Initialize audit logger
        let audit_logger = Arc::new(AuditLogger::new(10000));
        info!("📝 Audit logger initialized");

        // Initialize bridge service
        let bridge = Arc::new(Mutex::new(
            ArbitrageBridge::new(
                (*config).clone(),
                arbitrage_engine.clone(),
                strategy_registry.clone(),
                audit_logger.clone(),
            )
            .await?,
        ));

        // Initialize order executor
        let executor_config = ExecutorConfig {
            execution_timeout_ms: 5000,
            max_slippage_percent: config.risk.max_slippage_percent,
            enable_rollback: true,
            min_profit_threshold: config.trading.min_profit_threshold_percent,
            max_position_size: config.risk.max_position_size_usd,
        };
        // Note: OrderExecutor needs ExchangeManager which is inside bridge
        // For now, we'll create it when needed in routes
        info!("⚙️ Order executor config initialized");

        // Initialize instruction cache
        let instruction_cache = Arc::new(InstructionCache::default());
        info!("📦 Instruction cache initialized");

        // Create application state
        let state = AppState {
            config: config.clone(),
            arbitrage_engine,
            storage,
            strategy_registry,
            bridge: bridge.clone(),
            audit_logger,
            executor_config,
            instruction_cache,
        };

        // Start background services
        Self::start_background_services(bridge, signal_receiver).await;

        // Build router
        let app = Self::build_router(state, &config).await?;

        // Create TCP listener
        let addr = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&addr).await?;

        info!("🚀 Server listening on http://{}", addr);

        Ok(Self { app, listener })
    }

    /// Initialize core components (storage, normalizer, scorers, etc.)
    async fn init_core_components(
        config: &Config,
    ) -> Result<
        (
            Arc<StorageService>,
            Arc<Normalizer>,
            Arc<ConfidenceScorer>,
            Arc<SizeCalculator>,
            Arc<ExecutionPreparer>,
        ),
        Box<dyn std::error::Error>,
    > {
        let storage_config = StorageConfig {
            database_path: config.storage.database_url.clone(),
            max_signal_history: 10000,
            max_execution_history: 5000,
            enable_compression: false,
        };
        let storage = Arc::new(StorageService::new(storage_config).await?);

        let normalizer = Arc::new(Normalizer::new());
        let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
        let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
        let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));

        Ok((
            storage,
            normalizer,
            confidence_scorer,
            size_calculator,
            execution_preparer,
        ))
    }

    /// Initialize all 10 arbitrage strategies
    fn init_strategies() -> Result<StrategyRegistry, Box<dyn std::error::Error>> {
        let mut registry = StrategyRegistry::new();

        // Tier 1: Highest ROI strategies
        registry.register(Arc::new(CexArbitrageStrategy::new()))?;
        registry.register(Arc::new(FundingRateArbitrageStrategy::new()))?;
        registry.register(Arc::new(StablecoinArbitrageStrategy::new()))?;

        // Tier 2: Medium complexity
        registry.register(Arc::new(SpotPerpArbitrageStrategy::new()))?;
        registry.register(Arc::new(CrossExchangeArbitrageStrategy::new()))?;
        registry.register(Arc::new(NewListingArbitrageStrategy::new()))?;

        // Tier 3: Advanced strategies
        registry.register(Arc::new(LatencyArbitrageStrategy::new()))?;
        registry.register(Arc::new(SpreadCaptureStrategy::new()))?;
        registry.register(Arc::new(ConvergenceArbitrageStrategy::new()))?;
        registry.register(Arc::new(HedgedFundingStrategy::new()))?;

        Ok(registry)
    }

    /// Start background services (bridge, signal broadcaster)
    async fn start_background_services(
        bridge: Arc<Mutex<ArbitrageBridge>>,
        mut signal_receiver: tokio::sync::broadcast::Receiver<arbitrage_core::types::Signal>,
    ) {
        // Start bridge service
        let bridge_clone = Arc::clone(&bridge);
        tokio::spawn(async move {
            if let Err(e) = bridge_clone.lock().await.start().await {
                error!("Bridge service failed: {}", e);
            }
        });

        // Start signal broadcaster
        tokio::spawn(async move {
            info!("📡 Signal broadcaster started");
            while let Ok(signal) = signal_receiver.recv().await {
                info!(
                    "🎯 Signal: {} {:.4}% profit ({} → {})",
                    signal.symbol,
                    signal.net_profit_percent * rust_decimal::Decimal::from(100),
                    signal.buy_exchange,
                    signal.sell_exchange
                );
            }
        });
    }

    /// Build the HTTP router with all routes
    async fn build_router(
        state: AppState,
        config: &Config,
    ) -> Result<Router, Box<dyn std::error::Error>> {
        let rate_limit_state = Arc::new(RateLimitState::new());

        let allowed_origins: Vec<String> = if config.server.cors_origins.is_empty() {
            vec!["http://localhost:5173".to_string()]
        } else {
            config.server.cors_origins.clone()
        };

        let allowed_origins_values: Vec<http::HeaderValue> = allowed_origins
            .iter()
            .map(|s| {
                http::HeaderValue::from_str(s)
                    .unwrap_or_else(|_| http::HeaderValue::from_static("*"))
            })
            .collect();

        let cors = CorsLayer::new()
            .allow_origin(AllowOrigin::list(allowed_origins_values))
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any);

        let rate_limit_state_for_api = rate_limit_state.clone();
        let rate_limit_state_for_public = rate_limit_state.clone();

        // API routes (all public for local-only use)
        let api_routes = Router::new()
            // Signals
            .route("/signals", get(routes::get_signals))
            .route("/signals/{id}", get(routes::get_signal))
            // Order books
            .route(
                "/orderbooks/{exchange}/{symbol}",
                get(routes::get_orderbook),
            )
            // Execution
            .route("/executions/prepare", post(routes::prepare_execution))
            .route("/executions/confirm", post(routes::confirm_execution))
            // Analytics & Status
            .route("/analytics", get(routes::get_analytics))
            .route("/audit", get(routes::get_audit_log))
            .route("/status/exchanges", get(routes::get_exchange_status))
            .route("/status/bridge", get(routes::get_bridge_status))
            // Configuration
            .route("/config", get(routes::get_config))
            .route("/config", post(routes::update_config))
            // Credentials
            .route("/credentials", get(routes::get_credentials))
            .route("/credentials", post(routes::save_credentials))
            .route("/credentials/validate", post(routes::validate_credentials))
            .route("/credentials/{exchange}", delete(routes::delete_credentials))
            // Exchange mode toggle
            .route("/exchange/mode", post(routes::toggle_exchange_mode))
            .with_state(state.clone())
            .layer(axum::middleware::from_fn(
                move |req: Request<Body>, next: axum::middleware::Next| {
                    let rate_limit_state = rate_limit_state_for_api.clone();
                    async move {
                        // Rate limiting only (no auth for local-only use)
                        let client_ip = get_client_ip(&req);
                        if !rate_limit_state.check_rate_limit(
                            &client_ip,
                            RATE_LIMIT_MAX_REQUESTS,
                            RATE_LIMIT_WINDOW_SECS,
                        ) {
                            warn!("Rate limit exceeded for IP: {}", client_ip);
                            return Err(create_error_response(
                                StatusCode::TOO_MANY_REQUESTS,
                                "Rate limit exceeded",
                            ));
                        }

                        Ok(next.run(req).await)
                    }
                },
            ));

        // WebSocket route
        let ws_routes = Router::new()
            .route("/ws", get(websocket::websocket_handler))
            .with_state(state.clone());

        // Public routes (health check)
        let public_routes = Router::new()
            .route("/health", get(|| async { "OK" }))
            .with_state(state.clone())
            .layer(axum::middleware::from_fn(
                move |req: Request<Body>, next: axum::middleware::Next| {
                    let rate_limit_state = rate_limit_state_for_public.clone();
                    async move {
                        let client_ip = get_client_ip(&req);
                        if !rate_limit_state.check_rate_limit(
                            &client_ip,
                            RATE_LIMIT_MAX_REQUESTS,
                            RATE_LIMIT_WINDOW_SECS,
                        ) {
                            warn!("Rate limit exceeded for IP: {}", client_ip);
                            return Err(create_error_response(
                                StatusCode::TOO_MANY_REQUESTS,
                                "Rate limit exceeded",
                            ));
                        }
                        Ok(next.run(req).await)
                    }
                },
            ));

        // Static file serving for frontend
        let sanitized_static_path =
            sanitize_path(&config.server.static_files_path).map_err(|e| {
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
                    as Box<dyn std::error::Error>
            })?;
        let static_files = ServeDir::new(&sanitized_static_path).not_found_service(
            ServeDir::new(&sanitized_static_path).append_index_html_on_directories(true),
        );

        // Combine all routes
        let app = Router::new()
            .nest("/api", api_routes)
            .merge(ws_routes)
            .merge(public_routes)
            .fallback_service(static_files)
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(cors),
            );

        Ok(app)
    }

    /// Run the server (blocking)
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting HTTP server...");
        axum::serve(self.listener, self.app).await.map_err(|e| {
            error!("Server error: {}", e);
            e.into()
        })
    }
}
