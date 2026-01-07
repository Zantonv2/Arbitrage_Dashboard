use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::ConnectionEvent;
use crate::utils::{format_symbol, parse_symbol, parse_timestamp, parse_decimal, SymbolFormat};
use arbitrage_core::{types::{ExchangeId, Symbol, OrderBook, OrderBookLevel, ConnectionStatus}, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

pub struct GateIOConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
}

impl GateIOConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::GateIo,
            ws_url: "wss://api.gateio.ws/ws/v4/".to_string(),
            rest_url: "https://api.gateio.ws".to_string(),
            rate_limit_per_second: 30,
            rate_limit_burst: 60,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::GateIo;

        Self {
            config,
            client,
            event_sender,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
        }
    }

    fn symbol_to_gateio(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Underscore)
    }

    fn symbol_from_gateio(&self, gateio_symbol: &str) -> Result<Symbol> {
        parse_symbol(gateio_symbol, SymbolFormat::Underscore)
    }
}

#[async_trait]
impl ExchangeConnector for GateIOConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::GateIo
    }

    fn status(&self) -> ConnectionStatus {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                *self.status.read().await
            })
        })
    }

    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_sender.subscribe()
    }

    async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook> {
        let gateio_symbol = self.symbol_to_gateio(symbol);
        let url = format!("{}/api/v4/spot/order_book?currency_pair={}&limit={}", 
                         self.config.rest_url, gateio_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        self.parse_order_book(&data, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/api/v4/spot/currency_pairs", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(pairs_array) = data.as_array() {
            for item in pairs_array {
                if let Some(id) = item["id"].as_str() {
                    if let Some(trade_status) = item["trade_status"].as_str() {
                        if trade_status == "tradable" {
                            if let Ok(symbol) = self.symbol_from_gateio(id) {
                                symbols.push(symbol);
                            }
                        }
                    }
                }
            }
        }
        Ok(symbols)
    }

    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
        let url = format!("{}/api/v4/spot/tickers", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut tickers = HashMap::new();
        if let Some(ticker_array) = data.as_array() {
            for ticker_data in ticker_array {
                if let Some(currency_pair) = ticker_data["currency_pair"].as_str() {
                    if let Ok(symbol) = self.symbol_from_gateio(currency_pair) {
                        if symbols.contains(&symbol) {
                            if let Ok(ticker) = self.parse_ticker(ticker_data) {
                                tickers.insert(symbol, ticker);
                            }
                        }
                    }
                }
            }
        }
        Ok(tickers)
    }

    async fn fetch_funding_rates(&self, _symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        // Gate.io doesn't provide funding rates in the same way as other exchanges
        Ok(HashMap::new())
    }

    async fn connect(&mut self) -> Result<()> {
        *self.status.write().await = ConnectionStatus::Connected;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        *self.status.write().await = ConnectionStatus::Disconnected;
        Ok(())
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

    async fn health_check(&self) -> Result<HealthStatus> {
        let status = *self.status.read().await;
        let stats = self.stats.lock().await;
        
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
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.stats.lock().await.clone()
            })
        })
    }

    async fn force_reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }
}

impl GateIOConnector {
    fn parse_order_book(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<OrderBook> {
        let asks_data = data["asks"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string()))?;
        let bids_data = data["bids"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bids data".to_string()))?;

        let mut asks = Vec::new();
        for ask in asks_data.iter().take(self.config.order_book_depth as usize) {
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price = parse_decimal(&ask_array[0])?;
                    let quantity = parse_decimal(&ask_array[1])?;
                    asks.push(OrderBookLevel { price, quantity });
                }
            }
        }

        let mut bids = Vec::new();
        for bid in bids_data.iter().take(self.config.order_book_depth as usize) {
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price = parse_decimal(&bid_array[0])?;
                    let quantity = parse_decimal(&bid_array[1])?;
                    bids.push(OrderBookLevel { price, quantity });
                }
            }
        }

        Ok(OrderBook {
            exchange: ExchangeId::GateIo,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_ticker(&self, data: &serde_json::Value) -> Result<TickerData> {
        let currency_pair = data["currency_pair"].as_str().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing currency_pair".to_string()))?;
        let symbol = self.symbol_from_gateio(currency_pair)?;
        
        Ok(TickerData {
            symbol,
            exchange: ExchangeId::GateIo,
            last_price: parse_decimal(&data["last"])?,
            bid_price: parse_decimal(&data["highest_bid"])?,
            ask_price: parse_decimal(&data["lowest_ask"])?,
            volume_24h: parse_decimal(&data["base_volume"]).unwrap_or_default(),
            price_change_24h: parse_decimal(&data["change_percentage"]).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }
}