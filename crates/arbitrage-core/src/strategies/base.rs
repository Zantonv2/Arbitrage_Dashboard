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
//!
//! # Available Types
//!
//! This module provides access to the following types (re-exported from submodules):
//! - [`traits::Strategy`] - Core strategy trait
//! - [`traits::ArbitrageStrategy`] - Extended strategy trait with defaults
//! - [`filter_context::FilterContext`] - Signal filtering context
//! - [`execution_context::ExecutionContext`] - Execution parameters
//! - [`market_types::MarketBundle`] - Market data container
//! - [`market_types::RawSignal`] - Raw signal before validation
//! - [`market_types::Ticker`] - Ticker price data
//! - [`market_types::FundingRate`] - Funding rate data
//! - [`market_types::TradeLeg`] - Individual trade leg
//!
//! These types are also available via [`crate::strategies`] through re-exports in the parent module.
