use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::ConnectionEvent;
use crate::utils::{format_symbol, parse_symbol, parse_timestamp, parse_decimal, SymbolFormat};
use arbitrage_core::{types::{ExchangeId, Symbol, OrderBook, OrderBookLevel, ConnectionStatus}, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

pub struct BitstampConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
}

impl BitstampConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::Bitstamp,
            ws_url: "wss://ws.bitstamp.net".to_string(),
            rest_url: "https://www.bitstamp.net".to_string(),
            rate_limit_per_second: 8000,  // Bitstamp has high rate limits
            rate_limit_burst: 16000,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::Bitstamp;

        Self {
            config,
            client,
            event_sender,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
        }
    }

    fn symbol_to_bitstamp(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator).to_lowercase()
    }

    fn symbol_from_bitstamp(&self, bitstamp_symbol: &str) -> Result<Symbol> {
        // Bitstamp uses lowercase symbols without separators
        let uppercase_symbol = bitstamp_symbol.to_uppercase();
        parse_symbol(&uppercase_symbol, SymbolFormat::NoSeparator)
    }
}

#[async_trait]
impl ExchangeConnector for BitstampConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Bitstamp
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
        let bitstamp_symbol = self.symbol_to_bitstamp(symbol);
        let url = format!("{}/api/v2/order_book/{}/", self.config.rest_url, bitstamp_symbol);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        self.parse_order_book(&data, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/api/v2/trading-pairs-info/", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(pairs_array) = data.as_array() {
            for item in pairs_array {
                if let Some(url_symbol) = item["url_symbol"].as_str() {
                    if let Some(trading) = item["trading"].as_str() {
                        if trading == "Enabled" {
                            if let Ok(symbol) = self.symbol_from_bitstamp(url_symbol) {
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
        let mut tickers = HashMap::new();
        for symbol in symbols {
            let bitstamp_symbol = self.symbol_to_bitstamp(symbol);
            let url = format!("{}/api/v2/ticker/{}/", self.config.rest_url, bitstamp_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Ok(ticker) = self.parse_ticker(&data, symbol) {
                        tickers.insert(symbol.clone(), ticker);
                    }
                }
            }
        }
        Ok(tickers)
    }

    async fn fetch_funding_rates(&self, _symbols: &[Symbol]) -> Result<HashMap<Symbol, FundingRate>> {
        // Bitstamp doesn't offer perpetual contracts with funding rates
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

impl BitstampConnector {
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
            exchange: ExchangeId::Bitstamp,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_ticker(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<TickerData> {
        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::Bitstamp,
            last_price: parse_decimal(&data["last"])?,
            bid_price: parse_decimal(&data["bid"])?,
            ask_price: parse_decimal(&data["ask"])?,
            volume_24h: parse_decimal(&data["volume"]).unwrap_or_default(),
            price_change_24h: rust_decimal::Decimal::ZERO, // Bitstamp doesn't provide 24h change directly
            timestamp: chrono::Utc::now(),
        })
    }
}