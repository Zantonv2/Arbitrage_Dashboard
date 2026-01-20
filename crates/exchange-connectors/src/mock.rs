use arbitrage_core::types::{ConnectionStatus, ExchangeId, OrderBook, Symbol};
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::connector::{
    Balance, CancelResponse, ConnectorConfig, ConnectorStats, FundingRate, HealthStatus,
    OrderRequest, OrderResponse, OrderSide, OrderStatus, OrderStatusType, OrderType, TickerData,
    TimeInForce,
};
use crate::events::ConnectionEvent;
use crate::ExchangeConnector;

#[derive(Debug, Clone)]
pub struct MockConnector {
    config: ConnectorConfig,
    status: Arc<tokio::sync::RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
    event_sender: broadcast::Sender<ConnectionEvent>,
    latency_ms: Arc<Mutex<u64>>,
    failure_injected: Arc<Mutex<bool>>,
    order_books: Arc<Mutex<HashMap<Symbol, OrderBook>>>,
    tickers: Arc<Mutex<HashMap<Symbol, TickerData>>>,
}

impl MockConnector {
    pub fn new(exchange_id: ExchangeId) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        let stats = ConnectorStats::default();
        let stats = Mutex::new(stats);

        Self {
            config: ConnectorConfig {
                exchange_id,
                order_book_depth: 20,
                ..Default::default()
            },
            status: Arc::new(tokio::sync::RwLock::new(ConnectionStatus::Disconnected)),
            stats,
            event_sender,
            latency_ms: Arc::new(Mutex::new(0)),
            failure_injected: Arc::new(Mutex::new(false)),
            order_books: Arc::new(Mutex::new(HashMap::new())),
            tickers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_latency(mut self, ms: u64) -> Self {
        *self.latency_ms = Mutex::new(ms);
        self
    }

    pub fn with_failure(mut self) -> Self {
        *self.failure_injected = Mutex::new(true);
        self
    }

    pub fn set_order_book(&mut self, order_book: OrderBook) {
        let mut books = self.order_books.lock().unwrap();
        books.insert(order_book.symbol.clone(), order_book);
    }

    pub fn set_ticker(&mut self, ticker: TickerData) {
        let mut tickers = self.tickers.lock().unwrap();
        tickers.insert(ticker.symbol.clone(), ticker);
    }

    pub fn inject_failure(&self) {
        let mut failure = self.failure_injected.lock().unwrap();
        *failure = true;
    }

    pub fn clear_failure(&self) {
        let mut failure = self.failure_injected.lock().unwrap();
        *failure = false;
    }

    async fn simulate_latency(&self) {
        let latency = {
            let guard = self.latency_ms.lock().unwrap();
            *guard
        };
        if latency > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(latency)).await;
        }
    }

    fn check_failure(&self) -> arbitrage_core::Result<()> {
        let failure = self.failure_injected.lock().unwrap();
        if *failure {
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

        let books = self.order_books.lock().unwrap();
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

        let tickers = self.tickers.lock().unwrap();
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

        let mut stats = self.stats.lock().unwrap();
        stats.exchange = self.config.exchange_id;

        self.event_sender
            .send(ConnectionEvent::Connected(self.config.exchange_id))
            .ok();

        Ok(())
    }

    async fn disconnect(&mut self) -> arbitrage_core::Result<()> {
        let mut status = self.status.write().await;
        *status = ConnectionStatus::Disconnected;

        self.event_sender
            .send(ConnectionEvent::Disconnected(self.config.exchange_id))
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
        let stats = self.stats.lock().unwrap().clone();

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
        let stats = self.stats.lock().unwrap();
        stats.clone()
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    async fn place_order(&self, _request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        self.check_failure()?;

        Ok(OrderResponse {
            order_id: format!("mock_order_{}", uuid::Uuid::new_v4().simple()),
            client_order_id: None,
            symbol: self.config.exchange_id.to_string(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(50000)),
            status: OrderStatusType::Filled,
            timestamp: Utc::now(),
        })
    }

    async fn cancel_order(&self, _order_id: &str) -> arbitrage_core::Result<CancelResponse> {
        self.check_failure()?;

        Ok(CancelResponse {
            order_id: _order_id.to_string(),
            client_order_id: None,
            status: OrderStatusType::Cancelled,
            timestamp: Utc::now(),
        })
    }

    async fn get_order_status(&self, _order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        self.check_failure()?;

        Ok(OrderStatus {
            order_id: _order_id.to_string(),
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

pub struct MockOKXConnector;

impl MockOKXConnector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExchangeConnector for MockOKXConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::OKX
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
            ExchangeId::OKX,
            symbol.clone(),
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50000),
                Decimal::from(1),
            )],
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50010),
                Decimal::from(1),
            )],
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
                exchange: ExchangeId::OKX,
                last_price: Decimal::from(50000),
                bid_price: Decimal::from(49999),
                ask_price: Decimal::from(50001),
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
            last_message_time: Some(Utc::now()),
            websocket_status: ConnectionStatus::Connected,
            rest_api_status: ConnectionStatus::Connected,
            error_count: 0,
            reconnect_count: 0,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        ConnectorStats {
            exchange: ExchangeId::OKX,
            ..Default::default()
        }
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn place_order(&self, _request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        Ok(OrderResponse {
            order_id: "mock_okx_order".to_string(),
            client_order_id: None,
            symbol: _request.symbol.to_string(),
            side: _request.side,
            order_type: _request.order_type,
            quantity: _request.quantity,
            price: _request.price,
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

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
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
        Ok(Balance {
            exchange: ExchangeId::OKX,
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

pub struct MockByBitConnector;

impl MockByBitConnector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExchangeConnector for MockByBitConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::ByBit
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
            ExchangeId::ByBit,
            symbol.clone(),
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50200),
                Decimal::from(1),
            )],
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50210),
                Decimal::from(1),
            )],
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
                exchange: ExchangeId::ByBit,
                last_price: Decimal::from(50200),
                bid_price: Decimal::from(50199),
                ask_price: Decimal::from(50201),
                volume_24h: Decimal::from(1500000),
                price_change_24h: Decimal::from(200),
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
            last_message_time: Some(Utc::now()),
            websocket_status: ConnectionStatus::Connected,
            rest_api_status: ConnectionStatus::Connected,
            error_count: 0,
            reconnect_count: 0,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        ConnectorStats {
            exchange: ExchangeId::ByBit,
            ..Default::default()
        }
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn place_order(&self, _request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        Ok(OrderResponse {
            order_id: "mock_bybit_order".to_string(),
            client_order_id: None,
            symbol: _request.symbol.to_string(),
            side: _request.side,
            order_type: _request.order_type,
            quantity: _request.quantity,
            price: _request.price,
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

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: None,
            symbol: Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(50200)),
            filled_quantity: Decimal::from(1),
            remaining_quantity: Decimal::ZERO,
            average_price: Some(Decimal::from(50200)),
            status: OrderStatusType::Filled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
        Ok(Balance {
            exchange: ExchangeId::ByBit,
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

pub struct MockMEXCConnector;

impl MockMEXCConnector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExchangeConnector for MockMEXCConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::MEXC
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
            ExchangeId::MEXC,
            symbol.clone(),
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(49950),
                Decimal::from(2),
            )],
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(49960),
                Decimal::from(2),
            )],
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
                exchange: ExchangeId::MEXC,
                last_price: Decimal::from(49955),
                bid_price: Decimal::from(49950),
                ask_price: Decimal::from(49960),
                volume_24h: Decimal::from(800000),
                price_change_24h: Decimal::from(-50),
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
            last_message_time: Some(Utc::now()),
            websocket_status: ConnectionStatus::Connected,
            rest_api_status: ConnectionStatus::Connected,
            error_count: 0,
            reconnect_count: 0,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        ConnectorStats {
            exchange: ExchangeId::MEXC,
            ..Default::default()
        }
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn place_order(&self, _request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        Ok(OrderResponse {
            order_id: "mock_mexc_order".to_string(),
            client_order_id: None,
            symbol: _request.symbol.to_string(),
            side: _request.side,
            order_type: _request.order_type,
            quantity: _request.quantity,
            price: _request.price,
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

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: None,
            symbol: Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(49950)),
            filled_quantity: Decimal::from(1),
            remaining_quantity: Decimal::ZERO,
            average_price: Some(Decimal::from(49950)),
            status: OrderStatusType::Filled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
        Ok(Balance {
            exchange: ExchangeId::MEXC,
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

pub struct MockGateIOConnector;

impl MockGateIOConnector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExchangeConnector for MockGateIOConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::GateIo
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
            ExchangeId::GateIo,
            symbol.clone(),
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50025),
                Decimal::from(1),
            )],
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50035),
                Decimal::from(1),
            )],
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
                exchange: ExchangeId::GateIo,
                last_price: Decimal::from(50030),
                bid_price: Decimal::from(50025),
                ask_price: Decimal::from(50035),
                volume_24h: Decimal::from(600000),
                price_change_24h: Decimal::from(30),
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
            last_message_time: Some(Utc::now()),
            websocket_status: ConnectionStatus::Connected,
            rest_api_status: ConnectionStatus::Connected,
            error_count: 0,
            reconnect_count: 0,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        ConnectorStats {
            exchange: ExchangeId::GateIo,
            ..Default::default()
        }
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn place_order(&self, _request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        Ok(OrderResponse {
            order_id: "mock_gateio_order".to_string(),
            client_order_id: None,
            symbol: _request.symbol.to_string(),
            side: _request.side,
            order_type: _request.order_type,
            quantity: _request.quantity,
            price: _request.price,
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

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: None,
            symbol: Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(50025)),
            filled_quantity: Decimal::from(1),
            remaining_quantity: Decimal::ZERO,
            average_price: Some(Decimal::from(50025)),
            status: OrderStatusType::Filled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
        Ok(Balance {
            exchange: ExchangeId::GateIo,
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

pub struct MockKrakenConnector;

impl MockKrakenConnector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExchangeConnector for MockKrakenConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Kraken
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
            ExchangeId::Kraken,
            symbol.clone(),
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50050),
                Decimal::from(1),
            )],
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50060),
                Decimal::from(1),
            )],
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
                exchange: ExchangeId::Kraken,
                last_price: Decimal::from(50055),
                bid_price: Decimal::from(50050),
                ask_price: Decimal::from(50060),
                volume_24h: Decimal::from(400000),
                price_change_24h: Decimal::from(55),
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
            last_message_time: Some(Utc::now()),
            websocket_status: ConnectionStatus::Connected,
            rest_api_status: ConnectionStatus::Connected,
            error_count: 0,
            reconnect_count: 0,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        ConnectorStats {
            exchange: ExchangeId::Kraken,
            ..Default::default()
        }
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn place_order(&self, _request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        Ok(OrderResponse {
            order_id: "mock_kraken_order".to_string(),
            client_order_id: None,
            symbol: _request.symbol.to_string(),
            side: _request.side,
            order_type: _request.order_type,
            quantity: _request.quantity,
            price: _request.price,
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

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: None,
            symbol: Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(50050)),
            filled_quantity: Decimal::from(1),
            remaining_quantity: Decimal::ZERO,
            average_price: Some(Decimal::from(50050)),
            status: OrderStatusType::Filled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
        Ok(Balance {
            exchange: ExchangeId::Kraken,
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

pub struct MockBitstampConnector;

impl MockBitstampConnector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExchangeConnector for MockBitstampConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Bitstamp
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
            ExchangeId::Bitstamp,
            symbol.clone(),
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(49975),
                Decimal::from(1),
            )],
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(49985),
                Decimal::from(1),
            )],
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
                exchange: ExchangeId::Bitstamp,
                last_price: Decimal::from(49980),
                bid_price: Decimal::from(49975),
                ask_price: Decimal::from(49985),
                volume_24h: Decimal::from(200000),
                price_change_24h: Decimal::from(-20),
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
            last_message_time: Some(Utc::now()),
            websocket_status: ConnectionStatus::Connected,
            rest_api_status: ConnectionStatus::Connected,
            error_count: 0,
            reconnect_count: 0,
        })
    }

    fn get_stats(&self) -> ConnectorStats {
        ConnectorStats {
            exchange: ExchangeId::Bitstamp,
            ..Default::default()
        }
    }

    async fn force_reconnect(&mut self) -> arbitrage_core::Result<()> {
        Ok(())
    }

    async fn place_order(&self, _request: &OrderRequest) -> arbitrage_core::Result<OrderResponse> {
        Ok(OrderResponse {
            order_id: "mock_bitstamp_order".to_string(),
            client_order_id: None,
            symbol: _request.symbol.to_string(),
            side: _request.side,
            order_type: _request.order_type,
            quantity: _request.quantity,
            price: _request.price,
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

    async fn get_order_status(&self, order_id: &str) -> arbitrage_core::Result<OrderStatus> {
        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: None,
            symbol: Symbol::new("BTC", "USDT"),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(49975)),
            filled_quantity: Decimal::from(1),
            remaining_quantity: Decimal::ZERO,
            average_price: Some(Decimal::from(49975)),
            status: OrderStatusType::Filled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn get_balance(&self) -> arbitrage_core::Result<Balance> {
        Ok(Balance {
            exchange: ExchangeId::Bitstamp,
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
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50000),
                Decimal::from(1),
            )],
            vec![arbitrage_core::types::OrderBookLevel::new(
                Decimal::from(50010),
                Decimal::from(1),
            )],
        );

        connector.set_order_book(order_book);

        let result = connector.fetch_order_book(&symbol).await.unwrap();
        assert_eq!(result.bids.len(), 1);
        assert_eq!(result.asks.len(), 1);
    }
}
