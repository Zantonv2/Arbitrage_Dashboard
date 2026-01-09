use crate::{routes, websocket, config_manager::ConfigManager, bridge::ArbitrageBridge};
use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    strategies::{StrategyRegistry, CexArbitrageStrategy},
    config::Config,
    normalizer::Normalizer,
    confidence_scorer::ConfidenceScorer,
    size_calculator::SizeCalculator,
    execution_preparer::ExecutionPreparer,
    storage::StorageService,
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
    pub strategy_registry: Arc<StrategyRegistry>,
    pub bridge: Arc<Mutex<ArbitrageBridge>>,
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

        // Initialize storage with proper config
        let storage_config = arbitrage_core::storage::StorageConfig {
            database_path: config.storage.database_url.clone(),
            max_signal_history: 10000,
            max_execution_history: 5000,
            enable_compression: false,
        };
        let storage = Arc::new(StorageService::new(storage_config)?);

        // Initialize components with proper configs
        let normalizer = Arc::new(Normalizer::new());
        
        let confidence_config = arbitrage_core::confidence_scorer::ConfidenceConfig::default();
        let confidence_scorer = Arc::new(ConfidenceScorer::new(confidence_config));
        
        let (arbitrage_engine, signal_receiver) = ArbitrageEngine::new(
            (*config).clone(),
            normalizer.clone(),
            confidence_scorer.clone(),
            storage.clone(),
        )?;
        let arbitrage_engine = Arc::new(arbitrage_engine);
        
        let size_config = arbitrage_core::size_calculator::SizeConfig::default();
        let size_calculator = Arc::new(SizeCalculator::new(size_config));
        
        let execution_config = arbitrage_core::execution_preparer::ExecutionConfig::default();
        let execution_preparer = Arc::new(ExecutionPreparer::new(execution_config));

        // Initialize strategy registry with CEX arbitrage
        let mut strategy_registry = StrategyRegistry::new();
        let cex_strategy = Arc::new(CexArbitrageStrategy::new());
        strategy_registry.register(cex_strategy)?;
        let strategy_registry = Arc::new(strategy_registry);

        // Initialize bridge service
        let bridge = Arc::new(Mutex::new(ArbitrageBridge::new(
            (*config).clone(),
            arbitrage_engine.clone(),
            strategy_registry.clone(),
        ).await?));

        // Create application state
        let state = AppState {
            config: config.clone(),
            normalizer,
            arbitrage_engine,
            confidence_scorer,
            size_calculator,
            execution_preparer,
            strategy_registry,
            bridge: bridge.clone(),
        };

        // Start the bridge service in background
        let bridge_for_spawn = Arc::clone(&bridge);
        tokio::spawn(async move {
            if let Err(e) = bridge_for_spawn.lock().await.start().await {
                error!("Bridge service failed to start: {}", e);
            }
        });

        // Start signal broadcasting to WebSocket clients
        Self::start_signal_broadcaster(signal_receiver).await;

        // Build router
        let app = Self::build_router(state).await?;

        // Create TCP listener
        let addr = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&addr).await?;
        info!("🚀 Arbitrage Dashboard Server listening on {}", addr);
        info!("📊 Real-time arbitrage detection enabled");
        info!("🌐 Web dashboard available at http://{}", addr);

        Ok(Self { app, listener })
    }

    /// Start signal broadcasting to WebSocket clients
    async fn start_signal_broadcaster(mut signal_receiver: tokio::sync::broadcast::Receiver<arbitrage_core::types::Signal>) {
        tokio::spawn(async move {
            info!("📡 Signal broadcaster started");
            
            while let Ok(signal) = signal_receiver.recv().await {
                info!("🚨 Broadcasting arbitrage signal: {} profit on {}", 
                      signal.net_profit_percent * rust_decimal::Decimal::from(100), 
                      signal.symbol);
                
                // TODO: Broadcast to WebSocket clients
                // This will be handled by the websocket module
            }
        });
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
            .route("/status/bridge", get(routes::get_bridge_status))
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