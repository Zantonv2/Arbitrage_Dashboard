use crate::strategies::RiskLimits;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub enabled: bool,
    pub min_profit_bps: i32,
    pub max_exposure: Decimal,
    pub confidence_threshold: Decimal,
    pub risk_limits: RiskLimits,
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
