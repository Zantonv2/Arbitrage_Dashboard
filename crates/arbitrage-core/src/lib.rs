pub mod types;
pub mod normalizer;
pub mod arbitrage_engine;
pub mod confidence_scorer;
pub mod size_calculator;
pub mod execution_preparer;
pub mod simulation;
pub mod storage;
pub mod config;
pub mod error;

// Russian Financial Advisor Strategies (8-22% daily returns)
pub mod strategies;

pub use types::*;
pub use error::{ArbitrageError, Result};