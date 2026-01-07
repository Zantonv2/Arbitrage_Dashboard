use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::ConnectionEvent;
use crate::utils::{format_symbol, parse_symbol, parse_timestamp, parse_decimal, SymbolFormat};
use arbitrage_core::{types::{ExchangeId, Symbol, OrderBook, OrderBookLevel, ConnectionStatus}, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

pub struct BybitConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
}

impl BybitConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::ByBit,
            ws_url: "wss://stream.bybit.com/v5/public/spot".to_string(),
            rest_url: "https://api.bybit.com".to_string(),
            rate_limit_per_second: 60,
            rate_limit_burst: 120,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::ByBit;

        Self {
            config,
            client,
            event_sender,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
        }
    }

    fn symbol_to_bybit(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    fn symbol_from_bybit(&self, bybit_symbol: &str) -> Result<Symbol> {
        parse_symbol(bybit_symbol, SymbolFormat::NoSeparator)
    }
}

#[async_trait]
impl ExchangeConnector for BybitConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::ByBit
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
        let bybit_symbol = self.symbol_to_bybit(symbol);
        let url = format!("{}/v5/market/orderbook?category=spot&symbol={}&limit={}", 
                         self.config.rest_url, bybit_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        if let Some(result) = data["result"].as_object() {
            return self.parse_order_book(result, symbol);
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection("No order book data".to_string()))
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/v5/market/instruments-info?category=spot", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(result) = data["result"].as_object() {
            if let Some(list) = result["list"].as_array() {
                for item in list {
                    if let Some(symbol_str) = item["symbol"].as_str() {
                        if let Ok(symbol) = self.symbol_from_bybit(symbol_str) {
                            symbols.push(symbol);
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
            let bybit_symbol = self.symbol_to_bybit(symbol);
            let url = format!("{}/v5/market/tickers?category=spot&symbol={}", self.config.rest_url, bybit_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(result) = data["result"].as_object() {
                        if let Some(list) = result["list"].as_array() {
                            if let Some(ticker_data) = list.first() {
                                if let Ok(ticker) = self.parse_ticker(ticker_data) {
                                    tickers.insert(symbol.clone(), ticker);
                                }
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
            let bybit_symbol = self.symbol_to_bybit(symbol);
            let url = format!("{}/v5/market/funding/history?category=linear&symbol={}&limit=1", 
                             self.config.rest_url, bybit_symbol);
            
            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(result) = data["result"].as_object() {
                        if let Some(list) = result["list"].as_array() {
                            if let Some(funding_data) = list.first() {
                                if let Ok(funding_rate) = self.parse_funding_rate(funding_data, symbol) {
                                    funding_rates.insert(symbol.clone(), funding_rate);
                                }
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

impl BybitConnector {
    fn parse_order_book(&self, data: &serde_json::Map<String, serde_json::Value>, symbol: &Symbol) -> Result<OrderBook> {
        let asks_data = data["a"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string()))?;
        let bids_data = data["b"].as_array().ok_or_else(|| 
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
            exchange: ExchangeId::ByBit,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_ticker(&self, data: &serde_json::Value) -> Result<TickerData> {
        let symbol_str = data["symbol"].as_str().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing symbol".to_string()))?;
        let symbol = self.symbol_from_bybit(symbol_str)?;
        
        Ok(TickerData {
            symbol,
            exchange: ExchangeId::ByBit,
            last_price: parse_decimal(&data["lastPrice"])?,
            bid_price: parse_decimal(&data["bid1Price"])?,
            ask_price: parse_decimal(&data["ask1Price"])?,
            volume_24h: parse_decimal(&data["volume24h"]).unwrap_or_default(),
            price_change_24h: parse_decimal(&data["price24hPcnt"]).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_funding_rate(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["fundingRate"])?;
        let funding_time = parse_timestamp(&data["fundingRateTimestamp"])?;
        
        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::ByBit,
            funding_rate,
            predicted_rate: None,
            funding_time,
            timestamp: chrono::Utc::now(),
        })
    }
}