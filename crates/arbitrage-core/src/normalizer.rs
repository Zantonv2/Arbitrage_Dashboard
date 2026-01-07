use crate::{
    config::{SymbolMapping, StablecoinGroup},
    types::{ExchangeId, FeeSchedule, OrderBook, OrderBookLevel, Symbol},
    ArbitrageError, Result,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Normalizes exchange-specific data to canonical format
pub struct Normalizer {
    symbol_mappings: HashMap<Symbol, SymbolMapping>,
    fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    stablecoin_groups: Vec<StablecoinGroup>,
}

impl Normalizer {
    pub fn new() -> Self {
        Self {
            symbol_mappings: HashMap::new(),
            fee_schedules: HashMap::new(),
            stablecoin_groups: vec![StablecoinGroup::default()],
        }
    }

    /// Load symbol mappings from configuration
    pub fn load_symbol_mappings(&mut self, mappings: Vec<SymbolMapping>) {
        for mapping in mappings {
            self.symbol_mappings.insert(mapping.canonical.clone(), mapping);
        }
    }

    /// Load fee schedules for exchanges
    pub fn load_fee_schedules(&mut self, schedules: Vec<FeeSchedule>) {
        for schedule in schedules {
            self.fee_schedules.insert(schedule.exchange, schedule);
        }
    }

    /// Map exchange-specific symbol to canonical format
    pub fn map_symbol(&self, exchange: ExchangeId, exchange_symbol: &str) -> Option<Symbol> {
        // Find mapping where exchange_symbol matches for this exchange
        for (canonical, mapping) in &self.symbol_mappings {
            if let Some(mapped_symbol) = mapping.exchange_symbols.get(&exchange) {
                if mapped_symbol == exchange_symbol {
                    return Some(canonical.clone());
                }
            }
        }
        None
    }

    /// Get exchange-specific symbol from canonical
    pub fn get_exchange_symbol(&self, canonical: &Symbol, exchange: ExchangeId) -> Option<String> {
        self.symbol_mappings
            .get(canonical)?
            .exchange_symbols
            .get(&exchange)
            .cloned()
    }

    /// Get minimum precision for a symbol across exchanges
    pub fn get_min_precision(&self, symbol: &Symbol) -> Option<u32> {
        let mapping = self.symbol_mappings.get(symbol)?;
        mapping.precision.values().min().copied()
    }

    /// Format quantity to exchange-specific precision
    pub fn format_quantity(&self, symbol: &Symbol, exchange: ExchangeId, quantity: Decimal) -> Result<Decimal> {
        let mapping = self.symbol_mappings
            .get(symbol)
            .ok_or_else(|| ArbitrageError::Normalization(format!("No mapping for symbol {}", symbol)))?;

        let precision = mapping.precision
            .get(&exchange)
            .ok_or_else(|| ArbitrageError::Normalization(format!("No precision for {} on {}", symbol, exchange)))?;

        let scale = 10_u64.pow(*precision);
        let scaled = quantity * Decimal::from(scale);
        let rounded = scaled.round();
        Ok(rounded / Decimal::from(scale))
    }

    /// Get fee schedule for exchange
    pub fn get_fee_schedule(&self, exchange: ExchangeId) -> Option<&FeeSchedule> {
        self.fee_schedules.get(&exchange)
    }

    /// Check if two symbols are equivalent (e.g., stablecoins)
    pub fn are_symbols_equivalent(&self, symbol1: &str, symbol2: &str) -> bool {
        // Normalize to uppercase for comparison
        let sym1 = symbol1.to_uppercase();
        let sym2 = symbol2.to_uppercase();
        
        if sym1 == sym2 {
            return true;
        }

        // Check if both symbols are in the same stablecoin group
        for group in &self.stablecoin_groups {
            let group_upper: Vec<String> = group.symbols.iter().map(|s| s.to_uppercase()).collect();
            if group_upper.contains(&sym1) && group_upper.contains(&sym2) {
                return true;
            }
        }

        false
    }

    /// Normalize order book from exchange format
    pub fn normalize_order_book(
        &self,
        exchange: ExchangeId,
        exchange_symbol: &str,
        bids: Vec<(Decimal, Decimal)>,
        asks: Vec<(Decimal, Decimal)>,
        timestamp: Option<DateTime<Utc>>,
        sequence: Option<u64>,
    ) -> Result<OrderBook> {
        // Map symbol to canonical format
        let symbol = self.map_symbol(exchange, exchange_symbol)
            .ok_or_else(|| ArbitrageError::Normalization(
                format!("Unknown symbol {} on {}", exchange_symbol, exchange)
            ))?;

        // Convert to OrderBookLevel structs and sort
        // Note: For high-frequency trading with large order books, 
        // consider pre-sorted data or incremental updates to avoid sorting overhead
        let mut bid_levels: Vec<OrderBookLevel> = bids
            .into_iter()
            .map(|(price, qty)| OrderBookLevel::new(price, qty))
            .collect();
        
        let mut ask_levels: Vec<OrderBookLevel> = asks
            .into_iter()
            .map(|(price, qty)| OrderBookLevel::new(price, qty))
            .collect();

        // Sort bids descending (highest first), asks ascending (lowest first)
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

        // Validate the order book
        if !order_book.is_valid() {
            return Err(ArbitrageError::Normalization(
                "Invalid order book: bid >= ask or unsorted levels".to_string()
            ));
        }

        Ok(order_book)
    }

    /// Normalize timestamp to UTC milliseconds
    pub fn normalize_timestamp(&self, timestamp_ms: i64) -> DateTime<Utc> {
        match DateTime::from_timestamp_millis(timestamp_ms) {
            Some(dt) => dt,
            None => {
                // Log problematic timestamp for debugging
                tracing::warn!(
                    timestamp_ms = timestamp_ms,
                    "Invalid timestamp received, using current time as fallback"
                );
                Utc::now()
            }
        }
    }

    /// Validate minimum order requirements
    pub fn validate_order_size(
        &self,
        symbol: &Symbol,
        exchange: ExchangeId,
        quantity: Decimal,
        price: Decimal,
    ) -> Result<()> {
        let mapping = self.symbol_mappings
            .get(symbol)
            .ok_or_else(|| ArbitrageError::Validation(format!("No mapping for symbol {}", symbol)))?;

        // Check minimum quantity
        if let Some(min_qty) = mapping.min_quantity.get(&exchange) {
            if quantity < *min_qty {
                return Err(ArbitrageError::Validation(
                    format!("Quantity {} below minimum {} for {} on {}", 
                           quantity, min_qty, symbol, exchange)
                ));
            }
        }

        // Check minimum notional value
        if let Some(min_notional) = mapping.min_notional.get(&exchange) {
            let notional = quantity * price;
            if notional < *min_notional {
                return Err(ArbitrageError::Validation(
                    format!("Notional value {} below minimum {} for {} on {}", 
                           notional, min_notional, symbol, exchange)
                ));
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