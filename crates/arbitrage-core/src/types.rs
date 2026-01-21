use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Supported cryptocurrency exchanges - PROFESSIONAL ARBITRAGE GRADE
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExchangeId {
    // Tier 1: High-liquidity, low-latency exchanges
    OKX,   // Spot + Futures + Options
    ByBit, // Spot + Futures + Options
    MEXC,  // Spot + Futures

    // Tier 2: Solid liquidity exchanges
    GateIo,   // Spot + Futures
    Bitstamp, // Spot (fiat pairs)
    Kraken,   // Spot + Futures (fiat pairs)

    // Legacy (keeping for compatibility)
    HTX, // Former Huobi
    BingX,
    Hyperliquid,
    KuCoin,
    Bitget,
    Binance,
    Coinbase,
}

impl std::fmt::Display for ExchangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExchangeId::OKX => write!(f, "okx"),
            ExchangeId::ByBit => write!(f, "bybit"),
            ExchangeId::MEXC => write!(f, "mexc"),
            ExchangeId::GateIo => write!(f, "gateio"),
            ExchangeId::Bitstamp => write!(f, "bitstamp"),
            ExchangeId::Kraken => write!(f, "kraken"),
            ExchangeId::HTX => write!(f, "htx"),
            ExchangeId::BingX => write!(f, "bingx"),
            ExchangeId::Hyperliquid => write!(f, "hyperliquid"),
            ExchangeId::KuCoin => write!(f, "kucoin"),
            ExchangeId::Bitget => write!(f, "bitget"),
            ExchangeId::Binance => write!(f, "binance"),
            ExchangeId::Coinbase => write!(f, "coinbase"),
        }
    }
}

impl std::str::FromStr for ExchangeId {
    type Err = crate::ArbitrageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "okx" => Ok(ExchangeId::OKX),
            "bybit" => Ok(ExchangeId::ByBit),
            "mexc" => Ok(ExchangeId::MEXC),
            "gateio" | "gate.io" => Ok(ExchangeId::GateIo),
            "bitstamp" => Ok(ExchangeId::Bitstamp),
            "kraken" => Ok(ExchangeId::Kraken),
            "htx" => Ok(ExchangeId::HTX),
            "bingx" => Ok(ExchangeId::BingX),
            "hyperliquid" => Ok(ExchangeId::Hyperliquid),
            "kucoin" => Ok(ExchangeId::KuCoin),
            "bitget" => Ok(ExchangeId::Bitget),
            "binance" => Ok(ExchangeId::Binance),
            "coinbase" => Ok(ExchangeId::Coinbase),
            _ => Err(crate::ArbitrageError::Validation(format!(
                "Unknown exchange: {}",
                s
            ))),
        }
    }
}

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "Buy"),
            Side::Sell => write!(f, "Sell"),
        }
    }
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopLimit,
}

/// Time in force for orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good Till Cancelled
    GTC,
    /// Immediate Or Cancel
    IOC,
    /// Fill Or Kill
    FOK,
    /// Good Till Date
    GTD,
}

/// Trading symbol with base and quote currencies
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    pub base: String,
    pub quote: String,
}

impl Symbol {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }

    pub fn from_pair(pair: &str) -> Option<Self> {
        let parts: Vec<&str> = pair.split('/').collect();
        if parts.len() == 2 {
            Some(Self::new(parts[0], parts[1]))
        } else {
            None
        }
    }

    pub fn to_pair(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// Single price/quantity level in an order book
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub quantity: Decimal,
}

impl OrderBookLevel {
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self { price, quantity }
    }
}

/// Order book snapshot with bids and asks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub exchange: ExchangeId,
    pub symbol: Symbol,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: DateTime<Utc>,
    pub sequence: Option<u64>,
}

impl OrderBook {
    pub fn new(
        exchange: ExchangeId,
        symbol: Symbol,
        bids: Vec<OrderBookLevel>,
        asks: Vec<OrderBookLevel>,
    ) -> Self {
        Self {
            exchange,
            symbol,
            bids,
            asks,
            timestamp: Utc::now(),
            sequence: None,
        }
    }

    pub fn best_bid(&self) -> Option<&OrderBookLevel> {
        self.bids.first()
    }

    pub fn best_ask(&self) -> Option<&OrderBookLevel> {
        self.asks.first()
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some((ask.price + bid.price) / Decimal::from(2)),
            _ => None,
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.bids.is_empty() || self.asks.is_empty() {
            return false;
        }

        if let (Some(bid), Some(ask)) = (self.best_bid(), self.best_ask()) {
            if bid.price >= ask.price {
                return false;
            }
        } else {
            return false;
        }

        if !self.bids.windows(2).all(|w| w[0].price >= w[1].price) {
            return false;
        }

        if !self.asks.windows(2).all(|w| w[0].price <= w[1].price) {
            return false;
        }

        true
    }

    /// Calculate VWAP for buying a given quantity
    pub fn vwap_buy(&self, target_quantity: Decimal) -> Option<VwapResult> {
        if target_quantity <= Decimal::ZERO || self.asks.is_empty() {
            return None;
        }

        let mut remaining_qty = target_quantity;
        let mut total_cost = Decimal::ZERO;
        let mut filled_qty = Decimal::ZERO;

        for level in &self.asks {
            if remaining_qty <= Decimal::ZERO {
                break;
            }

            let qty_from_level = remaining_qty.min(level.quantity);
            total_cost += qty_from_level * level.price;
            filled_qty += qty_from_level;
            remaining_qty -= qty_from_level;
        }

        if filled_qty > Decimal::ZERO {
            let vwap = total_cost / filled_qty;
            Some(VwapResult {
                vwap_price: vwap,
                filled_quantity: filled_qty,
                total_cost,
                is_fully_filled: remaining_qty <= Decimal::ZERO,
                slippage_bps: self.calculate_slippage_bps(self.best_ask()?.price, vwap),
            })
        } else {
            None
        }
    }

    /// Calculate VWAP for selling a given quantity
    pub fn vwap_sell(&self, target_quantity: Decimal) -> Option<VwapResult> {
        if target_quantity <= Decimal::ZERO || self.bids.is_empty() {
            return None;
        }

        let mut remaining_qty = target_quantity;
        let mut total_revenue = Decimal::ZERO;
        let mut filled_qty = Decimal::ZERO;

        for level in &self.bids {
            if remaining_qty <= Decimal::ZERO {
                break;
            }

            let qty_from_level = remaining_qty.min(level.quantity);
            total_revenue += qty_from_level * level.price;
            filled_qty += qty_from_level;
            remaining_qty -= qty_from_level;
        }

        if filled_qty > Decimal::ZERO {
            let vwap = total_revenue / filled_qty;
            Some(VwapResult {
                vwap_price: vwap,
                filled_quantity: filled_qty,
                total_cost: total_revenue,
                is_fully_filled: remaining_qty <= Decimal::ZERO,
                slippage_bps: self.calculate_slippage_bps(self.best_bid()?.price, vwap),
            })
        } else {
            None
        }
    }

    /// Calculate available liquidity up to a price level
    pub fn liquidity_at_price(&self, price: Decimal, is_buy: bool) -> Decimal {
        if is_buy {
            self.asks
                .iter()
                .take_while(|level| level.price <= price)
                .map(|level| level.quantity)
                .sum()
        } else {
            self.bids
                .iter()
                .take_while(|level| level.price >= price)
                .map(|level| level.quantity)
                .sum()
        }
    }

    /// Calculate slippage in basis points
    fn calculate_slippage_bps(&self, best_price: Decimal, vwap_price: Decimal) -> i32 {
        if best_price.is_zero() {
            return 0;
        }

        let slippage_ratio = (vwap_price - best_price).abs() / best_price;
        (slippage_ratio * Decimal::from(10000))
            .to_i32()
            .unwrap_or(0)
    }
}

/// VWAP calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapResult {
    pub vwap_price: Decimal,
    pub filled_quantity: Decimal,
    pub total_cost: Decimal,
    pub is_fully_filled: bool,
    pub slippage_bps: i32,
}

/// Arbitrage signal representing a trading opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: Uuid,
    pub symbol: Symbol,
    pub buy_exchange: ExchangeId,
    pub sell_exchange: ExchangeId,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub gross_profit_percent: Decimal,
    pub net_profit_percent: Decimal,
    pub net_profit_absolute: Decimal,
    pub confidence: Decimal,
    pub recommended_size: Decimal,
    pub max_size: Decimal,
    pub expected_slippage: Decimal,
    pub estimated_execution_time_ms: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Signal {
    pub fn new(
        symbol: Symbol,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
        buy_price: Decimal,
        sell_price: Decimal,
        created_at: DateTime<Utc>,
    ) -> Self {
        if created_at > Utc::now() {
            panic!("Signal created_at cannot be in the future");
        }
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            symbol,
            buy_exchange,
            sell_exchange,
            buy_price,
            sell_price,
            gross_profit_percent: Decimal::ZERO,
            net_profit_percent: Decimal::ZERO,
            net_profit_absolute: Decimal::ZERO,
            confidence: Decimal::ZERO,
            recommended_size: Decimal::ZERO,
            max_size: Decimal::ZERO,
            expected_slippage: Decimal::ZERO,
            estimated_execution_time_ms: 0,
            created_at,
            expires_at: now + chrono::Duration::minutes(5), // Default 5 min expiry
            metadata: HashMap::new(),
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn age_seconds(&self) -> i64 {
        let duration = Utc::now() - self.created_at;
        if duration.num_seconds() < 0 {
            0
        } else {
            duration.num_seconds()
        }
    }
}

/// Order instruction for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub client_order_id: String,
    pub exchange: ExchangeId,
    pub symbol: Symbol,
    pub side: Side,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub expected_fee: Decimal,
    pub created_at: DateTime<Utc>,
}

impl Order {
    pub fn new(
        exchange: ExchangeId,
        symbol: Symbol,
        side: Side,
        order_type: OrderType,
        quantity: Decimal,
        price: Option<Decimal>,
    ) -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            client_order_id: format!("arb_{}", id.simple()),
            exchange,
            symbol,
            side,
            order_type,
            quantity,
            price,
            time_in_force: TimeInForce::IOC,
            expected_fee: Decimal::ZERO,
            created_at: Utc::now(),
        }
    }
}

/// Execution instruction containing buy and sell orders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInstruction {
    pub id: Uuid,
    pub signal_id: Uuid,
    pub buy_order: Order,
    pub sell_order: Order,
    pub expected_profit: Decimal,
    pub worst_case_profit: Decimal,
    pub total_fees: Decimal,
    pub slippage_buffer: Decimal,
    pub created_at: DateTime<Utc>,
    pub validation_errors: Vec<String>,
}

impl ExecutionInstruction {
    pub fn new(signal_id: Uuid, buy_order: Order, sell_order: Order) -> Self {
        Self {
            id: Uuid::new_v4(),
            signal_id,
            buy_order,
            sell_order,
            expected_profit: Decimal::ZERO,
            worst_case_profit: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            slippage_buffer: Decimal::ZERO,
            created_at: Utc::now(),
            validation_errors: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.validation_errors.is_empty()
    }
}

/// Exchange fee schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeSchedule {
    pub exchange: ExchangeId,
    pub maker_fee: Decimal,
    pub taker_fee: Decimal,
    pub tier: Option<String>,
}

impl FeeSchedule {
    pub fn new(exchange: ExchangeId, maker_fee: Decimal, taker_fee: Decimal) -> Self {
        if maker_fee < Decimal::ZERO {
            panic!("maker_fee cannot be negative: {}", maker_fee);
        }
        if taker_fee < Decimal::ZERO {
            panic!("taker_fee cannot be negative: {}", taker_fee);
        }
        Self {
            exchange,
            maker_fee,
            taker_fee,
            tier: None,
        }
    }
}

/// Exchange connection status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

/// Exchange connection info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeStatus {
    pub exchange: ExchangeId,
    pub status: ConnectionStatus,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub uptime_percent: Decimal,
    pub error_count: u64,
    pub subscribed_symbols: Vec<Symbol>,
}

impl ExchangeStatus {
    pub fn new(exchange: ExchangeId) -> Self {
        Self {
            exchange,
            status: ConnectionStatus::Disconnected,
            last_heartbeat: None,
            uptime_percent: Decimal::ZERO,
            error_count: 0,
            subscribed_symbols: Vec::new(),
        }
    }
}

/// Trait for converting Decimal values to basis points (bps)
pub trait ToBps {
    fn to_bps(&self) -> Option<i32>;
    fn to_bps_or_zero(&self) -> i32;
}

impl ToBps for Decimal {
    fn to_bps(&self) -> Option<i32> {
        (self * Decimal::from(10000)).to_i32()
    }

    fn to_bps_or_zero(&self) -> i32 {
        self.to_bps().unwrap_or(0)
    }
}

/// Trait for calculating mid price from market data
pub trait MidPrice {
    fn mid_price(&self) -> Option<Decimal>;
}

impl MidPrice for crate::strategies::Ticker {
    fn mid_price(&self) -> Option<Decimal> {
        if self.bid.is_zero() && self.ask.is_zero() {
            None
        } else {
            Some((self.bid + self.ask) / Decimal::from(2))
        }
    }
}

impl MidPrice for OrderBook {
    fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some((ask.price + bid.price) / Decimal::from(2)),
            _ => None,
        }
    }
}

/// Exchange constants for convenience
pub mod exchange_constants {
    use super::ExchangeId;

    pub const ALL_EXCHANGES: [ExchangeId; 12] = [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Bitstamp,
        ExchangeId::Kraken,
        ExchangeId::HTX,
        ExchangeId::BingX,
        ExchangeId::Hyperliquid,
        ExchangeId::KuCoin,
        ExchangeId::Bitget,
        ExchangeId::Binance,
    ];

    pub const PERPETUAL_EXCHANGES: [ExchangeId; 3] =
        [ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC];

    pub const SPOT_EXCHANGES: [ExchangeId; 6] = [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Bitstamp,
        ExchangeId::Kraken,
    ];
}

/// Calculate profit in basis points from buy and sell prices
pub fn calculate_profit_bps(buy_price: Decimal, sell_price: Decimal) -> Option<i32> {
    if buy_price.is_zero() {
        return None;
    }
    let profit_ratio = (sell_price - buy_price) / buy_price;
    (profit_ratio * Decimal::from(10000)).to_i32()
}
