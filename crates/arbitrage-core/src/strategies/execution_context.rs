use crate::{types::FeeSchedule, ExchangeId, Symbol};
use rust_decimal::Decimal;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub available_balances: HashMap<(ExchangeId, String), Decimal>,
    pub fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    pub slippage_tolerance: Decimal,
    pub max_order_size: Decimal,
}

impl ExecutionContext {
    pub fn new() -> Self {
        let default_slippage = Decimal::new(1, 3);

        Self {
            available_balances: HashMap::new(),
            fee_schedules: HashMap::new(),
            slippage_tolerance: default_slippage,
            max_order_size: Decimal::from(1000),
        }
    }

    pub fn get_balance(&self, exchange: ExchangeId, asset: &str) -> Decimal {
        self.available_balances
            .get(&(exchange, asset.to_string()))
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    pub fn set_balance(
        &mut self,
        exchange: ExchangeId,
        asset: impl Into<String>,
        balance: Decimal,
    ) {
        self.available_balances
            .insert((exchange, asset.into()), balance);
    }

    pub fn has_sufficient_balance(
        &self,
        exchange: ExchangeId,
        asset: &str,
        required: Decimal,
    ) -> bool {
        self.get_balance(exchange, asset) >= required
    }

    pub fn add_fee_schedule(&mut self, schedule: FeeSchedule) {
        self.fee_schedules.insert(schedule.exchange, schedule);
    }

    pub fn get_fee_schedule(&self, exchange: ExchangeId) -> Option<&FeeSchedule> {
        self.fee_schedules.get(&exchange)
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}
