use crate::connector::{
    CancelResponse, ConnectorConfig, ConnectorStats, ExchangeConnector, FundingRate, HealthStatus,
    OrderSide, OrderStatus, OrderStatusType, OrderType, TickerData,
};
use crate::connector_trait::ConnectorBase;
use crate::events::{ConnectionEvent, MarketDataEvent};
use crate::utils::{format_symbol, parse_decimal, parse_symbol, SymbolFormat};

use arbitrage_core::{
    types::{ConnectionStatus, ExchangeId, OrderBook, OrderBookLevel, Symbol},
    Result,
};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};

/// MEXC WebSocket subscription message - used for order book subscriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MexcSubscription {
    method: String,
    params: Vec<String>,
}

#[derive(Clone)]
pub struct MEXCConnector {
    pub base: ConnectorBase,
    client: Client,
    subscribed_symbols: Arc<RwLock<Vec<Symbol>>>,
}

impl Default for MEXCConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl MEXCConnector {
    pub fn new() -> Self {
        let config = ConnectorConfig {
            exchange_id: ExchangeId::MEXC,
            ws_url: "wss://wbs.mexc.com/ws".to_string(),
            rest_url: "https://api.mexc.com".to_string(),
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
        }
    }

    pub fn symbol_to_mexc(&self, symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    pub fn symbol_from_mexc(&self, mexc_symbol: &str) -> Result<Symbol> {
        parse_symbol(mexc_symbol, SymbolFormat::NoSeparator)
    }
}

#[async_trait]
impl ExchangeConnector for MEXCConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::MEXC
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
        let mexc_symbol = self.symbol_to_mexc(symbol);
        let url = format!(
            "{}/api/v3/depth?symbol={}&limit={}",
            self.base.config.rest_url, mexc_symbol, self.base.config.order_book_depth
        );

        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.get(&url).send(),
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC fetch_order_book failed: operation=fetch_order_book, exchange=MEXC, symbol={}, error={}",
                    mexc_symbol, e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    format!("MEXC fetch_order_book timed out after 5s: operation=fetch_order_book, exchange=MEXC, symbol={}", mexc_symbol)
                ));
            }
        };
        let order_book_json: Value = response.json().await?;

        self.parse_order_book(&order_book_json, symbol)
    }

    async fn fetch_symbols(&self) -> Result<Vec<Symbol>> {
        let url = format!("{}/api/v3/exchangeInfo", self.base.config.rest_url);
        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.get(&url).send(),
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC fetch_symbols failed: operation=fetch_symbols, exchange=MEXC, error={}",
                    e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    "MEXC fetch_symbols timed out after 5s: operation=fetch_symbols, exchange=MEXC"
                        .to_string(),
                ));
            }
        };
        let exchange_info_json: Value = response.json().await?;

        let mut symbols = Vec::with_capacity(512);
        if let Some(symbols_array) = exchange_info_json["symbols"].as_array() {
            for item in symbols_array {
                if let Some(symbol_str) = item["symbol"].as_str() {
                    let status = item["status"].as_str().unwrap_or("");
                    let is_spot_allowed = item["isSpotTradingAllowed"].as_bool().unwrap_or(false);

                    if (status == "1" || status == "TRADING") && is_spot_allowed {
                        if let Ok(symbol) = self.symbol_from_mexc(symbol_str) {
                            symbols.push(symbol);
                        }
                    }
                }
            }
        }
        Ok(symbols)
    }

    async fn fetch_tickers(&self, symbols: &[Symbol]) -> Result<HashMap<Symbol, TickerData>> {
        let url = format!("{}/api/v3/ticker/bookTicker", self.base.config.rest_url);
        let symbols_str: String = symbols
            .iter()
            .map(|s| format!("{}/{}", s.base, s.quote))
            .collect::<Vec<_>>()
            .join(",");
        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.get(&url).send(),
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC fetch_tickers failed: operation=fetch_tickers, exchange=MEXC, symbols={}, error={}",
                    symbols_str, e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    format!("MEXC fetch_tickers timed out after 5s: operation=fetch_tickers, exchange=MEXC, symbols={}", symbols_str)
                ));
            }
        };
        let tickers_json: Value = response.json().await?;

        let mut tickers = HashMap::with_capacity(symbols.len());
        if let Some(ticker_array) = tickers_json.as_array() {
            for ticker_data in ticker_array {
                if let Some(symbol_str) = ticker_data["symbol"].as_str() {
                    if let Ok(symbol) = self.symbol_from_mexc(symbol_str) {
                        if symbols.contains(&symbol) {
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

    async fn fetch_funding_rates(
        &self,
        symbols: &[Symbol],
    ) -> Result<HashMap<Symbol, FundingRate>> {
        let mut funding_rates = HashMap::with_capacity(symbols.len());
        for symbol in symbols {
            let mexc_symbol = format!("{}_USDT", symbol.base.to_uppercase());
            let url = format!(
                "{}/api/v1/contract/funding_rate/{}",
                self.base.config.rest_url, mexc_symbol
            );

            let response = match tokio::time::timeout(
                Duration::from_secs(5),
                self.client.get(&url).send(),
            )
            .await
            {
                Ok(Ok(resp)) => resp,
                Ok(Err(e)) => {
                    warn!("MEXC funding rate request failed: operation=fetch_funding_rates, exchange=MEXC, symbol={}, error={}", mexc_symbol, e);
                    continue;
                }
                Err(_) => {
                    warn!("MEXC funding rate request timed out: operation=fetch_funding_rates, exchange=MEXC, symbol={}", mexc_symbol);
                    continue;
                }
            };

            let data = match response.json::<Value>().await {
                Ok(data) => data,
                Err(e) => {
                    warn!(
                        "Failed to parse funding rate response for {}: {}",
                        mexc_symbol, e
                    );
                    continue;
                }
            };

            match self.parse_funding_rate(&data, symbol) {
                Ok(funding_rate) => {
                    funding_rates.insert(symbol.clone(), funding_rate);
                }
                Err(e) => {
                    warn!("Failed to parse funding rate for {}: {}", mexc_symbol, e);
                    continue;
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
        // Store symbols for reconnection
        let mut subscribed = self.subscribed_symbols.write().await;
        for symbol in symbols {
            if !subscribed.contains(symbol) {
                subscribed.push(symbol.clone());
            }
        }
        Ok(())
    }

    async fn unsubscribe_symbols(&mut self, symbols: &[Symbol]) -> Result<()> {
        // Remove symbols from subscription list
        let mut subscribed = self.subscribed_symbols.write().await;
        subscribed.retain(|s| !symbols.contains(s));
        Ok(())
    }

    async fn subscribe_tickers(&mut self, _symbols: &[Symbol]) -> Result<()> {
        // MEXC tickers are included in order book subscriptions
        Ok(())
    }

    async fn subscribe_order_books(&mut self, symbols: &[Symbol]) -> Result<()> {
        info!(
            "Subscribing to MEXC order books for {} symbols",
            symbols.len()
        );

        // Add symbols to subscription list
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

        let url = format!("{}/api/v3/order", self.base.config.rest_url);

        let mexc_order = serde_json::json!({
            "symbol": format!("{}{}", order.symbol.base, order.symbol.quote),
            "side": match order.side {
                crate::connector::OrderSide::Buy => "BUY",
                crate::connector::OrderSide::Sell => "SELL",
            },
            "type": match order.order_type {
                crate::connector::OrderType::Market => "MARKET",
                crate::connector::OrderType::Limit => "LIMIT",
                _ => "LIMIT",
            },
            "quantity": order.quantity.to_string(),
            "price": order.price.map(|p| p.to_string()).unwrap_or_default(),
            "newClientOrderId": order.client_order_id.as_deref().unwrap_or(""),
        });

        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.post(&url).json(&mexc_order).send(),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC place_order failed: operation=place_order, exchange=MEXC, symbol={}/{}, side={:?}, type={:?}, quantity={}, error={}",
                    order.symbol.base, order.symbol.quote, order.side, order.order_type, order.quantity, e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    format!("MEXC place_order timed out after 5s: operation=place_order, exchange=MEXC, symbol={}/{}, side={:?}, type={:?}, quantity={}",
                        order.symbol.base, order.symbol.quote, order.side, order.order_type, order.quantity)
                ));
            }
        };

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "MEXC place order failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let order_id = response_json
            .get("orderId")
            .and_then(|id| id.as_u64())
            .map(|id| id.to_string())
            .unwrap_or("unknown".to_string());

        Ok(OrderResponse {
            order_id,
            client_order_id: order.client_order_id.clone(),
            symbol: order.symbol.clone(),
            side: order.side.clone(),
            order_type: order.order_type.clone(),
            quantity: order.quantity,
            price: order.price,
            status: OrderStatusType::New,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn cancel_order(&self, _symbol: &Symbol, order_id: &str) -> Result<CancelResponse> {
        let url = format!("{}/api/v3/order", self.base.config.rest_url);

        let cancel_request = serde_json::json!({
            "orderId": order_id,
        });

        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.delete(&url).json(&cancel_request).send(),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC cancel_order failed: operation=cancel_order, exchange=MEXC, order_id={}, error={}",
                    order_id, e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    format!("MEXC cancel_order timed out after 5s: operation=cancel_order, exchange=MEXC, order_id={}", order_id)
                ));
            }
        };

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "MEXC cancel order failed with status: {}",
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
        let url = format!(
            "{}/api/v3/order?orderId={}",
            self.base.config.rest_url, order_id
        );

        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.get(&url).send(),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC get_order_status failed: operation=get_order_status, exchange=MEXC, order_id={}, error={}",
                    order_id, e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    format!("MEXC get_order_status timed out after 5s: operation=get_order_status, exchange=MEXC, order_id={}", order_id)
                ));
            }
        };

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "MEXC get order status failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let symbol_str = response_json
            .get("symbol")
            .and_then(|s| s.as_str())
            .unwrap_or("BTCUSDT");
        let symbol = if let Some(base) = symbol_str.strip_suffix("USDT") {
            arbitrage_core::types::Symbol::new(base, "USDT")
        } else {
            arbitrage_core::types::Symbol::new("BTC", "USDT")
        };

        let side = match response_json.get("side").and_then(|s| s.as_str()) {
            Some("BUY") => OrderSide::Buy,
            Some("SELL") => OrderSide::Sell,
            _ => OrderSide::Buy,
        };

        let quantity = response_json
            .get("origQty")
            .and_then(|q| q.as_str())
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
            .unwrap_or_default();

        let filled_quantity = response_json
            .get("executedQty")
            .and_then(|f| f.as_str())
            .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
            .unwrap_or_default();

        let status = match response_json.get("status").and_then(|s| s.as_str()) {
            Some("NEW") => OrderStatusType::New,
            Some("PARTIALLY_FILLED") => OrderStatusType::PartiallyFilled,
            Some("FILLED") => OrderStatusType::Filled,
            Some("CANCELED") => OrderStatusType::Cancelled,
            _ => OrderStatusType::New,
        };

        Ok(OrderStatus {
            order_id: order_id.to_string(),
            client_order_id: response_json
                .get("clientOrderId")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            symbol,
            side,
            order_type: OrderType::Limit,
            quantity,
            price: response_json
                .get("price")
                .and_then(|p| p.as_str())
                .and_then(|s| rust_decimal::Decimal::from_str(s).ok()),
            filled_quantity,
            remaining_quantity: quantity - filled_quantity,
            average_price: None,
            status,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    async fn get_balance(&self) -> Result<crate::connector::Balance> {
        use crate::connector::{AssetBalance, Balance};

        let url = format!("{}/api/v3/account", self.base.config.rest_url);

        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.get(&url).send(),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC get_balance failed: operation=get_balance, exchange=MEXC, error={}",
                    e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    "MEXC get_balance timed out after 5s: operation=get_balance, exchange=MEXC"
                        .to_string(),
                ));
            }
        };

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "MEXC get balance failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let mut balances = std::collections::HashMap::with_capacity(64);

        if let Some(balance_array) = response_json.get("balances").and_then(|b| b.as_array()) {
            for balance in balance_array {
                if let Some(asset) = balance.get("asset").and_then(|a| a.as_str()) {
                    let free = balance
                        .get("free")
                        .and_then(|f| f.as_str())
                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                        .unwrap_or_default();

                    let locked = balance
                        .get("locked")
                        .and_then(|l| l.as_str())
                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                        .unwrap_or_default();

                    balances.insert(
                        asset.to_string(),
                        AssetBalance {
                            asset: asset.to_string(),
                            free,
                            locked,
                            total: free + locked,
                        },
                    );
                }
            }
        }

        Ok(Balance {
            exchange: ExchangeId::MEXC,
            balances,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn get_open_orders(
        &self,
        symbol: Option<&Symbol>,
    ) -> Result<Vec<crate::connector::OrderStatus>> {
        use crate::connector::{OrderSide, OrderStatus, OrderStatusType, OrderType};

        let mut url = format!("{}/api/v3/openOrders", self.base.config.rest_url);

        if let Some(sym) = symbol {
            url.push_str(&format!("?symbol={}{}", sym.base, sym.quote));
        }

        let symbol_filter = symbol
            .map(|s| format!("{}/{}", s.base, s.quote))
            .unwrap_or_else(|| "all".to_string());
        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.get(&url).send(),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                return Err(arbitrage_core::ArbitrageError::Network(format!(
                    "MEXC get_open_orders failed: operation=get_open_orders, exchange=MEXC, symbol={}, error={}",
                    symbol_filter, e
                )));
            }
            Err(_) => {
                return Err(arbitrage_core::ArbitrageError::Timeout(
                    format!("MEXC get_open_orders timed out after 5s: operation=get_open_orders, exchange=MEXC, symbol={}", symbol_filter)
                ));
            }
        };

        if !response.status().is_success() {
            return Err(arbitrage_core::ArbitrageError::ExchangeConnection(format!(
                "MEXC get open orders failed with status: {}",
                response.status()
            )));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            arbitrage_core::ArbitrageError::Network(format!("Failed to parse JSON: {}", e))
        })?;

        let mut orders = Vec::with_capacity(64);

        if let Some(order_array) = response_json.as_array() {
            for order_data in order_array {
                let order_id = order_data
                    .get("orderId")
                    .and_then(|id| id.as_u64())
                    .map(|id| id.to_string())
                    .unwrap_or("unknown".to_string());

                let symbol_str = order_data
                    .get("symbol")
                    .and_then(|s| s.as_str())
                    .unwrap_or("BTCUSDT");
                let order_symbol = if let Some(base) = symbol_str.strip_suffix("USDT") {
                    arbitrage_core::types::Symbol::new(base, "USDT")
                } else {
                    arbitrage_core::types::Symbol::new("BTC", "USDT")
                };

                let side = match order_data.get("side").and_then(|s| s.as_str()) {
                    Some("BUY") => OrderSide::Buy,
                    Some("SELL") => OrderSide::Sell,
                    _ => OrderSide::Buy,
                };

                let quantity = order_data
                    .get("origQty")
                    .and_then(|q| q.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                let filled_quantity = order_data
                    .get("executedQty")
                    .and_then(|f| f.as_str())
                    .and_then(|s| rust_decimal::Decimal::from_str(s).ok())
                    .unwrap_or_default();

                orders.push(OrderStatus {
                    order_id,
                    client_order_id: order_data
                        .get("clientOrderId")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string()),
                    symbol: order_symbol,
                    side,
                    order_type: OrderType::Limit,
                    quantity,
                    price: order_data
                        .get("price")
                        .and_then(|p| p.as_str())
                        .and_then(|s| rust_decimal::Decimal::from_str(s).ok()),
                    filled_quantity,
                    remaining_quantity: quantity - filled_quantity,
                    average_price: None,
                    status: OrderStatusType::New,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }

        Ok(orders)
    }
}

impl MEXCConnector {
    /// Handle incoming text messages for MEXC
    fn handle_text_message(
        text: &str,
        event_sender: &broadcast::Sender<ConnectionEvent>,
    ) -> Result<()> {
        debug!("Received MEXC message: {}", text);

        // Try to parse as JSON
        let ws_message_json: Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        // Handle PONG response
        if ws_message_json.get("msg").and_then(|m| m.as_str()) == Some("PONG") {
            debug!("MEXC PONG received");
            return Ok(());
        }

        // Handle subscription response
        if let Some(code) = ws_message_json.get("code").and_then(|c| c.as_i64()) {
            if code == 0 {
                debug!("MEXC subscription successful");
            } else {
                warn!("MEXC subscription error: code {}", code);
            }
            return Ok(());
        }

        // Handle market data
        if let Some(channel) = ws_message_json.get("c").and_then(|c| c.as_str()) {
            if channel.contains("bookTicker") {
                if let Ok(order_book) = Self::parse_bookticker_message(&ws_message_json) {
                    let event = ConnectionEvent::MarketData(MarketDataEvent::OrderBook {
                        exchange: ExchangeId::MEXC,
                        order_book,
                        timestamp: chrono::Utc::now(),
                    });
                    let _ = event_sender.send(event);
                }
            }
        }

        Ok(())
    }

    /// Parse MEXC book ticker message into OrderBook
    fn parse_bookticker_message(data: &Value) -> Result<OrderBook> {
        let d = data.get("d").ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing data".to_string())
        })?;

        let symbol_str = data.get("s").and_then(|s| s.as_str()).ok_or_else(|| {
            arbitrage_core::ArbitrageError::ExchangeConnection("Missing symbol".to_string())
        })?;

        let symbol = Self::symbol_from_mexc_static(symbol_str)?;

        let bid_price = parse_decimal(&d["b"])?;
        let bid_qty = parse_decimal(&d["B"])?;
        let ask_price = parse_decimal(&d["a"])?;
        let ask_qty = parse_decimal(&d["A"])?;

        let timestamp = if let Some(t) = data.get("t").and_then(|t| t.as_i64()) {
            chrono::DateTime::from_timestamp_millis(t).unwrap_or_else(chrono::Utc::now)
        } else {
            chrono::Utc::now()
        };

        Ok(OrderBook {
            exchange: ExchangeId::MEXC,
            symbol,
            bids: vec![OrderBookLevel {
                price: bid_price,
                quantity: bid_qty,
            }],
            asks: vec![OrderBookLevel {
                price: ask_price,
                quantity: ask_qty,
            }],
            timestamp,
            sequence: None,
        })
    }

    /// Static version of symbol conversion for use in async contexts
    fn symbol_to_mexc_static(symbol: &Symbol) -> String {
        format_symbol(symbol, SymbolFormat::NoSeparator)
    }

    /// Static version of symbol parsing for use in async contexts
    fn symbol_from_mexc_static(mexc_symbol: &str) -> Result<Symbol> {
        parse_symbol(mexc_symbol, SymbolFormat::NoSeparator)
    }

    pub fn parse_order_book(&self, data: &Value, symbol: &Symbol) -> Result<OrderBook> {
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

        Ok(OrderBook {
            exchange: ExchangeId::MEXC,
            symbol: symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: None,
        })
    }

    pub fn parse_ticker(&self, data: &Value, symbol: &Symbol) -> Result<TickerData> {
        Ok(TickerData {
            symbol: symbol.clone(),
            exchange: ExchangeId::MEXC,
            last_price: parse_decimal(&data["price"]).unwrap_or_default(),
            bid_price: parse_decimal(&data["bidPrice"])?,
            ask_price: parse_decimal(&data["askPrice"])?,
            volume_24h: parse_decimal(&data["bidQty"]).unwrap_or_default(),
            price_change_24h: rust_decimal::Decimal::ZERO,
            timestamp: chrono::Utc::now(),
        })
    }

    pub fn parse_funding_rate(&self, data: &Value, symbol: &Symbol) -> Result<FundingRate> {
        let funding_rate = parse_decimal(&data["data"]["fundingRate"])?;

        Ok(FundingRate {
            symbol: symbol.clone(),
            exchange: ExchangeId::MEXC,
            funding_rate,
            predicted_rate: None,
            funding_time: chrono::Utc::now(),
            timestamp: chrono::Utc::now(),
        })
    }
}
