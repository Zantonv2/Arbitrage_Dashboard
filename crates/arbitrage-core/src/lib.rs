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

pub use error::{ArbitrageError, Result};
pub use types::*;
