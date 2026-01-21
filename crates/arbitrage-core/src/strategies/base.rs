//! Core strategy trait and context structures.
//!
//! This module defines the Strategy trait that all arbitrage strategies must implement,
//! along with the context structures used for filtering and execution.
//!
//! # Strategy Trait
//!
//! The [`Strategy`] trait defines the core interface for arbitrage strategies:
//! - `detect()`: Scan market data for opportunities
//! - `filter()`: Apply strategy-specific filtering criteria
//! - `config()`: Access strategy configuration
//!
//! The [`ArbitrageStrategy`] trait extends [`Strategy`] with default implementations
//! for common filtering logic.
//!
//! # Context Structures
//!
//! - [`FilterContext`]: Provides constraints for signal filtering (min profit, max exposure, etc.)
//! - [`ExecutionContext`]: Provides execution parameters (balances, fees, slippage tolerance)
//!
//! # Example
//!
//! ```rust
//! use arbitrage_core::strategies::{Strategy, ArbitrageStrategy, FilterContext, ExecutionContext};
//!
//! // A strategy would implement the Strategy trait
//! // FilterContext provides constraints for filtering signals
//! let filter_ctx = FilterContext::new(10); // 10 bps min profit
//! assert!(filter_ctx.min_profit_bps == 10);
//! ```

use crate::{
    ArbitrageError, ExchangeId, ExecutionInstruction, OrderBook, OrderType, Result, Side, Signal,
    Symbol,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Core strategy trait that all arbitrage strategies must implement.
///
/// Strategies are responsible for:
/// 1. Detecting arbitrage opportunities from market data
/// 2. Filtering signals based on strategy-specific criteria
/// 3. Managing their own configuration
///
/// # Method Contracts
///
/// - `id()`: Returns a static string identifier (e.g., "cross_exchange")
/// - `name()`: Returns a human-readable name
/// - `detect()`: Takes market data, returns detected opportunities
/// - `filter()`: Validates a raw signal against context constraints
/// - `config()`: Returns current strategy configuration
/// - `update_config()`: Updates strategy parameters dynamically
///
/// # Thread Safety
///
/// Strategies must be `Send + Sync` to allow concurrent execution.
pub trait Strategy: Send + Sync {
    /// Returns a static string identifier for the strategy.
    ///
    /// Used for logging, metrics, and configuration.
    fn id(&self) -> &'static str;

    /// Returns a human-readable name for the strategy.
    fn name(&self) -> &'static str;

    /// Detects arbitrage opportunities from market data.
    ///
    /// # Arguments
    ///
    /// * `market_data` - Bundle containing order books, tickers, and market state
    ///
    /// # Returns
    ///
    /// A vector of detected raw signals, possibly empty.
    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>>;

    /// Filters a raw signal based on strategy-specific criteria.
    ///
    /// # Arguments
    ///
    /// * `signal` - The raw signal to filter
    /// * `context` - Filter constraints (min profit, max exposure, etc.)
    ///
    /// # Returns
    ///
    /// `Ok(true)` if signal passes filters, `Ok(false)` if rejected,
    /// or an error if filtering failed.
    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool>;

    /// Plans execution for a validated signal.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated. Use the `ExecutionPreparer` struct instead.
    /// Strategies should focus on signal detection, not execution planning.
    fn plan(&self, signal: &Signal, context: &ExecutionContext) -> Result<ExecutionInstruction> {
        let _ = (signal, context);
        Err(ArbitrageError::Configuration(
            "Strategy execution planning is deprecated - use ExecutionPreparer instead".to_string(),
        ))
    }

    /// Returns the current strategy configuration.
    fn config(&self) -> &StrategyConfig;

    /// Updates the strategy configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - New configuration parameters
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, error if configuration is invalid.
    fn update_config(&mut self, config: StrategyConfig) -> Result<()>;
}

/// Extended strategy trait with default filter implementation.
///
/// Eliminates boilerplate by providing common filter logic that most
/// strategies will want to use.
///
/// # Default Implementation
///
/// The default implementations provide:
/// - Signal validity checking
/// - Leg count validation
/// - Exchange whitelist checking
/// - Minimum profit threshold checking
/// - Maximum exposure checking
/// - Inventory validation hook
///
/// # Example
///
/// ```rust
/// use arbitrage_core::strategies::{ArbitrageStrategy, FilterContext, RawSignal};
/// use arbitrage_core::types::{Symbol, ExchangeId};
/// use std::sync::Arc;
///
/// let signal = RawSignal::new("test", Arc::new(Symbol::new("BTC", "USDT")));
/// let context = FilterContext::new(10); // 10 bps min profit
///
/// // Default validation is available via ArbitrageStrategy
/// ```
pub trait ArbitrageStrategy: Strategy {
    /// Returns the expected number of legs for this strategy.
    ///
    /// Default: 2 (buy and sell)
    fn expected_leg_count(&self) -> usize {
        2
    }

    /// Validates a signal with default criteria.
    ///
    /// Default checks:
    /// 1. Signal is valid (has legs, positive profit)
    /// 2. Leg count matches expected
    /// 3. All exchanges are allowed
    /// 4. Profit exceeds minimum threshold
    /// 5. Total notional doesn't exceed max exposure
    /// 6. Inventory validation (via hook)
    ///
    /// # Arguments
    ///
    /// * `signal` - The signal to validate
    /// * `context` - Filter constraints
    ///
    /// # Returns
    ///
    /// `Ok(true)` if signal passes all checks, `Ok(false)` otherwise.
    fn validate_signal(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        if !signal.is_valid() {
            return Ok(false);
        }

        if signal.legs.len() != self.expected_leg_count() {
            return Ok(false);
        }

        let exchanges = signal.get_exchanges();
        if !exchanges.iter().all(|ex| context.is_exchange_allowed(*ex)) {
            return Ok(false);
        }

        if signal.expected_profit_bps < context.min_profit_bps {
            return Ok(false);
        }

        let total = signal.total_notional();
        if total > context.max_exposure {
            return Ok(false);
        }

        self.validate_inventory(signal, context)
    }

    /// Validates inventory constraints.
    ///
    /// Override this method to implement strategy-specific inventory checks.
    /// Default implementation always returns `true` (no inventory check).
    ///
    /// # Arguments
    ///
    /// * `signal` - The signal to check
    /// * `context` - Filter context with inventory limits
    ///
    /// # Returns
    ///
    /// `Ok(true)` if inventory constraints are satisfied.
    fn validate_inventory(&self, _signal: &RawSignal, _context: &FilterContext) -> Result<bool> {
        Ok(true)
    }
}

/// Bundle of market data for strategy analysis.
///
/// Contains all market data available for a strategy's detection phase.
/// Data is organized by exchange and symbol for efficient lookup.
///
/// # Structure
///
/// - `order_books`: Order book snapshots by (exchange, symbol)
/// - `funding_rates`: Perpetual funding rates
/// - `tickers`: Latest ticker data
/// - `timestamp`: When this bundle was created
///
/// # Example
///
/// ```rust
/// use arbitrage_core::strategies::MarketBundle;
/// use arbitrage_core::types::{OrderBook, ExchangeId, Symbol};
/// use std::sync::Arc;
///
/// let mut bundle = MarketBundle::new();
///
/// let order_book = Arc::new(OrderBook::new(
///     ExchangeId::Binance,
///     Symbol::new("BTC", "USDT"),
///     vec![],
///     vec![],
/// ));
///
/// bundle.add_order_book(order_book);
/// ```
#[derive(Debug, Clone)]
pub struct MarketBundle {
    /// Order books indexed by (exchange, symbol)
    pub order_books: HashMap<(ExchangeId, Arc<Symbol>), Arc<OrderBook>>,
    /// Funding rates indexed by (exchange, symbol)
    pub funding_rates: HashMap<(ExchangeId, Arc<Symbol>), Arc<FundingRate>>,
    /// Tickers indexed by (exchange, symbol)
    pub tickers: HashMap<(ExchangeId, Arc<Symbol>), Arc<Ticker>>,
    /// Bundle creation timestamp
    pub timestamp: DateTime<Utc>,
}

impl MarketBundle {
    /// Creates an empty MarketBundle.
    pub fn new() -> Self {
        Self {
            order_books: HashMap::new(),
            funding_rates: HashMap::new(),
            tickers: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Adds an order book to the bundle.
    pub fn add_order_book(&mut self, order_book: Arc<OrderBook>) {
        self.order_books.insert(
            (order_book.exchange, Arc::new(order_book.symbol.clone())),
            Arc::clone(&order_book),
        );
    }

    /// Adds a funding rate to the bundle.
    pub fn add_funding_rate(&mut self, funding_rate: Arc<FundingRate>) {
        self.funding_rates.insert(
            (funding_rate.exchange, Arc::new(funding_rate.symbol.clone())),
            Arc::clone(&funding_rate),
        );
    }

    /// Adds a ticker to the bundle.
    pub fn add_ticker(&mut self, ticker: Arc<Ticker>) {
        self.tickers.insert(
            (ticker.exchange, Arc::new(ticker.symbol.clone())),
            Arc::clone(&ticker),
        );
    }

    /// Gets an order book for a specific exchange and symbol.
    pub fn get_order_book(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<&Arc<OrderBook>> {
        self.order_books.get(&(exchange, Arc::new(symbol.clone())))
    }

    /// Gets a funding rate for a specific exchange and symbol.
    pub fn get_funding_rate(
        &self,
        exchange: ExchangeId,
        symbol: &Symbol,
    ) -> Option<&Arc<FundingRate>> {
        self.funding_rates
            .get(&(exchange, Arc::new(symbol.clone())))
    }

    /// Gets a ticker for a specific exchange and symbol.
    pub fn get_ticker(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<&Arc<Ticker>> {
        self.tickers.get(&(exchange, Arc::new(symbol.clone())))
    }

    /// Gets all exchanges that have data for a symbol.
    pub fn get_exchanges_for_symbol(&self, symbol: &Symbol) -> Vec<ExchangeId> {
        self.order_books
            .keys()
            .filter(|(_, s)| *s == Arc::new(symbol.clone()))
            .map(|(exchange, _)| *exchange)
            .collect()
    }

    /// Gets all unique symbols in the bundle.
    pub fn get_all_symbols(&self) -> Vec<Arc<Symbol>> {
        let mut symbols: Vec<Arc<Symbol>> = Vec::new();

        symbols.extend(
            self.order_books
                .keys()
                .map(|(_, symbol)| Arc::clone(symbol)),
        );

        symbols.extend(self.tickers.keys().map(|(_, symbol)| Arc::clone(symbol)));

        symbols.extend(
            self.funding_rates
                .keys()
                .map(|(_, symbol)| Arc::clone(symbol)),
        );

        symbols.sort_by_key(|a| a.to_pair());
        symbols.dedup();
        symbols
    }

    /// Checks if data exists for an exchange and symbol.
    pub fn has_data(&self, exchange: ExchangeId, symbol: &Symbol) -> bool {
        self.order_books
            .contains_key(&(exchange, Arc::new(symbol.clone())))
    }

    /// Gets the age of data for an exchange and symbol.
    pub fn get_data_age(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<chrono::Duration> {
        self.get_order_book(exchange, symbol)
            .map(|ob| self.timestamp - ob.timestamp)
    }

    /// Gets all order books for a specific symbol.
    pub fn get_order_books_for_symbol(&self, symbol: &Symbol) -> Vec<&Arc<OrderBook>> {
        let target_symbol = Arc::new(symbol.clone());
        self.order_books
            .iter()
            .filter(|((_, s), _)| *s == target_symbol)
            .map(|(_, ob)| ob)
            .collect()
    }

    /// Gets all tickers for a specific symbol.
    pub fn get_tickers_for_symbol(&self, symbol: &Symbol) -> Vec<&Arc<Ticker>> {
        let target_symbol = Arc::new(symbol.clone());
        self.tickers
            .iter()
            .filter(|((_, s), _)| *s == target_symbol)
            .map(|(_, t)| t)
            .collect()
    }
}

impl Default for MarketBundle {
    fn default() -> Self {
        Self::new()
    }
}

/// Raw signal detected by a strategy before filtering and scoring.
///
/// Contains the detected opportunity with all legs and metadata.
/// This is the output of the `detect()` phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSignal {
    /// Strategy identifier that generated this signal
    pub strategy_id: String,
    /// The trading symbol
    pub symbol: Arc<Symbol>,
    /// Individual legs of the arbitrage (buy/sell on different exchanges)
    pub legs: Vec<TradeLeg>,
    /// Expected profit in basis points
    pub expected_profit_bps: i32,
    /// Basis spread in basis points (if applicable)
    pub basis_bps: Option<i32>,
    /// Factors contributing to confidence
    pub confidence_factors: ConfidenceFactors,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// When the signal was detected
    pub detected_at: DateTime<Utc>,
}

impl RawSignal {
    /// Creates a new RawSignal.
    ///
    /// # Arguments
    ///
    /// * `strategy_id` - The strategy that detected this signal
    /// * `symbol` - The trading symbol
    pub fn new(strategy_id: impl Into<String>, symbol: Arc<Symbol>) -> Self {
        Self {
            strategy_id: strategy_id.into(),
            symbol,
            legs: Vec::new(),
            expected_profit_bps: 0,
            basis_bps: None,
            confidence_factors: ConfidenceFactors::default(),
            metadata: HashMap::new(),
            detected_at: Utc::now(),
        }
    }

    /// Adds a trade leg to the signal.
    pub fn add_leg(&mut self, leg: TradeLeg) {
        self.legs.push(leg);
    }

    /// Adds a buy leg with fluent builder pattern.
    ///
    /// # Arguments
    ///
    /// * `exchange` - Exchange to buy from
    /// * `price` - Buy price
    /// * `quantity` - Buy quantity
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn add_buy_leg(mut self, exchange: ExchangeId, price: Decimal, quantity: Decimal) -> Self {
        let leg = TradeLeg::new(
            exchange,
            Arc::clone(&self.symbol),
            Side::Buy,
            price,
            quantity,
        );
        self.legs.push(leg);
        self
    }

    /// Adds a sell leg with fluent builder pattern.
    ///
    /// # Arguments
    ///
    /// * `exchange` - Exchange to sell to
    /// * `price` - Sell price
    /// * `quantity` - Sell quantity
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn add_sell_leg(mut self, exchange: ExchangeId, price: Decimal, quantity: Decimal) -> Self {
        let leg = TradeLeg::new(
            exchange,
            Arc::clone(&self.symbol),
            Side::Sell,
            price,
            quantity,
        );
        self.legs.push(leg);
        self
    }

    /// Sets the profit in basis points.
    pub fn set_profit_bps(&mut self, profit_bps: i32) {
        self.expected_profit_bps = profit_bps;
    }

    /// Sets profit in basis points with fluent builder pattern.
    pub fn with_profit_bps(mut self, bps: i32) -> Self {
        self.expected_profit_bps = bps;
        self
    }

    /// Adds metadata to the signal.
    pub fn add_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata.insert(key.into(), value);
    }

    /// Adds metadata with fluent builder pattern.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Validates that the signal has valid legs and profit.
    ///
    /// # Returns
    ///
    /// `true` if signal has at least one leg, all legs have non-zero price
    /// and quantity, and profit is positive.
    pub fn is_valid(&self) -> bool {
        !self.legs.is_empty()
            && self
                .legs
                .iter()
                .all(|leg| !leg.price.is_zero() && !leg.quantity.is_zero())
            && self.expected_profit_bps > 0
    }

    /// Gets total notional value of all legs.
    ///
    /// # Returns
    ///
    /// Sum of (price * quantity) for all legs.
    pub fn total_notional(&self) -> Decimal {
        self.legs.iter().map(|leg| leg.price * leg.quantity).sum()
    }

    /// Gets unique exchanges involved in this signal.
    ///
    /// # Returns
    ///
    /// Vector of unique exchange IDs, sorted alphabetically.
    pub fn get_exchanges(&self) -> Vec<ExchangeId> {
        let mut exchanges: Vec<ExchangeId> = self.legs.iter().map(|leg| leg.exchange).collect();
        exchanges.sort_by_key(|a| a.to_string());
        exchanges.dedup();
        exchanges
    }

    /// Checks if signal involves a specific exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - Exchange to check
    ///
    /// # Returns
    ///
    /// `true` if any leg involves the exchange.
    pub fn involves_exchange(&self, exchange: ExchangeId) -> bool {
        self.legs.iter().any(|leg| leg.exchange == exchange)
    }
}

/// Individual trade leg in an arbitrage opportunity.
///
/// Represents one side of the trade (buy or sell on a specific exchange).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeLeg {
    /// The exchange for this leg
    pub exchange: ExchangeId,
    /// The trading symbol
    pub symbol: Arc<Symbol>,
    /// Buy or Sell
    pub side: Side,
    /// Price for this leg
    pub price: Decimal,
    /// Quantity for this leg
    pub quantity: Decimal,
    /// Order type for execution
    pub order_type: OrderType,
}

impl TradeLeg {
    /// Creates a new TradeLeg.
    pub fn new(
        exchange: ExchangeId,
        symbol: Arc<Symbol>,
        side: Side,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        Self {
            exchange,
            symbol,
            side,
            price,
            quantity,
            order_type: OrderType::Market,
        }
    }

    /// Sets the order type with fluent builder pattern.
    pub fn with_order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = order_type;
        self
    }
}

/// Factors contributing to signal confidence.
///
/// Individual scores that contribute to the overall confidence calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceFactors {
    /// Score based on market depth (0-100)
    pub depth_score: Decimal,
    /// Score based on price volatility (0-100)
    pub volatility_score: Decimal,
    /// Score based on exchange reliability (0-100)
    pub reliability_score: Decimal,
    /// Score based on spread stability (0-100)
    pub spread_stability: Decimal,
    /// Score based on data freshness (0-100)
    pub freshness_score: Decimal,
}

impl Default for ConfidenceFactors {
    fn default() -> Self {
        Self {
            depth_score: Decimal::ZERO,
            volatility_score: Decimal::ZERO,
            reliability_score: Decimal::ZERO,
            spread_stability: Decimal::ZERO,
            freshness_score: Decimal::ZERO,
        }
    }
}

/// Context for signal filtering.
///
/// Provides constraints and limits for signal filtering decisions.
/// All fields are public for direct access.
///
/// # Fields
///
/// - `min_profit_bps`: Minimum profit in basis points required (default: 10)
/// - `max_exposure`: Maximum total notional exposure in quote currency (default: $10,000)
/// - `allowed_exchanges`: List of whitelisted exchanges (default: OKX, ByBit, MEXC, GateIo)
/// - `risk_limits`: Risk management limits
/// - `fee_schedules`: Exchange fee schedules for profit calculations
/// - `max_latency_ms`: Maximum acceptable data age in milliseconds (default: 500)
/// - `min_notional_usd`: Minimum order notional in USD (default: $10)
/// - `inventory_limits`: Per-exchange, per-asset inventory limits
///
/// # Example
///
/// ```rust
/// use arbitrage_core::strategies::FilterContext;
/// use arbitrage_core::types::{ExchangeId, FeeSchedule};
/// use rust_decimal::Decimal;
///
/// let mut ctx = FilterContext::new(15); // 15 bps min profit
///
/// // Customize
/// ctx.max_exposure = Decimal::from(50000);
/// ctx.allowed_exchanges = vec![ExchangeId::Binance, ExchangeId::ByBit];
///
/// // Add fee schedule
/// ctx.add_fee_schedule(FeeSchedule::new(
///     ExchangeId::Binance,
///     Decimal::new(1, 3), // 0.1% maker
///     Decimal::new(1, 3), // 0.1% taker
/// ));
///
/// assert!(ctx.is_exchange_allowed(ExchangeId::Binance));
/// assert!(!ctx.is_exchange_allowed(ExchangeId::Coinbase));
/// ```
#[derive(Debug, Clone)]
pub struct FilterContext {
    /// Minimum profit in basis points required
    pub min_profit_bps: i32,
    /// Maximum total notional exposure
    pub max_exposure: Decimal,
    /// List of allowed exchanges
    pub allowed_exchanges: Vec<ExchangeId>,
    /// Risk management limits
    pub risk_limits: RiskLimits,
    /// Exchange fee schedules
    pub fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    /// Maximum data age in milliseconds
    pub max_latency_ms: u64,
    /// Minimum order notional in USD
    pub min_notional_usd: Decimal,
    /// Per-exchange, per-asset inventory limits
    pub inventory_limits: HashMap<(ExchangeId, String), Decimal>,
}

impl FilterContext {
    /// Creates a new FilterContext with minimum profit threshold.
    ///
    /// # Arguments
    ///
    /// * `min_profit_bps` - Minimum profit in basis points
    ///
    /// # Default Values
    ///
    /// - `max_exposure`: $10,000
    /// - `allowed_exchanges`: OKX, ByBit, MEXC, GateIo
    /// - `max_latency_ms`: 500ms
    /// - `min_notional_usd`: $10
    pub fn new(min_profit_bps: i32) -> Self {
        Self {
            min_profit_bps,
            max_exposure: Decimal::from(10000),
            allowed_exchanges: vec![
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::MEXC,
                ExchangeId::GateIo,
            ],
            risk_limits: RiskLimits::default(),
            fee_schedules: HashMap::new(),
            max_latency_ms: 500,
            min_notional_usd: Decimal::from(10),
            inventory_limits: HashMap::new(),
        }
    }

    /// Gets fee schedule for an exchange.
    pub fn get_fee_schedule(&self, exchange: ExchangeId) -> Option<&FeeSchedule> {
        self.fee_schedules.get(&exchange)
    }

    /// Checks if an asset can be sold on an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `asset` - The asset symbol
    /// * `quantity` - The quantity to sell
    ///
    /// # Returns
    ///
    /// `true` if inventory limit allows the sale.
    pub fn can_sell(&self, exchange: ExchangeId, asset: &str, quantity: Decimal) -> bool {
        let key = (exchange, asset.to_string());
        let available = self
            .inventory_limits
            .get(&key)
            .copied()
            .unwrap_or(Decimal::ZERO);

        available >= quantity
    }

    /// Checks if an exchange is allowed for trading.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange to check
    ///
    /// # Returns
    ///
    /// `true` if the exchange is in the allowed list.
    pub fn is_exchange_allowed(&self, exchange: ExchangeId) -> bool {
        self.allowed_exchanges.contains(&exchange)
    }

    /// Checks if both exchanges in a pair are allowed.
    ///
    /// # Arguments
    ///
    /// * `exchange1` - First exchange
    /// * `exchange2` - Second exchange
    ///
    /// # Returns
    ///
    /// `true` if both exchanges are allowed.
    pub fn are_exchanges_allowed(&self, exchange1: ExchangeId, exchange2: ExchangeId) -> bool {
        self.is_exchange_allowed(exchange1) && self.is_exchange_allowed(exchange2)
    }

    /// Gets inventory balance for an asset on an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `asset` - The asset symbol
    ///
    /// # Returns
    ///
    /// The available balance, or zero if not tracked.
    pub fn get_inventory(&self, exchange: ExchangeId, asset: &str) -> Decimal {
        self.inventory_limits
            .get(&(exchange, asset.to_string()))
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Sets inventory limit for an asset on an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `asset` - The asset symbol
    /// * `limit` - The inventory limit
    pub fn set_inventory_limit(
        &mut self,
        exchange: ExchangeId,
        asset: impl Into<String>,
        limit: Decimal,
    ) {
        self.inventory_limits
            .insert((exchange, asset.into()), limit);
    }

    /// Adds a fee schedule for an exchange.
    ///
    /// # Arguments
    ///
    /// * `schedule` - The fee schedule to add
    pub fn add_fee_schedule(&mut self, schedule: FeeSchedule) {
        self.fee_schedules.insert(schedule.exchange, schedule);
    }
}

/// Context for execution planning.
///
/// Provides parameters for executing validated signals.
/// All fields are public for direct access.
///
/// # Fields
///
/// - `available_balances`: Per-exchange, per-asset available balances
/// - `fee_schedules`: Exchange fee schedules
/// - `slippage_tolerance`: Maximum acceptable slippage (default: 0.1%)
/// - `max_order_size`: Maximum order size (default: $1,000)
///
/// # Example
///
/// ```rust
/// use arbitrage_core::strategies::ExecutionContext;
/// use arbitrage_core::types::ExchangeId;
/// use rust_decimal::Decimal;
///
/// let mut ctx = ExecutionContext::new();
///
/// // Set balances
/// ctx.set_balance(ExchangeId::Binance, "USDT", Decimal::from(50000));
/// ctx.set_balance(ExchangeId::ByBit, "BTC", Decimal::from(1));
///
/// // Check balance
/// let has_btc = ctx.has_sufficient_balance(ExchangeId::ByBit, "BTC", Decimal::from(1));
/// assert!(has_btc);
/// ```
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Available balances indexed by (exchange, asset)
    pub available_balances: HashMap<(ExchangeId, String), Decimal>,
    /// Exchange fee schedules
    pub fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    /// Maximum slippage tolerance
    pub slippage_tolerance: Decimal,
    /// Maximum order size
    pub max_order_size: Decimal,
}

impl ExecutionContext {
    /// Creates a new ExecutionContext with defaults.
    ///
    /// # Default Values
    ///
    /// - `slippage_tolerance`: 0.1%
    /// - `max_order_size`: $1,000
    pub fn new() -> Self {
        let default_slippage = Decimal::new(1, 3);

        Self {
            available_balances: HashMap::new(),
            fee_schedules: HashMap::new(),
            slippage_tolerance: default_slippage,
            max_order_size: Decimal::from(1000),
        }
    }

    /// Gets balance for an asset on an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `asset` - The asset symbol
    ///
    /// # Returns
    ///
    /// The available balance, or zero if not set.
    pub fn get_balance(&self, exchange: ExchangeId, asset: &str) -> Decimal {
        self.available_balances
            .get(&(exchange, asset.to_string()))
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Sets balance for an asset on an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `asset` - The asset symbol
    /// * `balance` - The balance to set
    pub fn set_balance(
        &mut self,
        exchange: ExchangeId,
        asset: impl Into<String>,
        balance: Decimal,
    ) {
        self.available_balances
            .insert((exchange, asset.into()), balance);
    }

    /// Checks if there's sufficient balance for a trade.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `asset` - The asset symbol
    /// * `required` - The required amount
    ///
    /// # Returns
    ///
    /// `true` if available balance >= required amount.
    pub fn has_sufficient_balance(
        &self,
        exchange: ExchangeId,
        asset: &str,
        required: Decimal,
    ) -> bool {
        self.get_balance(exchange, asset) >= required
    }

    /// Adds a fee schedule for an exchange.
    ///
    /// # Arguments
    ///
    /// * `schedule` - The fee schedule to add
    pub fn add_fee_schedule(&mut self, schedule: FeeSchedule) {
        self.fee_schedules.insert(schedule.exchange, schedule);
    }

    /// Gets fee schedule for an exchange.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    ///
    /// # Returns
    ///
    /// Reference to the fee schedule if available.
    pub fn get_fee_schedule(&self, exchange: ExchangeId) -> Option<&FeeSchedule> {
        self.fee_schedules.get(&exchange)
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Risk limits for strategy execution.
///
/// Provides configurable risk management parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskLimits {
    /// Maximum single position size in quote currency
    pub max_position_size: Decimal,
    /// Maximum daily trading volume
    pub max_daily_volume: Decimal,
    /// Maximum drawdown percentage
    pub max_drawdown: Decimal,
    /// Stop-loss threshold percentage
    pub stop_loss_threshold: Decimal,
}

impl Default for RiskLimits {
    fn default() -> Self {
        let max_drawdown = Decimal::new(5, 2);
        let stop_loss_threshold = Decimal::new(2, 2);

        Self {
            max_position_size: Decimal::from(5000),
            max_daily_volume: Decimal::from(50000),
            max_drawdown,
            stop_loss_threshold,
        }
    }
}

/// Strategy configuration parameters.
///
/// Contains all configurable settings for a strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// Whether the strategy is enabled
    pub enabled: bool,
    /// Minimum profit in basis points
    pub min_profit_bps: i32,
    /// Maximum total exposure
    pub max_exposure: Decimal,
    /// Confidence score threshold (0-100)
    pub confidence_threshold: Decimal,
    /// Risk management limits
    pub risk_limits: RiskLimits,
    /// Strategy-specific parameters stored as JSON
    ///
    /// Common parameters include:
    /// - `max_latency_ms`: Maximum data age
    /// - `vwap_quantity_usd`: VWAP quantity target
    /// - `inventory_based`: Whether to consider inventory
    pub custom_params: HashMap<String, serde_json::Value>,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        let confidence_threshold = Decimal::new(7, 1);

        Self {
            enabled: true,
            min_profit_bps: 10,
            max_exposure: Decimal::from(10000),
            confidence_threshold,
            risk_limits: RiskLimits::default(),
            custom_params: HashMap::new(),
        }
    }
}

/// Funding rate information for perpetual contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    /// Exchange for this funding rate
    pub exchange: ExchangeId,
    /// Trading symbol
    pub symbol: Symbol,
    /// Current funding rate
    pub rate: Decimal,
    /// Next funding timestamp
    pub next_funding: DateTime<Utc>,
    /// Predicted next funding rate
    pub predicted_rate: Option<Decimal>,
    /// Rate timestamp
    pub timestamp: DateTime<Utc>,
}

impl FundingRate {
    /// Creates a new FundingRate.
    pub fn new(
        exchange: ExchangeId,
        symbol: Symbol,
        rate: Decimal,
        next_funding: DateTime<Utc>,
    ) -> Self {
        Self {
            exchange,
            symbol,
            rate,
            next_funding,
            predicted_rate: None,
            timestamp: Utc::now(),
        }
    }

    /// Calculates the annualized funding rate.
    ///
    /// # Returns
    ///
    /// The rate multiplied by 365 (funding typically occurs 3x daily).
    pub fn annualized_rate(&self) -> Result<Decimal> {
        let daily_rate = self
            .rate
            .checked_mul(Decimal::from(3))
            .ok_or_else(|| ArbitrageError::Calculation("Funding rate overflow".to_string()))?;

        let annual_rate = daily_rate
            .checked_mul(Decimal::from(365))
            .ok_or_else(|| ArbitrageError::Calculation("Annual rate overflow".to_string()))?;

        Ok(annual_rate)
    }

    /// Checks if funding rate is reasonable.
    ///
    /// # Returns
    ///
    /// `true` if rate is between -100% and +100%.
    pub fn is_valid(&self) -> bool {
        let min_rate = Decimal::new(-100, 2);
        let max_rate = Decimal::new(100, 2);
        self.rate >= min_rate && self.rate <= max_rate
    }

    /// Checks if funding rate is positive.
    ///
    /// # Returns
    ///
    /// `true` if long holders receive funding.
    pub fn is_positive(&self) -> bool {
        self.rate > Decimal::ZERO
    }

    /// Gets time until next funding in seconds.
    ///
    /// # Returns
    ///
    /// Seconds until next funding, may be negative if past due.
    pub fn time_to_funding(&self) -> i64 {
        (self.next_funding - Utc::now()).num_seconds()
    }
}

/// Ticker information with price and volume data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    /// Exchange for this ticker
    pub exchange: ExchangeId,
    /// Trading symbol
    pub symbol: Symbol,
    /// Current best bid price
    pub bid: Decimal,
    /// Current best ask price
    pub ask: Decimal,
    /// Last traded price
    pub last: Decimal,
    /// 24-hour trading volume
    pub volume_24h: Decimal,
    /// 24-hour price change
    pub change_24h: Decimal,
    /// Ticker timestamp
    pub timestamp: DateTime<Utc>,
}

impl Ticker {
    /// Creates a new Ticker.
    pub fn new(
        exchange: ExchangeId,
        symbol: Symbol,
        bid: Decimal,
        ask: Decimal,
        last: Decimal,
    ) -> Self {
        Self {
            exchange,
            symbol,
            bid,
            ask,
            last,
            volume_24h: Decimal::ZERO,
            change_24h: Decimal::ZERO,
            timestamp: Utc::now(),
        }
    }

    /// Calculates the spread.
    ///
    /// # Returns
    ///
    /// Ask price minus bid price.
    pub fn spread(&self) -> Decimal {
        self.ask - self.bid
    }

    /// Calculates the mid-price.
    ///
    /// # Returns
    ///
    /// Average of bid and ask.
    ///
    /// # Errors
    ///
    /// Returns an error if both bid and ask are zero.
    pub fn mid_price(&self) -> Result<Decimal> {
        if self.bid.is_zero() && self.ask.is_zero() {
            return Err(ArbitrageError::Calculation(
                "Both bid and ask are zero".to_string(),
            ));
        }

        Ok((self.bid + self.ask) / Decimal::from(2))
    }

    /// Calculates spread in basis points.
    ///
    /// # Returns
    ///
    /// Spread as a percentage in basis points.
    ///
    /// # Errors
    ///
    /// Returns an error if mid-price calculation fails.
    pub fn spread_bps(&self) -> Result<Decimal> {
        let mid = self.mid_price()?;

        let spread_ratio = self.spread().checked_div(mid).ok_or_else(|| {
            ArbitrageError::Calculation("Division by zero in spread calculation".to_string())
        })?;

        Ok(spread_ratio * Decimal::from(10000))
    }

    /// Validates that bid < ask.
    ///
    /// # Returns
    ///
    /// `true` if bid is less than ask and both are non-zero.
    pub fn is_valid(&self) -> bool {
        self.bid < self.ask && !self.bid.is_zero() && !self.ask.is_zero()
    }
}

/// Fee schedule for an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeSchedule {
    /// The exchange
    pub exchange: ExchangeId,
    /// Maker fee rate
    pub maker_fee: Decimal,
    /// Taker fee rate
    pub taker_fee: Decimal,
    /// Optional fee tier
    pub tier: Option<String>,
}

impl FeeSchedule {
    /// Creates a new FeeSchedule.
    pub fn new(exchange: ExchangeId, maker_fee: Decimal, taker_fee: Decimal) -> Self {
        Self {
            exchange,
            maker_fee,
            taker_fee,
            tier: None,
        }
    }

    /// Calculates the fee for a notional value.
    ///
    /// # Arguments
    ///
    /// * `notional` - The trade notional value
    /// * `is_maker` - True for maker, false for taker
    ///
    /// # Returns
    ///
    /// The fee amount.
    ///
    /// # Errors
    ///
    /// Returns an error on overflow.
    pub fn calculate_fee(&self, notional: Decimal, is_maker: bool) -> Result<Decimal> {
        let rate = if is_maker {
            self.maker_fee
        } else {
            self.taker_fee
        };

        notional
            .checked_mul(rate)
            .ok_or_else(|| ArbitrageError::Calculation("Fee calculation overflow".to_string()))
    }
}

/// Macro to create strategy new() function with default config.
#[macro_export]
macro_rules! strategy_new {
    ($type:ident, $strategy_id:expr) => {
        impl $type {
            pub fn new() -> Self {
                let config = StrategyConfig {
                    min_profit_bps: StrategyLimits::get_min_profit_bps($strategy_id),
                    max_exposure: StrategyLimits::get_max_exposure($strategy_id),
                    custom_params: StrategyUtils::create_base_custom_params(),
                    ..Default::default()
                };
                Self { config }
            }
        }
    };
}

/// Macro to implement Default for strategy using strategy_new!.
#[macro_export]
macro_rules! strategy_default {
    ($type:ident) => {
        impl Default for $type {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}
