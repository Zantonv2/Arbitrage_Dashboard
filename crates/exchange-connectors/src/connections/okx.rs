use crate::connector::{
    CancelResponse, ConnectorConfig, ConnectorStats, ExchangeConnector, FundingRate, HealthStatus,
    OrderStatus, OrderStatusType, TickerData,
};
use crate::connector_trait::ConnectorBase;
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{
    format_symbol, parse_decimal, parse_json_with_retry, parse_symbol, parse_timestamp,
    ExponentialBackoff, SymbolFormat,
};
use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};

#[derive(Clone)]
pub struct OKXConnector {
    pub base: ConnectorBase,
    client: Client,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
    ws_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    parsing_failures: Arc<AtomicU64>,
}

impl Default for OKXConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl OKXConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::OKX,
            ws_url: "wss://ws.okx.com:8443/ws/v5/public".to_string(),
            rest_url: "https://www.okx.com".to_string(),
            rate_limit_per_second: 20,
            rate_limit_burst: 40,
            ..Default::default()
        };

        let base = ConnectorBase::new(config);
        let client = Client::new();

        Self {
            base,
            client,
            subscribed_symbols: Arc::new(RwLock::new(Vec::new())),
            ws_handle: Arc::new(Mutex::new(None)),
            parsing_failures: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn symbol_to_okx(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::Dash)
    }

    pub fn symbol_from_okx(&self, okx_symbol: &str) -> Result<Symbol> {
        parse_symbol(okx_symbol, SymbolFormat::Dash)
    }

    pub fn get_parsing_failure_count(&self) -> u64 {
        self.parsing_failures.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ExchangeConnector for OKXConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::OKX
    }

    fn status(&self) -> ConnectionStatus {
        match self.base.status.try_read() {
            Ok(status) => status.clone(),
            Err(_) => ConnectionStatus::Disconnected,
        }
    }

    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.base.event_sender.subscribe()
    }

    async fn fetch_order_book(&self, symbol: &Symbol) -> Result<OrderBook> {
        let okx_symbol = self.symbol_to_okx(symbol);
        let url = format!(
            "{}/api/v5/market/books?instId={}&sz={}",
            self.base.config.rest_url, okx_symbol, self.base.config.order_book_depth
        );

        let response = self.client.get(&url).send().await?;
        let bytes = response.bytes().await?;
        let order_book_json = parse_json_with_retry(
            &bytes,
            ExchangeId::OKX,
            "fetch_order_book",
            Some(&self.parsing_failures),
        )
        .await?;

        if let Some(data_array) = order_book_json["data"].as_array() {
            if let Some(book) = data_array.first() {
                return self.parse_order_book(book, symbol);
            }
        }

        Err(arbitrage_core::ArbitrageError::ExchangeConnection(
            "No order book data".to_string(),
        ))
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!(
            "{}/api/v5/public/instruments?instType=SPOT",
            self.base.config.rest_url
        );
        let response = self.client.get(&url).send().await?;
        let bytes = response.bytes().await?;
        let symbols_json = parse_json_with_retry(
            &bytes,
            ExchangeId::OKX,
            "fetch_symbols",
            Some(&self.parsing_failures),
        )
        .await?;

        let mut symbols = Vec::with_capacity(512);
        if let Some(data_array) = symbols_json["data"].as_array() {
            for item in data_array {
                if let Some(inst_id) = item["instId"].as_str() {
                    if let Some(state) = item["state"].as_str() {
                        if state == "live" {
                            if let Ok(symbol) = self.symbol_from_okx(inst_id) {
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
        let mut tickers = HashMap::with_capacity(symbols.len());
        for symbol in symbols {
            let okx_symbol = self.symbol_to_okx(symbol);
            let url = format!(
                "{}/api/v5/market/ticker?instId={}",
                self.base.config.rest_url, okx_symbol
            );

            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(data_array) = data["data"].as_array() {
                        if let Some(ticker_data) = data_array.first() {
                            if let Ok(ticker) = self.parse_ticker(ticker_data, symbol) {
                                tickers.insert(symbol.clone(), ticker);
                            }
                        }
                    }
                }
            }
        }
        Ok(tickers)
    }

    async fn fetch_funding_rates(
        &self,
        symbols: &[Symbol],
    ) -> Result<HashMap<Symbol, FundingRate>> {
        let mut funding_rates = HashMap::with_capacity(symbols.len());
        for symbol in symbols {
            let okx_symbol = format!("{}-SWAP", self.symbol_to_okx(symbol));
            let url = format!(
                "{}/api/v5/public/funding-rate?instId={}",
                self.base.config.rest_url, okx_symbol
            );

            if let Ok(response) = self.client.get(&url).send().await {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(data_array) = data["data"].as_array() {
                        if let Some(funding_data) = data_array.first() {
                            if let Ok(funding_rate) = self.parse_funding_rate(funding_data, symbol)
                            {
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
        self.base.connect().await
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.base.disconnect().await
    }

    async fn subscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        let mut subscribed = self.subscribed_symbols.write().await;
        for symbol in symbols {
            if !subscribed.contains(symbol) {
                subscribed.push(symbol.clone());
            }
        }
        Ok(())
    }

    async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        let mut subscribed = self.subscribed_symbols.write().await;
        subscribed.retain(|s| !symbols.contains(s));
        Ok(())
    }

    async fn subscribe_tickers(&mut self, _symbols: &[Symbol]) -> Result<()> {
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!(
            "Subscribing to OKX order books for {} symbols",
            symbols.len()
        );
        self.subscribe_symbols(symbols).await?;
        Ok(())
    }

    async fn subscribe_trades(&mut self, _symbols: &[Symbol]) -> Result<()> {
        Ok(())
    }

    async fn subscribe_funding_rates(&mut self, _symbols: &[Symbol]) -> Result<()> {
        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let status = self.status();
        let stats = self.get_stats();
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
        self.base.get_stats()
    }

    async fn force_reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    // === Trading Methods ===

    async fn place_order(
        &self,
        order: &crate::connector::OrderRequest,
    ) -> Result<crate::connector::OrderResponse> {
        use crate::connector::{OrderResponse, OrderStatusType};

        // OKX API endpoint for placing orders
        let url = format!("{}/api/v5/trade/order", self.base.config.rest_url);

        // Convert our order request to OKX format
        let okx_order = serde_json::json!({
            "instId": format!("{}-{}", order.symbol.base, order.symbol.quote),
            "tdMode": "cash", // Trading mode: cash for spot trading
            "side": match order.side {
                crate::connector::OrderSide::Buy => "buy",
                crate::connector::OrderSide::Sell => "sell",
            },
            "ordType": match order.order_type {
                crate::connector::OrderType::Market => "market",
                crate::connector::OrderType::Limit => "limit",
                _ => "limit", // Default to limit for other types
            },
            "sz": order.quantity.to_string(),
            "px": order.price.map(|p| p.to_string()).unwrap_or_default(),
            "clOrdId": order.client_order_id.as_deref().unwrap_or(""),
        });

        // Make authenticated request to OKX
        let response = self
            .client
            .post(&url)
            .json(&okx_order)
            .send()
            .await
            .map_err(|e| {
                arbitrage_core::ArbitrageError::Network(format!(
                    "OKX place order request failed: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "OKX place order failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        // Parse OKX response
        if let Some(data) = response_json
            .get("data")
            .and_then(|d| d.as_array())
            .and_then(|arr| arr.first())
        {
            let order_id = data
                .get("ordId")
                .and_then(|id| id.as_str())
                .unwrap_or("unknown")
                .to_string();

            let status = match data.get("sCode").and_then(|s| s.as_str()) {
                Some("0") => OrderStatusType::New, // Success
                _ => OrderStatusType::Rejected,
            };

            Ok(OrderResponse {
                order_id,
                client_order_id: order.client_order_id.clone(),
                symbol: order.symbol.clone(),
                side: order.side.clone(),
                order_type: order.order_type.clone(),
                quantity: order.quantity,
                price: order.price,
                status,
                timestamp: chrono::Utc::now(),
            })
        } else {
            Err(arbitrage_core::ArbitrageError::ExchangeConnection(
                "Invalid response format from OKX".to_string(),
            ))
        }
    }

    async fn cancel_order(&self, symbol: &Symbol, order_id: &str) -> Result<CancelResponse> {
        let url = format!("{}/api/v5/trade/cancel-order", self.base.config.rest_url);

        let inst_id = format!("{}-{}", symbol.base, symbol.quote);
        let cancel_request = serde_json::json!({
            "instId": inst_id,
            "ordId": order_id,
        });

        let response = self
            .client
            .post(&url)
            .json(&cancel_request)
            .send()
            .await
            .map_err(|e| {
                arbitrage_core::ArbitrageError::Network(format!(
                    "OKX cancel order request failed: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "OKX cancel order failed with status: {}",
                response.status()
            )));
        }

        Ok(CancelResponse {
            order_id: order_id.to_string(),
            client_order_id: None,
            status: OrderStatusType::Cancelled,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus> {
        use crate::connector::{OrderSide, OrderStatus, OrderStatusType, OrderType};

        let url = format!(
            "{}/api/v5/trade/order?ordId={}",
            self.base.config.rest_url, order_id
        );

        let response = self.client.get(&url).send().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!(
                "OKX get order status request failed: {}",
                e
            ))
        })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "OKX get order status failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        if let Some(data) = response_json
            .get("data")
            .and_then(|d| d.as_array())
            .and_then(|arr| arr.first())
        {
            let symbol_str = data
                .get("instId")
                .and_then(|s| s.as_str())
                .unwrap_or("BTC-USDT");
            let parts: Vec<&str> = symbol_str.split('-').collect();
            let symbol = if parts.len() >= 2 {
                arbitrage_core::types::Symbol::new(parts[0], parts[1])
            } else {
                arbitrage_core::types::Symbol::new("BTC", "USDT")
            };

            let side = match data.get("side").and_then(|s| s.as_str()) {
                Some("buy") => OrderSide::Buy,
                Some("sell") => OrderSide::Sell,
                _ => OrderSide::Buy,
            };

            let order_type = match data.get("ordType").and_then(|t| t.as_str()) {
                Some("market") => OrderType::Market,
                Some("limit") => OrderType::Limit,
                _ => OrderType::Limit,
            };

            let quantity = data
                .get("sz")
                .and_then(|q| q.as_str())
                .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                .unwrap_or_default();

            let price = data
                .get("px")
                .and_then(|p| p.as_str())
                .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

            let filled_quantity = data
                .get("fillSz")
                .and_then(|f| f.as_str())
                .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                .unwrap_or_default();

            let status = match data.get("state").and_then(|s| s.as_str()) {
                Some("live") => OrderStatusType::New,
                Some("partially_filled") => OrderStatusType::PartiallyFilled,
                Some("filled") => OrderStatusType::Filled,
                Some("canceled") => OrderStatusType::Cancelled,
                _ => OrderStatusType::New,
            };

            Ok(OrderStatus {
                order_id: order_id.to_string(),
                client_order_id: data
                    .get("clOrdId")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string()),
                symbol,
                side,
                order_type,
                quantity,
                price,
                filled_quantity,
                remaining_quantity: quantity - filled_quantity,
                average_price: price,
                status,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
        } else {
            Err(arbitrage_core::ArbitrageError::ExchangeConnection(
                "Invalid response format from OKX".to_string(),
            ))
        }
    }

    async fn get_balance(&self) -> Result<crate::connector::Balance> {
        use crate::connector::{AssetBalance, Balance};

        let url = format!("{}/api/v5/account/balance", self.base.config.rest_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!(
                "OKX get balance request failed: {}",
                e
            ))
        })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "OKX get balance failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let mut balances = std::collections::HashMap::with_capacity(64);

        if let Some(data) = response_json
            .get("data")
            .and_then(|d| d.as_array())
            .and_then(|arr| arr.first())
        {
            if let Some(details) = data.get("details").and_then(|d| d.as_array()) {
                for detail in details {
                    if let Some(currency) = detail.get("ccy").and_then(|c| c.as_str()) {
                        let available = detail
                            .get("availBal")
                            .and_then(|a| a.as_str())
                            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                            .unwrap_or_default();

                        let frozen = detail
                            .get("frozenBal")
                            .and_then(|f| f.as_str())
                            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                            .unwrap_or_default();

                        balances.insert(
                            currency.to_string(),
                            AssetBalance {
                                asset: currency.to_string(),
                                free: available,
                                locked: frozen,
                                total: available + frozen,
                            },
                        );
                    }
                }
            }
        }

        Ok(Balance {
            exchange: ExchangeId::OKX,
            balances,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_open_orders(
        &self,
        symbol: Option<&Symbol>,
    ) -> Result<Vec<crate::connector::OrderStatus>> {
        use crate::connector::{OrderSide, OrderStatus, OrderStatusType, OrderType};

        let mut url = format!("{}/api/v5/trade/orders-pending", self.base.config.rest_url);

        if let Some(sym) = symbol {
            url.push_str(&format!("?instId={}-{}", sym.base, sym.quote));
        }

        let response = self.client.get(&url).send().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!(
                "OKX get open orders request failed: {}",
                e
            ))
        })?;

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "OKX get open orders failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let mut orders = Vec::with_capacity(64);

        if let Some(data) = response_json.get("data").and_then(|d| d.as_array()) {
            for order_data in data {
                let order_id = order_data
                    .get("ordId")
                    .and_then(|id| id.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let symbol_str = order_data
                    .get("instId")
                    .and_then(|s| s.as_str())
                    .unwrap_or("BTC-USDT");
                let parts: Vec<&str> = symbol_str.split('-').collect();
                let order_symbol = if parts.len() >= 2 {
                    arbitrage_core::types::Symbol::new(parts[0], parts[1])
                } else {
                    arbitrage_core::types::Symbol::new("BTC", "USDT")
                };

                let side = match order_data.get("side").and_then(|s| s.as_str()) {
                    Some("buy") => OrderSide::Buy,
                    Some("sell") => OrderSide::Sell,
                    _ => OrderSide::Buy,
                };

                let order_type = match order_data.get("ordType").and_then(|t| t.as_str()) {
                    Some("market") => OrderType::Market,
                    Some("limit") => OrderType::Limit,
                    _ => OrderType::Limit,
                };

                let quantity = order_data
                    .get("sz")
                    .and_then(|q| q.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                let price = order_data
                    .get("px")
                    .and_then(|p| p.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok());

                let filled_quantity = order_data
                    .get("fillSz")
                    .and_then(|f| f.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                orders.push(OrderStatus {
                    order_id,
                    client_order_id: order_data
                        .get("clOrdId")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string()),
                    symbol: order_symbol,
                    side,
                    order_type,
                    quantity,
                    price,
                    filled_quantity,
                    remaining_quantity: quantity - filled_quantity,
                    average_price: price,
                    status: OrderStatusType::New, // Pending orders are "New"
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }

        Ok(orders)
    }
}

impl OKXConnector {
    pub fn parse_order_book(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<OrderBook> {
        let asks_data = data["asks"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing asks data".to_string())
        })?;
        let bids_data = data["bids"].as_array().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing bids data".to_string())
        })?;

        let mut asks = Vec::with_capacity(self.base.config.order_book_depth as usize);
        for ask in asks_data
            .iter()
            .take(self.base.config.order_book_depth as usize)
        {
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price = parse_decimal(&ask_array[0])?;
                    let quantity = parse_decimal(&ask_array[1])?;
                    asks.push(OrderBookLevel { price, quantity });
                }
            }
        }

        let mut bids = Vec::with_capacity(self.base.config.order_book_depth as usize);
        for bid in bids_data
            .iter()
            .take(self.base.config.order_book_depth as usize)
        {
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price = parse_decimal(&bid_array[0])?;
                    let quantity = parse_decimal(&bid_array[1])?;
                    bids.push(OrderBookLevel { price, quantity });
                }
            }
        }

        let timestamp = if let Some(ts_str) = data["ts"].as_str() {
            if let Ok(ts) = ts_str.parse::<i64>() {
                chrono::DateTime::from_timestamp_millis(ts).unwrap_or_else(chrono::Utc::now)
            } else {
                chrono::Utc::now()
            }
        } else {
            chrono::Utc::now()
        };

        Ok(OrderBook {
            exchange: ExchangeId::OKX,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp,
            sequence: None,
        })
    }

    pub fn parse_ticker(&self, data: &serde_json::Value, symbol: &Symbol) -> Result<TickerData> {
        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::OKX,
            last_price: parse_decimal(&data["last"])?,
            bid_price: parse_decimal(&data["bidPx"])?,
            ask_price: parse_decimal(&data["askPx"])?,
            volume_24h: parse_decimal(&data["vol24h"]).unwrap_or_default(),
            price_change_24h: parse_decimal(&data["sodUtc0"]).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }

    pub fn parse_funding_rate(
        &self,
        data: &serde_json::Value,
        symbol: &Symbol,
    ) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["fundingRate"])?;
        let funding_time = parse_timestamp(&data["fundingTime"])?;
        let predicted_rate = parse_decimal(&data["nextFundingRate"]).ok();

        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::OKX,
            funding_rate,
            predicted_rate,
            funding_time,
            timestamp: chrono::Utc::now(),
        })
    }
}
