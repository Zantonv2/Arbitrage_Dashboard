use crate::{types::FeeSchedule, ExchangeId, Symbol};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FilterContext {
    pub min_profit_bps: i32,
    pub max_exposure: Decimal,
    pub allowed_exchanges: Vec<ExchangeId>,
    pub risk_limits: RiskLimits,
    pub fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    pub max_latency_ms: u64,
    pub min_notional_usd: Decimal,
    pub inventory_limits: HashMap<(ExchangeId, String), Decimal>,
}

impl FilterContext {
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

    pub fn get_fee_schedule(&self, exchange: ExchangeId) -> Option<&FeeSchedule> {
        self.fee_schedules.get(&exchange)
    }

    pub fn can_sell(&self, exchange: ExchangeId, asset: &str, quantity: Decimal) -> bool {
        let key = (exchange, asset.to_string());
        let available = self
            .inventory_limits
            .get(&key)
            .copied()
            .unwrap_or(Decimal::ZERO);

        available >= quantity
    }

    pub fn is_exchange_allowed(&self, exchange: ExchangeId) -> bool {
        self.allowed_exchanges.contains(&exchange)
    }

    pub fn are_exchanges_allowed(&self, exchange1: ExchangeId, exchange2: ExchangeId) -> bool {
        self.is_exchange_allowed(exchange1) && self.is_exchange_allowed(exchange2)
    }

    pub fn get_inventory(&self, exchange: ExchangeId, asset: &str) -> Decimal {
        self.inventory_limits
            .get(&(exchange, asset.to_string()))
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    pub fn set_inventory_limit(
        &mut self,
        exchange: ExchangeId,
        asset: impl Into<String>,
        limit: Decimal,
    ) {
        self.inventory_limits
            .insert((exchange, asset.into()), limit);
    }

    pub fn add_fee_schedule(&mut self, schedule: FeeSchedule) {
        self.fee_schedules.insert(schedule.exchange, schedule);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: Decimal,
    pub max_daily_volume: Decimal,
    pub max_drawdown: Decimal,
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
