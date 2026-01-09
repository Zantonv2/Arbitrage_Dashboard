//! # Arbitrage Server
//!
//! Main HTTP server for the arbitrage dashboard.
//! Provides REST API endpoints and WebSocket connections for real-time data.

use crate::{bridge::ArbitrageBridge, config_manager::ConfigManager, routes, websocket};
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
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{error, info};

/// Shared application state accessible by all route handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub arbitrage_engine: Arc<ArbitrageEngine>,
    pub storage: Arc<StorageService>,
    pub strategy_registry: Arc<StrategyRegistry>,
    pub bridge: Arc<Mutex<ArbitrageBridge>>,
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

        // Initialize bridge service
        let bridge = Arc::new(Mutex::new(
            ArbitrageBridge::new(
                (*config).clone(),
                arbitrage_engine.clone(),
                strategy_registry.clone(),
            )
            .await?,
        ));

        // Create application state
        let state = AppState {
            config: config.clone(),
            arbitrage_engine,
            storage,
            strategy_registry,
            bridge: bridge.clone(),
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
        let storage = Arc::new(StorageService::new_async(storage_config).await?);

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
        // API routes
        let api_routes = Router::new()
            // Signals
            .route("/signals", get(routes::get_signals))
            .route("/signals/:id", get(routes::get_signal))
            // Order books
            .route("/orderbooks/:exchange/:symbol", get(routes::get_orderbook))
            // Execution
            .route("/executions/prepare", post(routes::prepare_execution))
            .route("/executions/confirm", post(routes::confirm_execution))
            // Analytics & Status
            .route("/analytics", get(routes::get_analytics))
            .route("/status/exchanges", get(routes::get_exchange_status))
            .route("/status/bridge", get(routes::get_bridge_status))
            // Configuration
            .route("/config", get(routes::get_config))
            .route("/config", post(routes::update_config))
            .with_state(state.clone());

        // WebSocket route
        let ws_routes = Router::new()
            .route("/ws", get(websocket::websocket_handler))
            .with_state(state.clone());

        // Static file serving for frontend
        let static_files = ServeDir::new(&config.server.static_files_path)
            .not_found_service(
                ServeDir::new(&config.server.static_files_path)
                    .append_index_html_on_directories(true),
            );

        // Combine all routes
        let app = Router::new()
            .nest("/api", api_routes)
            .merge(ws_routes)
            .fallback_service(static_files)
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(
                        CorsLayer::new()
                            .allow_origin(Any)
                            .allow_methods(Any)
                            .allow_headers(Any),
                    ),
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
