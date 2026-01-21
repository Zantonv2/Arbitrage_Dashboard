pub mod arbitrage_engine;
pub mod confidence_scorer;
pub mod config;
pub mod error;
pub mod execution_preparer;
pub mod market_utils;
pub mod normalizer;
pub mod simulation;
pub mod size_calculator;
pub mod storage;
pub mod symbol_discovery;
pub mod symbol_manager;
pub mod test_utils;
pub mod types;

// Strategies folder (Potential 8-22% profit)
pub mod strategies;

#[cfg(test)]
pub mod property_tests;

#[cfg(test)]
pub mod unit_tests;

#[cfg(test)]
mod confidence_scorer_tests;

#[cfg(test)]
mod normalizer_tests;

#[cfg(test)]
mod execution_preparer_tests;

#[cfg(test)]
mod size_calculator_tests;

#[cfg(test)]
mod strategy_registry_tests;

#[cfg(test)]
mod symbol_discovery_tests;

#[cfg(test)]
mod ticker_funding_rate_tests;

#[cfg(test)]
mod storage_tests;

pub use error::{ArbitrageError, Result};
pub use types::*;
