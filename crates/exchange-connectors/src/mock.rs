use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol};
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::connector::{
    Balance, CancelResponse, ConnectorConfig, ConnectorStats, FundingRate, HealthStatus,
    OrderRequest, OrderResponse, OrderSide, OrderStatus, OrderStatusType, OrderType, TickerData,
};
use crate::events::ConnectionEvent;
use crate::ExchangeConnector;

#[derive(Debug, Clone)]
pub struct MockConnector {
    config: ConnectorConfig,
    status: Arc<tokio::sync::RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    latency_ms: u64,
    failure_injected: bool,
    order_books: Arc<Mutex<HashMap<Symbol, OrderBook>>>,
    tickers: Arc<Mutex<HashMap<Symbol, TickerData>>>,
}

impl MockConnector {
    pub fn new(exchange_id: ExchangeId) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        let stats = ConnectorStats::default();
        let config = ConnectorConfig {
            exchange_id,
            ..Default::default()
        };

        Self {
            config,
            status: Arc::new(tokio::sync::RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
            event_sender,
            latency_ms: 0,
            failure_injected: false,
            order_books: Arc::new(Mutex::new(HashMap::new())),
            tickers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_latency(mut self, ms: u64) -> Self {
        self.latency_ms = ms;
        self
    }

    pub fn with_failure(mut self) -> Self {
        self.failure_injected = true;
        self
    }

    pub fn set_order_book(&mut self, order_book: OrderBook) {
        let mut books = self.order_books.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned for order_books: {:?}", e);
            std::process::abort();
        });
        books.insert(order_book.symbol.clone(), order_book);
    }

    pub fn set_ticker(&mut self, ticker: TickerData) {
        let mut tickers = self.tickers.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned for tickers: {:?}", e);
            std::process::abort();
        });
        tickers.insert(ticker.symbol.clone(), ticker);
    }

    pub fn inject_failure(&mut self) {
        self.failure_injected = true;
    }

    pub fn clear_failure(&mut self) {
        self.failure_injected = false;
    }

    async fn simulate_latency(&self) {
        if self.latency_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.latency_ms)).await;
        }
    }

    fn check_failure(&self) -> arbitrage_core::Result<()> {
        if self.failure_injected {
            return Err(arbitrage_core::ArbitrageError::Exchange(
                "Injected failure".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl ExchangeConnector for MockConnector {
    fn exchange_id(&self) -> ExchangeId {
        self.config.exchange_id
    }

    fn status(&self) -> ConnectionStatus {
        let status = self.status.try_read();
        match status {
            Ok(s) => s.clone(),
            Err(_) => ConnectionStatus::Disconnected,
        }
    }

    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_sender.subscribe()
    }

    async fn fetch_order_book(&self, symbol: &Symbol) -> arbitrage_core::Result<OrderBook> {
        self.simulate_latency().await;
        self.check_failure()?;

        let books = self.order_books.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned for order_books: {:?}", e);
            std::process::abort();
        });
        match books.get(symbol).cloned() {
            Some(book) => Ok(book),
            None => {
                let order_book = OrderBook::new(
                    self.config.exchange_id,
                    symbol.clone(),
                    Vec::new(),
                    Vec::new(),
                );
                Ok(order_book)
            }
        }
    }

    async fn fetch_symbols(&self) -> arbitrage_core::Result<Vec<Symbol>> {
        self.simulate_latency().await;
        self.check_failure()?;

        Ok(vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
            Symbol::new("SOL", "USDT"),
        ])
    }

    async fn fetch_tickers(
        &self,
        symbols: &[Symbol],
    ) -> arbitrage_core::Result<HashMap<Symbol, TickerData>> {
        self.simulate_latency().await;
        self.check_failure()?;

        let tickers = self.tickers.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned for tickers: {:?}", e);
            std::process::abort();
        });
        let mut result = HashMap::new();
        for symbol in symbols {
            if let Some(ticker) = tickers.get(symbol).cloned() {
                result.insert(symbol.clone(), ticker);
            } else {
                let ticker = TickerData {
                    symbol: symbol.clone(),
                    exchange: self.config.exchange_id,
                    last_price: Decimal::from(50000),
                    bid_price: Decimal::from(49999),
                    ask_price: Decimal::from(50001),
                    volume_24h: Decimal::from(1000000),
                    price_change_24h: Decimal::ZERO,
                    timestamp: Utc::now(),
                };
                result.insert(symbol.clone(), ticker);
            }
        }
        Ok(result)
    }

    async fn fetch_funding_rates(
        &self,
        _symbols: &[Symbol],
    ) -> arbitrage_core::Result<HashMap<Symbol, FundingRate>> {
        self.simulate_latency().await;
        self.check_failure()?;

        Ok(HashMap::new())
    }

    async fn connect(&mut self) -> arbitrage_core::Result<()> {
        self.check_failure()?;

        let mut status = self.status.write().await;
        *status = ConnectionStatus::Connected;

        let mut stats = self.stats.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned for stats: {:?}", e);
            std::process::abort();
        });
        stats.exchange = self.config.exchange_id;

        self.event_sender
            .send(ConnectionEvent::StatusChange {
                exchange: self.config.exchange_id,
                old_status: ConnectionStatus::Disconnected,
                new_status: ConnectionStatus::Connected,
                timestamp: Utc::now(),
            })
            .ok();

        Ok(())
    }

    async fn disconnect(&mut self) -> arbitrage_core::Result<()> {
        let mut status = self.status.write().await;
        let old_status = status.clone();
        *status = ConnectionStatus::Disconnected;

        self.event_sender
            .send(ConnectionEvent::StatusChange {
                exchange: self.config.exchange_id,
                old_status,
                new_status: ConnectionStatus::Disconnected,
                timestamp: Utc::now(),
            })
            .ok();

        Ok(())
    }

    async fn subscribe_symbols(&mut self, _symbols: &[Symbol]) -> arbitrage_core::Result<()> {
        self.check_failure()?;
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
        let status = self.status.read().await.clone();
        let stats = self
            .stats
            .lock()
            .unwrap_or_else(|e| {
                tracing::error!("Mutex poisoned for stats: {:?}", e);
                std::process::abort();
            })
            .clone();

        Ok(HealthStatus {
            is_connected: status == ConnectionStatus::Connected,
            last_message_time: Some(stats.last_update),
            websocket_status: status,
            rest_api_status: ConnectionStatus::Connected,
            error_count: stats.errors_count,
            reconnect_count: stats.reconnections,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        let stats = self.stats.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned for stats: {:?}", e);
            std::process::abort();
        });
        stats.clone()
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    async fn place_order(&self, request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        self.check_failure()?;

        Ok(OrderResponse {
            order_id: format!("mock_order_{}", uuid::Uuid::new_v4().simple()),
            client_order_id: None,
            symbol: request.symbol.clone(),
            side: request.side.clone(),
            order_type: request.order_type.clone(),
            quantity: request.quantity,
            price: request.price,
            status: OrderStatusType::Filled,
            timestamp: Utc::now(),
        })
    }

    async fn cancel_order(&self, order_id: &str) -> arbitrage_core::Result<CancelResponse> {
        self.check_failure()?;
        Ok(CancelResponse {
            order_id: order_id.to_string(),
            client_order_id: None,
            status: OrderStatusType::Cancelled,
            timestamp: Utc::now(),
        })
    }

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        self.check_failure()?;

        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: None,
            symbol: Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(50000)),
            filled_quantity: Decimal::from(1),
            remaining_quantity: Decimal::ZERO,
            average_price: Some(Decimal::from(50000)),
            status: OrderStatusType::Filled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
        self.check_failure()?;

        Ok(Balance {
            exchange: self.config.exchange_id,
            balances: std::collections::HashMap::new(),
            timestamp: Utc::now(),
        })
    }

    async fn get_open_orders(
        &self,
        _symbol: Option<&Symbol>,
    ) -> arbitrage_core::Result<Vec<OrderStatus>> {
        self.check_failure()?;
        Ok(Vec::new())
    }
}

macro_rules! impl_mock_connector {
    ($name:ident, $exchange_id:expr, $price:expr) => {
        pub struct $name;

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait]
        impl ExchangeConnector for $name {
            fn exchange_id(&self) -> ExchangeId {
                $exchange_id
            }

            fn status(&self) -> ConnectionStatus {
                ConnectionStatus::Connected
            }

            fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
                let (sender, _) = broadcast::channel(100);
                sender.subscribe()
            }

            async fn fetch_order_book(&self, symbol: &Symbol) -> arbitrage_core::Result<OrderBook> {
                let order_book = OrderBook::new(
                    $exchange_id,
                    symbol.clone(),
                    vec![OrderBookLevel::new(
                        Decimal::from($price - 10),
                        Decimal::from(1),
                    )],
                    vec![OrderBookLevel::new(Decimal::from($price), Decimal::from(1))],
                );
                Ok(order_book)
            }

            async fn fetch_symbols(&self) -> arbitrage_core::Result<Vec<Symbol>> {
                Ok(vec![Symbol::new("BTC", "USDT"), Symbol::new("ETH", "USDT")])
            }

            async fn fetch_tickers(
                &self,
                symbols: &[Symbol],
            ) -> arbitrage_core::Result<HashMap<Symbol, TickerData>> {
                let mut result = HashMap::new();
                for symbol in symbols {
                    let ticker = TickerData {
                        symbol: symbol.clone(),
                        exchange: $exchange_id,
                        last_price: Decimal::from($price),
                        bid_price: Decimal::from($price - 1),
                        ask_price: Decimal::from($price + 1),
                        volume_24h: Decimal::from(1000000),
                        price_change_24h: Decimal::from(100),
                        timestamp: Utc::now(),
                    };
                    result.insert(symbol.clone(), ticker);
                }
                Ok(result)
            }

            async fn fetch_funding_rates(
                &self,
                _symbols: &[Symbol],
            ) -> arbitrage_core::Result<HashMap<Symbol, FundingRate>> {
                Ok(HashMap::new())
            }

            async fn connect(&mut self) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn disconnect(&mut self) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn subscribe_symbols(
                &mut self,
                _symbols: &[Symbol],
            ) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn unsubscribe_symbols(
                &mut self,
                _symbols: &[Symbol],
            ) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn subscribe_tickers(
                &mut self,
                _symbols: &[Symbol],
            ) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn subscribe_order_books(
                &mut self,
                _symbols: &[Symbol],
            ) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn subscribe_trades(
                &mut self,
                _symbols: &[Symbol],
            ) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn subscribe_funding_rates(
                &mut self,
                _symbols: &[Symbol],
            ) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn health_check(&self) -> arbitrage_core::Result<HealthStatus> {
                Ok(HealthStatus {
                    is_connected: true,
                    last_message_time: Some(Utc::now()),
                    websocket_status: ConnectionStatus::Connected,
                    rest_api_status: ConnectionStatus::Connected,
                    error_count: 0,
                    reconnect_count: 0,
                })
            }

            fn get_stats(&self) -> ConnectorStats {
                ConnectorStats {
                    exchange: $exchange_id,
                    ..Default::default()
                }
            }

            async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
                Ok(())
            }

            async fn place_order(
                &self,
                request: &OrderRequest,
            ) -> arbitrage_core::Result<OrderResponse> {
                Ok(OrderResponse {
                    order_id: format!("mock_{}_order", stringify!($name)),
                    client_order_id: None,
                    symbol: request.symbol.clone(),
                    side: request.side.clone(),
                    order_type: request.order_type.clone(),
                    quantity: request.quantity,
                    price: request.price,
                    status: OrderStatusType::Filled,
                    timestamp: Utc::now(),
                })
            }

            async fn cancel_order(&self, order_id: &str) -> arbitrage_core::Result<CancelResponse> {
                Ok(CancelResponse {
                    order_id: order_id.to_string(),
                    client_order_id: None,
                    status: OrderStatusType::Cancelled,
                    timestamp: Utc::now(),
                })
            }

            async fn get_order_status(
                &self,
                order_id: &str,
            ) -> arbitrage_core::Result<OrderStatus> {
                Ok(OrderStatus {
                    order_id: order_id.to_string(),
                    client_order_id: None,
                    symbol: Symbol::new("BTC", "USDT"),
                    side: OrderSide::Buy,
                    order_type: OrderType::Limit,
                    quantity: Decimal::from(1),
                    price: Some(Decimal::from($price)),
                    filled_quantity: Decimal::from(1),
                    remaining_quantity: Decimal::ZERO,
                    average_price: Some(Decimal::from($price)),
                    status: OrderStatusType::Filled,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                })
            }

            async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
                Ok(Balance {
                    exchange: $exchange_id,
                    balances: std::collections::HashMap::new(),
                    timestamp: Utc::now(),
                })
            }

            async fn get_open_orders(
                &self,
                _symbol: Option<&Symbol>,
            ) -> arbitrage_core::Result<Vec<OrderStatus>> {
                Ok(Vec::new())
            }
        }
    };
}

impl_mock_connector!(MockOKXConnector, ExchangeId::OKX, 50000);
impl_mock_connector!(MockByBitConnector, ExchangeId::ByBit, 50200);
impl_mock_connector!(MockMEXCConnector, ExchangeId::MEXC, 49950);
impl_mock_connector!(MockGateIOConnector, ExchangeId::GateIo, 50025);
impl_mock_connector!(MockKrakenConnector, ExchangeId::Kraken, 50050);
impl_mock_connector!(MockBitstampConnector, ExchangeId::Bitstamp, 49975);

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_connector_fetch_order_book() {
        let connector = MockConnector::new(ExchangeId::OKX);
        let symbol = Symbol::new("BTC", "USDT");

        let order_book = connector.fetch_order_book(&symbol).await.unwrap();
        assert_eq!(order_book.exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_mock_connector_with_latency() {
        let connector = MockConnector::new(ExchangeId::OKX).with_latency(100);
        let symbol = Symbol::new("BTC", "USDT");

        let start = std::time::Instant::now();
        let _ = connector.fetch_order_book(&symbol).await;
        let elapsed = start.elapsed();

        assert!(elapsed >= std::time::Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_mock_connector_with_failure() {
        let connector = MockConnector::new(ExchangeId::OKX).with_failure();
        let symbol = Symbol::new("BTC", "USDT");

        let result = connector.fetch_order_book(&symbol).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_okx_connector() {
        let connector = MockOKXConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::OKX);

        let symbol = Symbol::new("BTC", "USDT");
        let order_book = connector.fetch_order_book(&symbol).await.unwrap();
        assert_eq!(order_book.exchange, ExchangeId::OKX);
    }

    #[tokio::test]
    async fn test_mock_bybit_connector() {
        let connector = MockByBitConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::ByBit);

        let symbol = Symbol::new("BTC", "USDT");
        let order_book = connector.fetch_order_book(&symbol).await.unwrap();
        assert_eq!(order_book.exchange, ExchangeId::ByBit);
    }

    #[tokio::test]
    async fn test_mock_mexc_connector() {
        let connector = MockMEXCConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::MEXC);
    }

    #[tokio::test]
    async fn test_mock_gateio_connector() {
        let connector = MockGateIOConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::GateIo);
    }

    #[tokio::test]
    async fn test_mock_kraken_connector() {
        let connector = MockKrakenConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::Kraken);
    }

    #[tokio::test]
    async fn test_mock_bitstamp_connector() {
        let connector = MockBitstampConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::Bitstamp);
    }

    #[tokio::test]
    async fn test_mock_connector_set_order_book() {
        let mut connector = MockConnector::new(ExchangeId::OKX);
        let symbol = Symbol::new("BTC", "USDT");
        let order_book = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );

        connector.set_order_book(order_book);

        let result = connector.fetch_order_book(&symbol).await.unwrap();
        assert_eq!(result.bids.len(), 1);
        assert_eq!(result.asks.len(), 1);
    }
}
