use crate::{routes, websocket, config_manager::ConfigManager};
use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    config::Config,
    normalizer::Normalizer,
    confidence_scorer::ConfidenceScorer,
    size_calculator::SizeCalculator,
    execution_preparer::ExecutionPreparer,
};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{info, error};

/// Main application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub normalizer: Arc<Normalizer>,
    pub arbitrage_engine: Arc<ArbitrageEngine>,
    pub confidence_scorer: Arc<ConfidenceScorer>,
    pub size_calculator: Arc<SizeCalculator>,
    pub execution_preparer: Arc<ExecutionPreparer>,
}

/// Main server struct
pub struct ArbitrageServer {
    app: Router,
    listener: TcpListener,
}

impl ArbitrageServer {
    /// Create new server instance
    pub async fn new(
        config_path: &str,
        host: &str,
        port: u16,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Load configuration
        let config_manager = ConfigManager::new(config_path)?;
        let config = Arc::new(config_manager.get_config().clone());

        // Initialize components
        let normalizer = Arc::new(Normalizer::new());
        let confidence_scorer = Arc::new(ConfidenceScorer::default());
        
        let (arbitrage_engine, _signal_receiver) = ArbitrageEngine::new(
            normalizer.clone(),
            confidence_scorer.clone(),
            config.trading.min_profit_threshold_percent,
            config.trading.stale_orderbook_threshold_ms,
            config.trading.signal_deduplication_window_ms,
        );
        let arbitrage_engine = Arc::new(arbitrage_engine);
        let size_calculator = Arc::new(SizeCalculator::default());
        let execution_preparer = Arc::new(ExecutionPreparer::default());

        // Create application state
        let state = AppState {
            config: config.clone(),
            normalizer,
            arbitrage_engine,
            confidence_scorer,
            size_calculator,
            execution_preparer,
        };

        // Build router
        let app = Self::build_router(state).await?;

        // Create TCP listener
        let addr = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&addr).await?;
        info!("Server listening on {}", addr);

        Ok(Self { app, listener })
    }

    /// Build the application router
    async fn build_router(state: AppState) -> Result<Router, Box<dyn std::error::Error>> {
        // API routes
        let api_routes = Router::new()
            .route("/signals", get(routes::get_signals))
            .route("/signals/:id", get(routes::get_signal))
            .route("/orderbooks/:exchange/:symbol", get(routes::get_orderbook))
            .route("/executions/prepare", post(routes::prepare_execution))
            .route("/executions/confirm", post(routes::confirm_execution))
            .route("/analytics", get(routes::get_analytics))
            .route("/status/exchanges", get(routes::get_exchange_status))
            .route("/config", get(routes::get_config))
            .route("/config", post(routes::update_config))
            .with_state(state.clone());

        // WebSocket route
        let ws_routes = Router::new()
            .route("/ws", get(websocket::websocket_handler))
            .with_state(state.clone());

        // Static file serving
        let static_files = ServeDir::new(&state.config.server.static_files_path)
            .not_found_service(ServeDir::new(&state.config.server.static_files_path).append_index_html_on_directories(true));

        // Build main router
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

    /// Run the server
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting HTTP server...");
        
        axum::serve(self.listener, self.app)
            .await
            .map_err(|e| {
                error!("Server error: {}", e);
                e.into()
            })
    }
}