//! Core types and data structures for the arbitrage trading engine.
//!
//! This module defines the fundamental data types used throughout the arbitrage system,
//! including order books, trading signals, exchange identifiers, and related utilities.
//!
//! # Key Types
//!
//! - [`OrderBook`] - Represents a snapshot of market depth with bids and asks
//! - [`Signal`] - Generated arbitrage opportunity with pricing and metadata
//! - [`ExchangeId`] - Supported cryptocurrency exchanges
//! - [`Symbol`] - Trading pair identifier (e.g., BTC/USDT)
//!
//! # Example
//!
//! ```rust
//! use arbitrage_core::types::{ExchangeId, OrderBook, Symbol};
//!
//! let symbol = Symbol::new("BTC", "USDT");
//! assert_eq!(symbol.to_pair(), "BTC/USDT");
//!
//! let exchange = ExchangeId::Binance;
//! assert_eq!(exchange.to_string(), "binance");
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Supported cryptocurrency exchanges for professional arbitrage trading.
///
/// Exchanges are organized by tier based on liquidity characteristics:
/// - **Tier 1**: High-liquidity, low-latency exchanges ideal for arbitrage
/// - **Tier 2**: Solid liquidity exchanges with good connectivity
/// - **Legacy**: Maintained for backward compatibility
///
/// # Example
///
/// ```rust
/// use arbitrage_core::types::ExchangeId;
///
/// let tier1_exchanges = [
///     ExchangeId::OKX,
///     ExchangeId::ByBit,
///     ExchangeId::MEXC,
/// ];
///
/// for exchange in tier1_exchanges {
///     println!("Exchange: {} (tier 1)", exchange);
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExchangeId {
    /// OKX - Spot + Futures + Options (Tier 1)
    OKX,
    /// ByBit - Spot + Futures + Options (Tier 1)
    ByBit,
    /// MEXC - Spot + Futures (Tier 1)
    MEXC,
    /// Gate.io - Spot + Futures (Tier 2)
    GateIo,
    /// Bitstamp - Spot with fiat pairs (Tier 2)
    Bitstamp,
    /// Kraken - Spot + Futures with fiat pairs (Tier 2)
    Kraken,
    /// Former Huobi (Legacy)
    HTX,
    /// BingX (Legacy)
    BingX,
    /// Hyperliquid (Legacy)
    Hyperliquid,
    /// KuCoin (Legacy)
    KuCoin,
    /// Bitget (Legacy)
    Bitget,
    /// Binance (Legacy)
    Binance,
    /// Coinbase (Legacy)
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

/// Order side for trade execution.
///
/// Represents the direction of a trade order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// Buy order - acquiring the base currency
    Buy,
    /// Sell order - disposing of the base currency
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

/// Order type for trade execution.
///
/// Defines how an order should be executed when submitted to an exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Market order - executes immediately at best available price
    Market,
    /// Limit order - executes only at specified price or better
    Limit,
    /// Stop-limit order - becomes active when stop price is triggered
    StopLimit,
}

/// Time in force for order execution.
///
/// Defines how long an order remains active before execution or expiration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good Till Cancelled - order remains active until filled or explicitly cancelled
    GTC,
    /// Immediate Or Cancel - order must fill immediately or cancel
    IOC,
    /// Fill Or Kill - order must fill completely or cancel entirely
    FOK,
    /// Good Till Date - order expires at specified date/time
    GTD,
}

/// Trading symbol representing a currency pair.
///
/// A symbol consists of a base currency (the asset being traded) and a quote
/// currency (the currency used to price the base).
///
/// # Example
///
/// ```rust
/// use arbitrage_core::types::Symbol;
///
/// let btc_usdt = Symbol::new("BTC", "USDT");
/// assert_eq!(btc_usdt.base, "BTC");
/// assert_eq!(btc_usdt.quote, "USDT");
///
/// // Parse from string
/// let eth_btc = Symbol::from_pair("ETH/BTC").unwrap();
/// assert_eq!(eth_btc.to_pair(), "ETH/BTC");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    /// The base currency (e.g., BTC in BTC/USDT)
    pub base: String,
    /// The quote currency (e.g., USDT in BTC/USDT)
    pub quote: String,
}

impl Symbol {
    /// Creates a new Symbol from base and quote strings.
    ///
    /// # Arguments
    ///
    /// * `base` - The base currency identifier
    /// * `quote` - The quote currency identifier
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::Symbol;
    ///
    /// let symbol = Symbol::new("SOL", "USDC");
    /// ```
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }

    /// Attempts to parse a symbol from a pair string (e.g., "BTC/USDT").
    ///
    /// # Arguments
    ///
    /// * `pair` - A string in the format "BASE/QUOTE"
    ///
    /// # Returns
    ///
    /// `Some(Symbol)` if parsing succeeds, `None` if format is invalid.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::Symbol;
    ///
    /// let valid = Symbol::from_pair("ETH/USDT");
    /// assert!(valid.is_some());
    ///
    /// let invalid = Symbol::from_pair("invalid");
    /// assert!(invalid.is_none());
    /// ```
    pub fn from_pair(pair: &str) -> Option<Self> {
        let parts: Vec<&str> = pair.split('/').collect();
        if parts.len() == 2 {
            Some(Self::new(parts[0], parts[1]))
        } else {
            None
        }
    }

    /// Converts the symbol to a pair string representation.
    ///
    /// # Returns
    ///
    /// A string in the format "BASE/QUOTE".
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::Symbol;
    ///
    /// let symbol = Symbol::new("XRP", "USDT");
    /// assert_eq!(symbol.to_pair(), "XRP/USDT");
    /// ```
    pub fn to_pair(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// Single price/quantity level in an order book.
///
/// Represents a single entry in the order book showing available liquidity
/// at a specific price level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// The price level for this order book entry
    pub price: Decimal,
    /// The available quantity at this price level
    pub quantity: Decimal,
}

impl OrderBookLevel {
    /// Creates a new OrderBookLevel.
    ///
    /// # Arguments
    ///
    /// * `price` - The price level
    /// * `quantity` - The available quantity at this price
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::OrderBookLevel;
    /// use rust_decimal::Decimal;
    ///
    /// let level = OrderBookLevel::new(
    ///     Decimal::from(50000),
    ///     Decimal::from(1)
    /// );
    /// ```
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self { price, quantity }
    }
}

/// Order book snapshot representing market depth.
///
/// Contains a complete view of bids and asks for a trading pair on a specific
/// exchange at a point in time. This is the primary data structure for
/// identifying arbitrage opportunities.
///
/// # Validation Rules
///
/// An order book is considered valid when:
/// 1. Both bids and asks collections are non-empty
/// 2. Bids are sorted in descending price order (highest bid first)
/// 3. Asks are sorted in ascending price order (lowest ask first)
/// 4. The best bid price is strictly less than the best ask price
///
/// # Example
///
/// ```rust
/// use arbitrage_core::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};
/// use rust_decimal::Decimal;
///
/// let symbol = Symbol::new("BTC", "USDT");
/// let bids = vec![
///     OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)),
///     OrderBookLevel::new(Decimal::from(49999), Decimal::from(2)),
/// ];
/// let asks = vec![
///     OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
///     OrderBookLevel::new(Decimal::from(50002), Decimal::from(2)),
/// ];
///
/// let order_book = OrderBook::new(
///     ExchangeId::Binance,
///     symbol.clone(),
///     bids,
///     asks,
/// );
///
/// assert!(order_book.is_valid());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    /// The exchange this order book is from
    pub exchange: ExchangeId,
    /// The trading symbol
    pub symbol: Symbol,
    /// Bid levels (buy orders), sorted highest to lowest
    pub bids: Vec<OrderBookLevel>,
    /// Ask levels (sell orders), sorted lowest to highest
    pub asks: Vec<OrderBookLevel>,
    /// Timestamp when this snapshot was taken
    pub timestamp: DateTime<Utc>,
    /// Optional sequence number for update ordering
    pub sequence: Option<u64>,
}

impl OrderBook {
    /// Creates a new OrderBook with the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange identifier
    /// * `symbol` - The trading symbol
    /// * `bids` - Collection of bid levels
    /// * `asks` - Collection of ask levels
    ///
    /// # Note
    ///
    /// The timestamp is automatically set to `Utc::now()` and sequence is set to `None`.
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

    /// Returns the best (highest) bid price and quantity.
    ///
    /// # Returns
    ///
    /// `Some(&OrderBookLevel)` if bids exist, `None` if the book is empty on the bid side.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::{OrderBook, OrderBookLevel, Symbol, ExchangeId};
    /// use rust_decimal::Decimal;
    ///
    /// let bids = vec![
    ///     OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)),
    /// ];
    /// let asks = vec![
    ///     OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
    /// ];
    ///
    /// let book = OrderBook::new(ExchangeId::Binance, Symbol::new("BTC", "USDT"), bids, asks);
    /// let best_bid = book.best_bid().unwrap();
    /// assert_eq!(best_bid.price, Decimal::from(50000));
    /// ```
    pub fn best_bid(&self) -> Option<&OrderBookLevel> {
        self.bids.first()
    }

    /// Returns the best (lowest) ask price and quantity.
    ///
    /// # Returns
    ///
    /// `Some(&OrderBookLevel)` if asks exist, `None` if the book is empty on the ask side.
    pub fn best_ask(&self) -> Option<&OrderBookLevel> {
        self.asks.first()
    }

    /// Calculates the spread between best bid and best ask.
    ///
    /// # Returns
    ///
    /// `Some(Decimal)` representing the spread if both bid and ask exist,
    /// `None` if either side is empty.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::{OrderBook, OrderBookLevel, Symbol, ExchangeId};
    /// use rust_decimal::Decimal;
    ///
    /// let bids = vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))];
    /// let asks = vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))];
    ///
    /// let book = OrderBook::new(ExchangeId::Binance, Symbol::new("BTC", "USDT"), bids, asks);
    /// let spread = book.spread().unwrap();
    /// assert_eq!(spread, Decimal::from(5));
    /// ```
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    /// Calculates the mid-price (average of best bid and best ask).
    ///
    /// # Returns
    ///
    /// `Some(Decimal)` representing the mid-price if both bid and ask exist,
    /// `None` if either side is empty.
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some((ask.price + bid.price) / Decimal::from(2)),
            _ => None,
        }
    }

    /// Validates the order book's integrity.
    ///
    /// # Returns
    ///
    /// `true` if the order book is valid, `false` otherwise.
    ///
    /// # Validation Rules
    ///
    /// 1. Both bids and asks must be non-empty
    /// 2. Bids must be sorted in descending price order (highest first)
    /// 3. Asks must be sorted in ascending price order (lowest first)
    /// 4. Best bid price must be strictly less than best ask price
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::{OrderBook, OrderBookLevel, Symbol, ExchangeId};
    /// use rust_decimal::Decimal;
    ///
    /// // Valid order book
    /// let valid_bids = vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))];
    /// let valid_asks = vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))];
    /// let valid_book = OrderBook::new(ExchangeId::Binance, Symbol::new("BTC", "USDT"), valid_bids, valid_asks);
    /// assert!(valid_book.is_valid());
    ///
    /// // Invalid: empty bids
    /// let invalid_bids = vec![];
    /// let invalid_asks = vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))];
    /// let invalid_book = OrderBook::new(ExchangeId::Binance, Symbol::new("BTC", "USDT"), invalid_bids, invalid_asks);
    /// assert!(!invalid_book.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        if self.bids.is_empty() || self.asks.is_empty() {
            return false;
        }

        let bids_sorted = self.bids.windows(2).all(|w| w[0].price >= w[1].price);
        let asks_sorted = self.asks.windows(2).all(|w| w[0].price <= w[1].price);

        let spread_valid = match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => bid.price < ask.price,
            _ => false,
        };

        let no_zero_quantities = self.bids.iter().all(|l| l.quantity > Decimal::ZERO)
            && self.asks.iter().all(|l| l.quantity > Decimal::ZERO);

        bids_sorted && asks_sorted && spread_valid && no_zero_quantities
    }

    /// Calculates Volume-Weighted Average Price (VWAP) for buying a given quantity.
    ///
    /// Simulates executing a buy order by traversing the order book from best ask
    /// to worst, accumulating quantity until the target is filled.
    ///
    /// # Arguments
    ///
    /// * `target_quantity` - The quantity to attempt to buy
    ///
    /// # Returns
    ///
    /// `Some(VwapResult)` if any quantity can be filled, `None` if quantity is
    /// non-positive or asks are empty.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::{OrderBook, OrderBookLevel, Symbol, ExchangeId};
    /// use rust_decimal::Decimal;
    ///
    /// let asks = vec![
    ///     OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)),
    ///     OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
    /// ];
    /// let bids = vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(10))];
    ///
    /// let book = OrderBook::new(ExchangeId::Binance, Symbol::new("BTC", "USDT"), bids, asks);
    ///
    /// if let Some(result) = book.vwap_buy(Decimal::from(2)) {
    ///     println!("VWAP: {}, Filled: {}", result.vwap_price, result.filled_quantity);
    /// }
    /// ```
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

    /// Calculates Volume-Weighted Average Price (VWAP) for selling a given quantity.
    ///
    /// Simulates executing a sell order by traversing the order book from best bid
    /// to worst, accumulating quantity until the target is filled.
    ///
    /// # Arguments
    ///
    /// * `target_quantity` - The quantity to attempt to sell
    ///
    /// # Returns
    ///
    /// `Some(VwapResult)` if any quantity can be filled, `None` if quantity is
    /// non-positive or bids are empty.
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

    /// Calculates available liquidity at or better than a specified price level.
    ///
    /// # Arguments
    ///
    /// * `price` - The price limit
    /// * `is_buy` - If true, sums asks at or below price; if false, sums bids at or above price
    ///
    /// # Returns
    ///
    /// The total quantity available at the specified price level or better.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::{OrderBook, OrderBookLevel, Symbol, ExchangeId};
    /// use rust_decimal::Decimal;
    ///
    /// let asks = vec![
    ///     OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)),
    ///     OrderBookLevel::new(Decimal::from(50001), Decimal::from(2)),
    ///     OrderBookLevel::new(Decimal::from(50002), Decimal::from(3)),
    /// ];
    /// let bids = vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(10))];
    ///
    /// let book = OrderBook::new(ExchangeId::Binance, Symbol::new("BTC", "USDT"), bids, asks);
    ///
    /// // Liquidity at 50001 for buying
    /// let liquidity = book.liquidity_at_price(Decimal::from(50001), true);
    /// assert_eq!(liquidity, Decimal::from(3)); // Only level at exactly 50001
    /// ```
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

    /// Internal method to calculate slippage in basis points.
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

/// Result of a VWAP calculation.
///
/// Contains detailed information about the simulated execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapResult {
    /// The Volume-Weighted Average Price achieved
    pub vwap_price: Decimal,
    /// The total quantity successfully filled
    pub filled_quantity: Decimal,
    /// The total cost (for buy) or revenue (for sell)
    pub total_cost: Decimal,
    /// Whether the full target quantity was filled
    pub is_fully_filled: bool,
    /// Slippage in basis points from the best price
    pub slippage_bps: i32,
}

/// Arbitrage signal representing a detected trading opportunity.
///
/// A signal contains all the information needed to evaluate and potentially
/// execute an arbitrage trade. It includes pricing from both exchanges,
/// calculated profits, and metadata for tracking.
///
/// # Example
///
/// ```rust
/// use arbitrage_core::types::{ExchangeId, Signal, Symbol};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let symbol = Symbol::new("BTC", "USDT");
/// let signal = Signal::new(
///     symbol,
///     ExchangeId::Binance,
///     ExchangeId::ByBit,
///     Decimal::from(50000),
///     Decimal::from(50200),
///     Utc::now(),
/// );
///
/// // Signal created successfully with unique ID
/// assert!(!signal.id.to_string().is_empty());
/// assert_eq!(signal.buy_price, Decimal::from(50000));
/// assert_eq!(signal.sell_price, Decimal::from(50200));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    /// Unique identifier for this signal
    pub id: Uuid,
    /// The trading symbol
    pub symbol: Symbol,
    /// The exchange to buy from (lower price)
    pub buy_exchange: ExchangeId,
    /// The exchange to sell to (higher price)
    pub sell_exchange: ExchangeId,
    /// Price on the buy exchange
    pub buy_price: Decimal,
    /// Price on the sell exchange
    pub sell_price: Decimal,
    /// Gross profit percentage (before fees)
    pub gross_profit_percent: Decimal,
    /// Net profit percentage (after estimated fees)
    pub net_profit_percent: Decimal,
    /// Net profit in absolute terms (quote currency)
    pub net_profit_absolute: Decimal,
    /// Confidence score (0-100)
    pub confidence: Decimal,
    /// Recommended position size
    pub recommended_size: Decimal,
    /// Maximum allowable position size
    pub max_size: Decimal,
    /// Expected slippage percentage
    pub expected_slippage: Decimal,
    /// Estimated execution time in milliseconds
    pub estimated_execution_time_ms: u64,
    /// When this signal was created
    pub created_at: DateTime<Utc>,
    /// When this signal expires
    pub expires_at: DateTime<Utc>,
    /// Additional metadata for strategy-specific data
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Signal {
    /// Creates a new Signal with basic parameters.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading pair
    /// * `buy_exchange` - Exchange to buy from
    /// * `sell_exchange` - Exchange to sell to
    /// * `buy_price` - Price on buy exchange
    /// * `sell_price` - Price on sell exchange
    /// * `created_at` - Timestamp of signal creation
    ///
    /// # Panics
    ///
    /// Panics if `created_at` is in the future.
    ///
    /// # Note
    ///
    /// Profit fields are initialized to zero and should be calculated separately.
    /// The default expiry is 5 minutes from creation.
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
            expires_at: now + chrono::Duration::minutes(5),
            metadata: HashMap::new(),
        }
    }

    /// Checks if the signal has expired.
    ///
    /// # Returns
    ///
    /// `true` if `Utc::now()` is past the expiry time, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::types::{Signal, Symbol};
    /// use chrono::Utc;
    ///
    /// let signal = Signal::new(
    ///     Symbol::new("BTC", "USDT"),
    ///     arbitrage_core::types::ExchangeId::Binance,
    ///     arbitrage_core::types::ExchangeId::ByBit,
    ///     rust_decimal::Decimal::from(50000),
    ///     rust_decimal::Decimal::from(50010),
    ///     Utc::now(),
    /// );
    ///
    /// assert!(!signal.is_expired());
    /// ```
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Calculates the age of the signal in seconds.
    ///
    /// # Returns
    ///
    /// The number of seconds since creation, or 0 if creation time is in the future.
    pub fn age_seconds(&self) -> i64 {
        let duration = Utc::now() - self.created_at;
        if duration.num_seconds() < 0 {
            0
        } else {
            duration.num_seconds()
        }
    }
}

/// Order instruction for exchange execution.
///
/// Represents a prepared order ready to be submitted to an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique order identifier
    pub id: Uuid,
    /// Client-specified order identifier
    pub client_order_id: String,
    /// Target exchange
    pub exchange: ExchangeId,
    /// Trading symbol
    pub symbol: Symbol,
    /// Buy or Sell
    pub side: Side,
    /// Order type
    pub order_type: OrderType,
    /// Order quantity
    pub quantity: Decimal,
    /// Limit price (None for market orders)
    pub price: Option<Decimal>,
    /// Time in force
    pub time_in_force: TimeInForce,
    /// Expected fee in quote currency
    pub expected_fee: Decimal,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl Order {
    /// Creates a new Order with a generated client order ID.
    ///
    /// # Arguments
    ///
    /// * `exchange` - Target exchange
    /// * `symbol` - Trading symbol
    /// * `side` - Buy or Sell
    /// * `order_type` - Market, Limit, or StopLimit
    /// * `quantity` - Order quantity
    /// * `price` - Limit price (None for market orders)
    ///
    /// # Note
    ///
    /// Time in force defaults to IOC (Immediate Or Cancel).
    /// Expected fee is initialized to zero.
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

/// Execution instruction containing coordinated buy and sell orders.
///
/// Represents a complete arbitrage execution plan with both legs of the trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInstruction {
    /// Unique instruction identifier
    pub id: Uuid,
    /// Associated signal ID
    pub signal_id: Uuid,
    /// Buy order for the buy exchange
    pub buy_order: Order,
    /// Sell order for the sell exchange
    pub sell_order: Order,
    /// Expected profit after fees
    pub expected_profit: Decimal,
    /// Worst-case profit after slippage
    pub worst_case_profit: Decimal,
    /// Total fees for both orders
    pub total_fees: Decimal,
    /// Slippage buffer applied
    pub slippage_buffer: Decimal,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Validation errors, if any
    pub validation_errors: Vec<String>,
}

impl ExecutionInstruction {
    /// Creates a new ExecutionInstruction from a signal and orders.
    ///
    /// # Arguments
    ///
    /// * `signal_id` - The ID of the signal this instruction is based on
    /// * `buy_order` - The buy order
    /// * `sell_order` - The sell order
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

    /// Checks if the instruction is valid (no validation errors).
    ///
    /// # Returns
    ///
    /// `true` if validation_errors is empty, `false` otherwise.
    pub fn is_valid(&self) -> bool {
        self.validation_errors.is_empty()
    }
}

/// Exchange fee schedule for cost calculations.
///
/// Contains the maker and taker fee rates for an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeSchedule {
    /// The exchange this fee schedule applies to
    pub exchange: ExchangeId,
    /// Maker fee rate (for providing liquidity)
    pub maker_fee: Decimal,
    /// Taker fee rate (for taking liquidity)
    pub taker_fee: Decimal,
    /// Optional fee tier description
    pub tier: Option<String>,
}

impl FeeSchedule {
    /// Creates a new FeeSchedule.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `maker_fee` - Maker fee rate (e.g., 0.001 for 0.1%)
    /// * `taker_fee` - Taker fee rate (e.g., 0.001 for 0.1%)
    ///
    /// # Panics
    ///
    /// Panics if either fee is negative.
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

/// Exchange connection status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Not connected
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Successfully connected
    Connected,
    /// Attempting to reconnect
    Reconnecting,
    /// Error state with message
    Error(String),
}

/// Exchange connection and status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeStatus {
    /// The exchange
    pub exchange: ExchangeId,
    /// Current connection status
    pub status: ConnectionStatus,
    /// Last heartbeat timestamp
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// Uptime percentage
    pub uptime_percent: Decimal,
    /// Number of errors since startup
    pub error_count: u64,
    /// List of subscribed symbols
    pub subscribed_symbols: Vec<Symbol>,
}

impl ExchangeStatus {
    /// Creates a new ExchangeStatus with default values.
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

/// Trait for converting Decimal values to basis points (bps).
///
/// Basis points are commonly used in finance where 1 basis point = 0.01% = 0.0001.
pub trait ToBps {
    /// Converts the value to basis points.
    ///
    /// # Returns
    ///
    /// `Some(i32)` if conversion succeeds, `None` if the value is too large.
    fn to_bps(&self) -> Option<i32>;

    /// Converts the value to basis points, returning 0 on failure.
    ///
    /// # Returns
    ///
    /// The value in basis points, or 0 if conversion fails.
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

/// Trait for calculating mid price from market data.
pub trait MidPrice {
    /// Calculates the mid price.
    ///
    /// # Returns
    ///
    /// `Some(Decimal)` if mid price can be calculated, `None` otherwise.
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

/// Exchange constants for convenience access.
pub mod exchange_constants {
    use super::ExchangeId;

    /// All supported exchanges
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

    /// Exchanges supporting perpetual contracts
    pub const PERPETUAL_EXCHANGES: [ExchangeId; 3] =
        [ExchangeId::OKX, ExchangeId::ByBit, ExchangeId::MEXC];

    /// Exchanges with good spot liquidity
    pub const SPOT_EXCHANGES: [ExchangeId; 6] = [
        ExchangeId::OKX,
        ExchangeId::ByBit,
        ExchangeId::MEXC,
        ExchangeId::GateIo,
        ExchangeId::Bitstamp,
        ExchangeId::Kraken,
    ];
}

use super::constants::BASIS_POINTS_DIVISOR;

/// Calculates profit in basis points from buy and sell prices.
///
/// # Arguments
///
/// * `buy_price` - The purchase price
/// * `sell_price` - The sale price
///
/// # Returns
///
/// `Some(i32)` representing profit in basis points if buy_price > 0,
/// `None` if buy_price is zero.
pub fn calculate_profit_bps(buy_price: Decimal, sell_price: Decimal) -> Option<i32> {
    if buy_price.is_zero() {
        return None;
    }
    let profit_ratio = (sell_price - buy_price) / buy_price;
    (profit_ratio * Decimal::from(BASIS_POINTS_DIVISOR)).to_i32()
}
