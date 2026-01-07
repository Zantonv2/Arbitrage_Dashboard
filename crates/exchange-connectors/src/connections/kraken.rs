use crate::connector::{ExchangeConnector, ConnectorConfig, ConnectorStats, HealthStatus, TickerData, FundingRate};
use crate::events::ConnectionEvent;
use crate::utils::{format_symbol, parse_symbol, parse_timestamp, parse_decimal, SymbolFormat};
use arbitrage_core::{types::{ExchangeId, Symbol, OrderBook, OrderBookLevel, ConnectionStatus}, Result};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

pub struct KrakenConnector {
    config: ConnectorConfig,
    client: Client,
    event_sender: broadcast::Sender<ConnectionEvent>,
    status: Arc<RwLock<ConnectionStatus>>,
    stats: Arc<Mutex<ConnectorStats>>,
}

impl KrakenConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::Kraken,
            ws_url: "wss://ws.kraken.com".to_string(),
            rest_url: "https://api.kraken.com".to_string(),
            rate_limit_per_second: 1,  // Kraken has strict rate limits
            rate_limit_burst: 2,
            ..Default::default()
        };

        let (event_sender, _) = broadcast::channel(1000);
        let client = Client::new();
        let mut stats = ConnectorStats::default();
        stats.exchange = ExchangeId::Kraken;

        Self {
            config,
            client,
            event_sender,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            stats: Arc::new(Mutex::new(stats)),
        }
    }

    fn symbol_to_kraken(&self, symbol: &Symbol) -> String {
        // Kraken uses special naming conventions
        let base = &symbol.base;
        let quote = &symbol.quote;
        
        // Map common symbols to Kraken format
        let kraken_base = match base.as_str() {
            "BTC" => "XBT",
            "ETH" => "ETH",
            "USD" => "USD",
            "USDT" => "USDT",
            _ => base,
        };
        
        let kraken_quote = match quote.as_str() {
            "BTC" => "XBT",
            "ETH" => "ETH", 
            "USD" => "USD",
            "USDT" => "USDT",
            _ => quote,
        };
        
        format!("{}{}", kraken_base, kraken_quote)
    }

    fn symbol_from_kraken(&self, kraken_symbol: &str) -> Result<Symbol> {
        // This is simplified - Kraken has complex symbol mapping
        // In production, you'd need a proper mapping table
        if kraken_symbol.len() >= 6 {
            let (base_part, quote_part) = kraken_symbol.split_at(3);
            
            let base = match base_part {
                "XBT" => "BTC",
                _ => base_part,
            };
            
            let quote = match quote_part {
                "XBT" => "BTC",
                _ => quote_part,
            };
            
            Ok(Symbol {
                base: base.to_string(),
                quote: quote.to_string(),
            })
        } else {
            Err(arbitrage_core::ArbitrageError::Validation(
                format!("Invalid Kraken symbol format: {}", kraken_symbol)
            ))
        }
    }
}

#[async_trait]
impl ExchangeConnector for KrakenConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Kraken
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
        let kraken_symbol = self.symbol_to_kraken(symbol);
        let url = format!("{}/0/public/Depth?pair={}&count={}", 
                         self.config.rest_url, kraken_symbol, self.config.order_book_depth);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        if let Some(result) = data["result"].as_object() {
            // Kraken returns data with the symbol as key
            if let Some((_, book_data)) = result.iter().next() {
                return self.parse_order_book(book_data, symbol);
            }
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection("No order book data".to_string()))
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/0/public/AssetPairs", self.config.rest_url);
        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        let mut symbols = Vec::new();
        if let Some(result) = data["result"].as_object() {
            for (pair_name, pair_info) in result {
                if let Some(status) = pair_info["status"].as_str() {
                    if status == "online" {
                        if let Ok(symbol) = self.symbol_from_kraken(pair_name) {
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
        
        // Kraken allows multiple pairs in one request
        let kraken_symbols: Vec<String> = symbols.iter()
            .map(|s| self.symbol_to_kraken(s))
            .collect();
        
        let pairs_param = kraken_symbols.join(",");
        let url = format!("{}/0/public/Ticker?pair={}", self.config.rest_url, pairs_param);
        
        if let Ok(response) = self.client.get(&url).send().await {
            if let Ok(data) = response.json::<serde_json::Value>().await {
                if let Some(result) = data["result"].as_object() {
                    for (pair_name, ticker_data) in result {
                        if let Ok(symbol) = self.symbol_from_kraken(pair_name) {
                            if let Ok(ticker) = self.parse_ticker(ticker_data, &symbol) {
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
        // Kraken doesn't offer perpetual contracts with funding rates
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

impl KrakenConnector {
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
            exchange: ExchangeId::Kraken,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_ticker(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<TickerData> {
        // Kraken ticker format: {"a":["ask_price","ask_volume","ask_lot_volume"],"b":["bid_price","bid_volume","bid_lot_volume"],"c":["last_price","last_volume"],...}
        let ask_data = data["a"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing ask data".to_string()))?;
        let bid_data = data["b"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bid data".to_string()))?;
        let last_data = data["c"].as_array().ok_or_else(|| 
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing last price data".to_string()))?;
        
        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::Kraken,
            last_price: parse_decimal(&last_data[0])?,
            bid_price: parse_decimal(&bid_data[0])?,
            ask_price: parse_decimal(&ask_data[0])?,
            volume_24h: parse_decimal(&data["v"][1]).unwrap_or_default(),
            price_change_24h: rust_decimal::Decimal::ZERO, // Would need to calculate from open price
            timestamp: chrono::Utc::now(),
        })
    }
}