use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::{broadcast, RwLock};

use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol},
    ArbitrageError, Result,
};

use crate::connector::{
    Balance, CancelResponse, ConnectorConfig, ConnectorStats, FundingRate, HealthStatus,
    OrderRequest, OrderResponse, OrderSide, OrderStatus, OrderStatusType, OrderType, TickerData,
    TimeInForce,
};
use crate::events::ConnectionEvent;

#[derive(Clone)]
pub struct ConnectorBase {
    pub config: ConnectorConfig,
    pub status: Arc<RwLock<ConnectionStatus>>,
    pub stats: Arc<Mutex<ConnectorStats>>,
    pub event_sender: broadcast::Sender<ConnectionEvent>,
}

impl ConnectorBase {
    pub fn new(config: ConnectorConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        let mut stats = ConnectorStats::default();
        stats.exchange = config.exchange_id;

        Self {
            config,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
            event_sender,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        {
            let status = self.status.read().await;
            if *status == ConnectionStatus::Connected {
                return Ok(());
            }
        }

        *self.status.write().await = ConnectionStatus::Connecting;

        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        *self.status.write().await = ConnectionStatus::Disconnected;
        Ok(())
    }

    pub fn get_stats(&self) -> ConnectorStats {
        match self.stats.try_lock() {
            Ok(stats) => stats.clone(),
            Err(_) => ConnectorStats::default(),
        }
    }

    pub fn stats(&self) -> MutexGuard<'_, ConnectorStats> {
        self.stats.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    fn id(&self) -> ExchangeId;
    fn config(&self) -> &ConnectorConfig;
    fn base(&self) -> &ConnectorBase;
    fn base_mut(&mut self) -> &mut ConnectorBase;

    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent>;

    async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook>;
    async fn fetch_symbols(&self) -> Result<Vec<Symbol>>;
    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>>;
    async fn fetch_funding_rates(&self, symbols: &[Symbol])
        -> Result<HashMap<Symbol, FundingRate>>;

    async fn connect(&mut self) -> Result<()> {
        self.base_mut()
            .connect()
            .await
            .map_err(|e| ArbitrageError::Exchange(format!("connect failed: {}", e)))
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.base_mut().disconnect().await
    }

    async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn subscribe_funding_rates(&mut self, symbols: &[Symbol]) -> Result<()>;

    async fn health_check(&self) -> Result<HealthStatus> {
        let status = self.base().status.read().await.clone();
        let stats = self.base().stats();
        Ok(HealthStatus {
            is_connected: status == ConnectionStatus::Connected,
            last_message_time: Some(stats.last_update),
            websocket_status: status,
            rest_api_status: ConnectionStatus::Connected,
            error_count: stats.errors_count,
            reconnect_count: stats.reconnections,
        })
    }

    fn status(&self) -> ConnectionStatus {
        match self.base().status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ConnectionStatus::Disconnected,
        }
    }

    fn get_stats(&self) -> ConnectorStats {
        self.base().get_stats()
    }

    async fn force_reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    async fn place_order(&self, _request: &OrderRequest) -> Result<OrderResponse> {
        Err(ArbitrageError::NotImplemented(
            "place_order not implemented".to_string(),
        ))
    }
    async fn cancel_order(&self, _order_id: &str, _symbol: &Symbol) -> Result<()> {
        Err(ArbitrageError::NotImplemented(
            "cancel_order not implemented".to_string(),
        ))
    }
    async fn get_order_status(&self, _order_id: &str, _symbol: &Symbol) -> Result<OrderStatus> {
        Err(ArbitrageError::NotImplemented(
            "get_order_status not implemented".to_string(),
        ))
    }
    async fn get_balance(&self) -> Result<Balance> {
        Err(ArbitrageError::NotImplemented(
            "get_balance not implemented".to_string(),
        ))
    }
    async fn get_open_orders(&self, _symbol: Option<&Symbol>) -> Result<Vec<OrderStatus>> {
        Err(ArbitrageError::NotImplemented(
            "get_open_orders not implemented".to_string(),
        ))
    }

    fn parse_order_book(
        &self,
        data: &Value,
        symbol: &Symbol,
        bids_path: &[&str],
        asks_path: &[&str],
    ) -> Result<OrderBook> {
        let mut bids = Vec::new();
        let mut asks = Vec::new();

        let depth = self.config().order_book_depth as usize;

        for path_segment in bids_path {
            if let Some(obj) = data.pointer(path_segment) {
                if let Some(bids_array) = obj.as_array() {
                    for item in bids_array.iter().take(depth) {
                        if let Some(level) = self.parse_order_book_level(item) {
                            bids.push(level);
                        }
                    }
                }
                break;
            }
        }

        for path_segment in asks_path {
            if let Some(obj) = data.pointer(path_segment) {
                if let Some(asks_array) = obj.as_array() {
                    for item in asks_array.iter().take(depth) {
                        if let Some(level) = self.parse_order_book_level(item) {
                            asks.push(level);
                        }
                    }
                }
                break;
            }
        }

        Ok(OrderBook::new(self.id(), symbol.clone(), bids, asks))
    }

    fn parse_order_book_level(&self, data: &Value) -> Option<OrderBookLevel> {
        let price = data
            .get("price")
            .or(data.get(0))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())?;

        let quantity = data
            .get("size")
            .or(data.get("quantity"))
            .or(data.get(1))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())?;

        Some(OrderBookLevel { price, quantity })
    }

    fn parse_order_book_from_array(
        &self,
        data: &Value,
        symbol: &Symbol,
        bids_key: &str,
        asks_key: &str,
    ) -> Result<OrderBook> {
        let empty_vec: Vec<Value> = Vec::new();
        let bids_array = data
            .get(bids_key)
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);
        let asks_array = data
            .get(asks_key)
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);

        let depth = self.config().order_book_depth as usize;
        let mut bids = Vec::with_capacity(depth);
        let mut asks = Vec::with_capacity(depth);

        for item in bids_array.iter().take(depth) {
            if let Some(level) = self.parse_order_book_level(item) {
                bids.push(level);
            }
        }

        for item in asks_array.iter().take(depth) {
            if let Some(level) = self.parse_order_book_level(item) {
                asks.push(level);
            }
        }

        Ok(OrderBook::new(self.id(), symbol.clone(), bids, asks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn create_test_connector_base() -> ConnectorBase {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::OKX,
            order_book_depth: 10,
            ..Default::default()
        };
        ConnectorBase::new(config)
    }

    fn create_test_value() -> Value {
        serde_json::json!({
            "bids": [
                {"price": "50000.00", "size": "1.5"},
                {"price": "49999.50", "size": "2.0"},
                {"price": "49999.00", "size": "0.5"}
            ],
            "asks": [
                {"price": "50001.00", "size": "1.0"},
                {"price": "50001.50", "size": "2.5"},
                {"price": "50002.00", "size": "1.0"}
            ]
        })
    }

    struct TestConnector {
        base: ConnectorBase,
    }

    #[async_trait]
    impl ExchangeConnector for TestConnector {
        fn id(&self) -> ExchangeId {
            ExchangeId::OKX
        }

        fn config(&self) -> &ConnectorConfig {
            &self.base.config
        }

        fn base(&self) -> &ConnectorBase {
            &self.base
        }

        fn base_mut(&mut self) -> &mut ConnectorBase {
            &mut self.base
        }

        fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
            self.base.event_sender.subscribe()
        }

        async fn fetch_order_book(&self, _symbol: &Symbol) -> Result<OrderBook> {
            unimplemented!()
        }

        async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
            unimplemented!()
        }

        async fn fetch_tickers(&self, _symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
            unimplemented!()
        }

        async fn fetch_funding_rates(
            &self,
            _symbols: &[Symbol],
        ) -> Result<HashMap<Symbol, FundingRate>> {
            unimplemented!()
        }

        async fn subscribe_symbols(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn unsubscribe_symbols(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_tickers(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_order_books(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_trades(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }

        async fn subscribe_funding_rates(&mut self, _symbols: &[Symbol]) -> Result<()> {
            Ok(())
        }
    }

    impl TestConnector {
        fn new() -> Self {
            Self {
                base: create_test_connector_base(),
            }
        }
    }

    #[tokio::test]
    async fn test_parse_order_book() {
        let connector = TestConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_test_value();

        let order_book = connector
            .parse_order_book_from_array(&data, &symbol, "bids", "asks")
            .unwrap();

        assert_eq!(order_book.exchange, ExchangeId::OKX);
        assert_eq!(order_book.symbol.base, "BTC");
        assert_eq!(order_book.symbol.quote, "USDT");
        assert_eq!(order_book.bids.len(), 3);
        assert_eq!(order_book.asks.len(), 3);

        assert_eq!(order_book.bids[0].price, Decimal::new(50000, 2));
        assert_eq!(order_book.bids[0].quantity, Decimal::new(15, 1));
    }

    #[tokio::test]
    async fn test_parse_order_book_with_depth_limit() {
        let mut base = create_test_connector_base();
        base.config.order_book_depth = 2;
        let connector = TestConnector { base };
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_test_value();

        let order_book = connector
            .parse_order_book_from_array(&data, &symbol, "bids", "asks")
            .unwrap();

        assert_eq!(order_book.bids.len(), 2);
        assert_eq!(order_book.asks.len(), 2);
    }

    #[tokio::test]
    async fn test_status() {
        let connector = TestConnector::new();
        let status = connector.status();
        assert_eq!(status, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let connector = TestConnector::new();
        let stats = connector.get_stats();
        assert_eq!(stats.exchange, ExchangeId::OKX);
    }
}
