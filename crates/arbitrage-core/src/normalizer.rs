//! Exchange data normalization utilities.
//!
//! This module provides utilities for normalizing exchange-specific data
//! to a canonical format, including symbol mapping, precision handling,
//! and order book normalization.
//!
//! # Normalization Process
//!
//! 1. **Symbol Mapping**: Convert exchange-specific symbols to canonical format
//! 2. **Precision Handling**: Format quantities to exchange-specific decimals
//! 3. **Order Book Processing**: Sort and validate order book data
//! 4. **Timestamp Handling**: Normalize timestamps to UTC
//!
//! # Example
//!
//! ```rust
//! use arbitrage_core::normalizer::Normalizer;
//! use arbitrage_core::types::{ExchangeId, Symbol};
//!
//! let normalizer = Normalizer::new();
///
/// // Map exchange symbol to canonical
/// let btc = normalizer.map_symbol(ExchangeId::Binance, "BTCUSDT");
/// ```
use crate::{
    config::{StablecoinGroup, SymbolMapping},
    types::{ExchangeId, FeeSchedule, OrderBook, OrderBookLevel, Symbol},
    ArbitrageError, Result,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Normalizes exchange-specific data to canonical format.
///
/// Handles symbol mapping, precision conversion, and data validation
/// for consistent processing across multiple exchanges.
///
/// # Capabilities
///
/// - Map exchange-specific symbols to canonical format
/// - Format quantities to exchange-specific precision
/// - Normalize and validate order books
/// - Check symbol equivalence (e.g., stablecoins)
/// - Manage fee schedules
///
/// # Example
///
/// ```rust
/// use arbitrage_core::normalizer::Normalizer;
/// use arbitrage_core::types::{ExchangeId, Symbol};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let normalizer = Normalizer::new();
///
/// // Normalize an order book
/// let order_book = normalizer.normalize_order_book(
///     ExchangeId::Binance,
///     "BTCUSDT",
///     vec![(Decimal::from(50000), Decimal::from(1))],
///     vec![(Decimal::from(50001), Decimal::from(1))],
///     Some(Utc::now()),
///     None,
/// );
/// ```
#[derive(Debug, Clone)]
pub struct Normalizer {
    symbol_mappings: HashMap<Symbol, SymbolMapping>,
    exchange_symbol_index: HashMap<(ExchangeId, String), Symbol>,
    fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    /// Stablecoin groups for normalization
    /// TODO: Replace with external stablecoin classification service
    stablecoin_groups: Vec<StablecoinGroup>,
    stablecoin_index: HashMap<String, Vec<String>>,
}

impl Normalizer {
    /// Creates a new Normalizer with default settings.
    pub fn new() -> Self {
        let stablecoin_groups = vec![StablecoinGroup::default()];
        let stablecoin_index = Self::build_stablecoin_index(&stablecoin_groups);

        Self {
            symbol_mappings: HashMap::with_capacity(1024),
            exchange_symbol_index: HashMap::with_capacity(2048),
            fee_schedules: HashMap::with_capacity(32),
            stablecoin_groups,
            stablecoin_index,
        }
    }

    fn build_stablecoin_index(groups: &[StablecoinGroup]) -> HashMap<String, Vec<String>> {
        let mut index: HashMap<String, Vec<String>> = HashMap::with_capacity(groups.len() * 16);
        for group in groups {
            let group_upper: Vec<String> = group.symbols.iter().map(|s| s.to_uppercase()).collect();
            for symbol_upper in &group_upper {
                index.insert(symbol_upper.clone(), group_upper.clone());
            }
        }
        index
    }

    /// Loads symbol mappings from configuration.
    ///
    /// # Arguments
    ///
    /// * `mappings` - Vector of symbol mappings to load
    pub fn load_symbol_mappings(&mut self, mappings: Vec<SymbolMapping>) {
        self.symbol_mappings.reserve(mappings.len());
        self.exchange_symbol_index.reserve(mappings.len() * 4);

        for mapping in mappings {
            let canonical = mapping.canonical.clone();
            for (exchange, exchange_symbol) in &mapping.exchange_symbols {
                self.exchange_symbol_index
                    .insert((*exchange, exchange_symbol.clone()), canonical.clone());
            }
            self.symbol_mappings.insert(canonical, mapping);
        }
    }

    /// Loads fee schedules for exchanges.
    ///
    /// # Arguments
    ///
    /// * `schedules` - Vector of fee schedules to load
    pub fn load_fee_schedules(&mut self, schedules: Vec<FeeSchedule>) {
        self.fee_schedules.reserve(schedules.len());
        for schedule in schedules {
            self.fee_schedules.insert(schedule.exchange, schedule);
        }
    }

    /// Maps an exchange-specific symbol to canonical format.
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `exchange_symbol` - The exchange-specific symbol string
    ///
    /// # Returns
    ///
    /// `Some(Symbol)` if a mapping exists, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::normalizer::Normalizer;
    /// use arbitrage_core::types::ExchangeId;
    ///
    /// let normalizer = Normalizer::new();
    /// // With appropriate symbol mappings loaded:
    /// // let canonical = normalizer.map_symbol(ExchangeId::Binance, "BTCUSDT");
    /// ```
    pub fn map_symbol(&self, exchange: ExchangeId, exchange_symbol: &str) -> Option<Symbol> {
        self.exchange_symbol_index
            .get(&(exchange, exchange_symbol.to_string()))
            .cloned()
    }

    /// Gets exchange-specific symbol from canonical.
    ///
    /// # Arguments
    ///
    /// * `canonical` - The canonical symbol
    /// * `exchange` - The target exchange
    ///
    /// # Returns
    ///
    /// The exchange-specific symbol string if found.
    pub fn get_exchange_symbol(&self, canonical: &Symbol, exchange: ExchangeId) -> Option<String> {
        self.symbol_mappings
            .get(canonical)?
            .exchange_symbols
            .get(&exchange)
            .cloned()
    }

    /// Gets minimum precision for a symbol across exchanges.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The canonical symbol
    ///
    /// # Returns
    ///
    /// The minimum decimal precision if mapping exists.
    pub fn get_min_precision(&self, symbol: &Symbol) -> Option<u32> {
        let mapping = self.symbol_mappings.get(symbol)?;
        mapping.precision.values().min().copied()
    }

    /// Gets count of loaded symbol mappings.
    ///
    /// # Returns
    ///
    /// The number of symbol mappings loaded.
    pub fn get_symbol_mapping_count(&self) -> usize {
        self.symbol_mappings.len()
    }

    /// Checks if two symbols are equivalent.
    ///
    /// # Arguments
    ///
    /// * `symbol1` - First symbol string
    /// * `symbol2` - Second symbol string
    ///
    /// # Returns
    ///
    /// `true` if symbols are equivalent (same or in same stablecoin group).
    pub fn check_symbols_equivalent(&self, symbol1: &str, symbol2: &str) -> bool {
        self.are_symbols_equivalent(symbol1, symbol2)
    }

    /// Formats quantity to exchange-specific precision.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `exchange` - The exchange
    /// * `quantity` - The quantity to format
    ///
    /// # Returns
    ///
    /// Quantity rounded to exchange precision.
    ///
    /// # Errors
    ///
    /// Returns error if no mapping exists for the symbol/exchange.
    pub fn format_quantity(
        &self,
        symbol: &Symbol,
        exchange: ExchangeId,
        quantity: Decimal,
    ) -> Result<Decimal> {
        let mapping = self.symbol_mappings.get(symbol).ok_or_else(|| {
            ArbitrageError::Normalization(format!("No mapping for symbol {}", symbol))
        })?;

        let precision = mapping.precision.get(&exchange).ok_or_else(|| {
            ArbitrageError::Normalization(format!("No precision for {} on {}", symbol, exchange))
        })?;

        let scale = 10_u64.pow(*precision);
        let scaled = quantity * Decimal::from(scale);
        let rounded = scaled.round();
        Ok(rounded / Decimal::from(scale))
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

    /// Checks if two symbols are equivalent.
    ///
    /// Symbols are considered equivalent if:
    /// 1. They are identical (case-insensitive)
    /// 2. Both are in the same stablecoin group
    ///
    /// # Arguments
    ///
    /// * `symbol1` - First symbol string
    /// * `symbol2` - Second symbol string
    ///
    /// # Returns
    ///
    /// `true` if symbols are equivalent.
    pub fn are_symbols_equivalent(&self, symbol1: &str, symbol2: &str) -> bool {
        let sym1 = symbol1.to_uppercase();
        let sym2 = symbol2.to_uppercase();

        if sym1 == sym2 {
            return true;
        }

        if let Some(group1) = self.stablecoin_index.get(&sym1) {
            return group1.contains(&sym2);
        }

        false
    }

    /// Normalizes order book from exchange format.
    ///
    /// Converts raw exchange data to a canonical OrderBook:
    /// 1. Maps symbol to canonical format
    /// 2. Converts to OrderBookLevel structs
    /// 3. Sorts bids (descending) and asks (ascending)
    /// 4. Validates order book integrity
    ///
    /// # Arguments
    ///
    /// * `exchange` - The exchange
    /// * `exchange_symbol` - Exchange-specific symbol string
    /// * `bids` - Raw bid data (price, quantity) pairs
    /// * `asks` - Raw ask data (price, quantity) pairs
    /// * `timestamp` - Optional timestamp, defaults to now
    /// * `sequence` - Optional sequence number
    ///
    /// # Returns
    ///
    /// Normalized OrderBook ready for arbitrage analysis.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Symbol mapping doesn't exist
    /// - Order book fails validation (bid >= ask, unsorted)
    ///
    /// # Example
    ///
    /// ```rust
    /// use arbitrage_core::normalizer::Normalizer;
    /// use arbitrage_core::types::ExchangeId;
    /// use rust_decimal::Decimal;
    /// use chrono::Utc;
    ///
    /// let normalizer = Normalizer::new();
    ///
    /// let bids = vec![
    ///     (Decimal::from(50000), Decimal::from(1)),
    ///     (Decimal::from(49999), Decimal::from(2)),
    /// ];
    /// let asks = vec![
    ///     (Decimal::from(50001), Decimal::from(1)),
    ///     (Decimal::from(50002), Decimal::from(2)),
    /// ];
    ///
    /// // With symbol mappings configured:
    /// // let order_book = normalizer.normalize_order_book(
    /// //     ExchangeId::Binance,
    /// //     "BTCUSDT",
    /// //     bids,
    /// //     asks,
    /// //     Some(Utc::now()),
    /// //     None,
    /// // );
    /// ```
    pub fn normalize_order_book(
        &self,
        exchange: ExchangeId,
        exchange_symbol: &str,
        bids: Vec<(Decimal, Decimal)>,
        asks: Vec<(Decimal, Decimal)>,
        timestamp: Option<DateTime<Utc>>,
        sequence: Option<u64>,
    ) -> Result<OrderBook> {
        let symbol = self.map_symbol(exchange, exchange_symbol).ok_or_else(|| {
            ArbitrageError::Normalization(format!(
                "Unknown symbol {} on {}",
                exchange_symbol, exchange
            ))
        })?;

        let mut bid_levels: Vec<OrderBookLevel> = Vec::with_capacity(bids.len());
        for (price, qty) in bids {
            bid_levels.push(OrderBookLevel::new(price, qty));
        }

        let mut ask_levels: Vec<OrderBookLevel> = Vec::with_capacity(asks.len());
        for (price, qty) in asks {
            ask_levels.push(OrderBookLevel::new(price, qty));
        }

        bid_levels.sort_by(|a, b| b.price.cmp(&a.price));
        ask_levels.sort_by(|a, b| a.price.cmp(&b.price));

        let order_book = OrderBook {
            exchange,
            symbol,
            bids: bid_levels,
            asks: ask_levels,
            timestamp: timestamp.unwrap_or_else(Utc::now),
            sequence,
        };

        if !order_book.is_valid() {
            return Err(ArbitrageError::Normalization(
                "Invalid order book: bid >= ask or unsorted levels".to_string(),
            ));
        }

        Ok(order_book)
    }

    /// Normalizes timestamp to UTC milliseconds.
    ///
    /// # Arguments
    ///
    /// * `timestamp_ms` - Timestamp in milliseconds since epoch
    ///
    /// # Returns
    ///
    /// DateTime in UTC, or current time if input is invalid.
    pub fn normalize_timestamp(&self, timestamp_ms: i64) -> DateTime<Utc> {
        match DateTime::from_timestamp_millis(timestamp_ms) {
            Some(dt) => dt,
            None => {
                tracing::warn!(
                    timestamp_ms = timestamp_ms,
                    "Invalid timestamp received, using current time as fallback"
                );
                Utc::now()
            }
        }
    }

    /// Validates minimum order requirements.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `exchange` - The exchange
    /// * `quantity` - Order quantity
    /// * `price` - Order price
    ///
    /// # Returns
    ///
    /// `Ok(())` if requirements are met.
    ///
    /// # Errors
    ///
    /// Returns error if quantity or notional is below minimum.
    pub fn validate_order_size(
        &self,
        symbol: &Symbol,
        exchange: ExchangeId,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<()> {
        let mapping = self.symbol_mappings.get(symbol).ok_or_else(|| {
            ArbitrageError::Validation(format!("No mapping for symbol {}", symbol))
        })?;

        if let Some(min_qty) = mapping.min_quantity.get(&exchange) {
            if quantity < *min_qty {
                return Err(ArbitrageError::Validation(format!(
                    "Quantity {} below minimum {} for {} on {}",
                    quantity, min_qty, symbol, exchange
                )));
            }
        }

        if let Some(min_notional) = mapping.min_notional.get(&exchange) {
            let notional = quantity * price;
            if notional < *min_notional {
                return Err(ArbitrageError::Validation(format!(
                    "Notional value {} below minimum {} for {} on {}",
                    notional, min_notional, symbol, exchange
                )));
            }
        }

        Ok(())
    }
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}
