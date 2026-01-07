use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::ConnectionEvent;
use crate::utils::{format_symbol, parse_symbol, parse_timestamp, parse_decimal, SymbolFormat};
use arbitrage_core::{types::{ExchangeId, Symbol, OrderBook, OrderBookLevel, ConnectionStatus}, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

pub struct OKXConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
}

impl OKXConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::OKX,
            ws_url: "wss://ws.okx.com:8443/ws/v5/public".to_string(),
            rest_url: "https://www.okx.com".to_string(),
            rate_limit_per_second: 40,
            rate_limit_burst: 80,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::OKX;

        Self {
            config,
            client,
            event_sender,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
        }
    }

    fn symbol_to_okx(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Dash)
    }

    fn symbol_from_okx(&self, okx_symbol: &str) -> Result<Symbol> {
        parse_symbol(okx_symbol, SymbolFormat::Dash)
    }
}

#[async_trait]
impl ExchangeConnector for OKXConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::OKX
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
        let okx_symbol = self.symbol_to_okx(symbol);
        let url = format!("{}/api/v5/market/books?instId={}&sz={}", 
                         self.config.rest_url, okx_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        if let Some(data_array) = data["data"].as_array() {
            if let Some(book_data) = data_array.first() {
                return self.parse_order_book(book_data, symbol);
            }
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection("No order book data".to_string()))
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/api/v5/public/instruments?instType=SPOT", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(data_array) = data["data"].as_array() {
            for item in data_array {
                if let Some(inst_id) = item["instId"].as_str() {
                    if let Ok(symbol) = self.symbol_from_okx(inst_id) {
                        symbols.push(symbol);
                    }
                }
            }
        }
        Ok(symbols)
    }

    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
        let mut tickers = HashMap::new();
        for symbol in symbols {
            let okx_symbol = self.symbol_to_okx(symbol);
            let url = format!("{}/api/v5/market/ticker?instId={}", self.config.rest_url, okx_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(data_array) = data["data"].as_array() {
                        if let Some(ticker_data) = data_array.first() {
                            if let Ok(ticker) = self.parse_ticker(ticker_data) {
                                tickers.insert(symbol.clone(), ticker);
                            }
                        }
                    }
                }
            }
        }
        Ok(tickers)
    }

    async fn fetch_funding_rates(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        let mut funding_rates = HashMap::new();
        for symbol in symbols {
            let okx_symbol = format!("{}-SWAP", self.symbol_to_okx(symbol));
            let url = format!("{}/api/v5/public/funding-rate?instId={}", self.config.rest_url, okx_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(data_array) = data["data"].as_array() {
                        if let Some(funding_data) = data_array.first() {
                            if let Ok(funding_rate) = self.parse_funding_rate(funding_data, symbol) {
                                funding_rates.insert(symbol.clone(), funding_rate);
                            }
                        }
                    }
                }
            }
        }
        Ok(funding_rates)
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

impl OKXConnector {
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
            exchange: ExchangeId::OKX,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_ticker(&self, data: &serde_json::Value) -> Result<TickerData> {
        let inst_id = data["instId"].as_str().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing instId".to_string()))?;
        let symbol = self.symbol_from_okx(inst_id)?;
        
        Ok(TickerData {
            symbol,
            exchange: ExchangeId::OKX,
            last_price: parse_decimal(&data["last"])?,
            bid_price: parse_decimal(&data["bidPx"])?,
            ask_price: parse_decimal(&data["askPx"])?,
            volume_24h: parse_decimal(&data["vol24h"]).unwrap_or_default(),
            price_change_24h: parse_decimal(&data["changeUtc8"]).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_funding_rate(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["fundingRate"])?;
        let funding_time = parse_timestamp(&data["fundingTime"])?;
        
        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::OKX,
            funding_rate,
            predicted_rate: data["nextFundingRate"].as_str()
                .and_then(|s| s.parse().ok()),
            funding_time,
            timestamp: chrono::Utc::now(),
        })
    }
}