pub mod connector;
pub mod utils;
pub mod websocket_pool; // THE BIG BOSS WebSocket connection pool

// WebSocket connectors for the 5 WORKING exchanges
pub mod bybit_ws;
pub mod okx_ws;
pub mod gateio_ws;
pub mod htx_ws;
pub mod hyperliquid_ws;

// P2P RUB arbitrage connectors (HIGH PRIORITY - 4-10% daily returns)
pub mod p2p_simple;

pub use connector::{ExchangeConnector, ConnectorError, ConnectionEvent};
pub use websocket_pool::{WebSocketPool, ConnectionStats};

// Export WebSocket connectors
pub use bybit_ws::ByBitWebSocketConnector;
pub use okx_ws::OKXWebSocketConnector;
pub use gateio_ws::GateIoWebSocketConnector;
pub use htx_ws::HTXWebSocketConnector;
pub use hyperliquid_ws::HyperliquidWebSocketConnector;

// Export P2P REST connectors
pub use p2p_simple::P2PManager;