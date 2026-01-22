use crate::{types::ExecutionInstruction, ArbitrageError, ExchangeId, Result, Signal};

const DEFAULT_MAX_DATA_AGE_MS: u64 = 5000;

pub trait Strategy: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn detect(&self, market_data: &super::MarketBundle) -> Result<Vec<super::RawSignal>>;
    fn filter(&self, signal: &super::RawSignal, context: &super::FilterContext) -> Result<bool>;
    fn check_exchange_health(
        &self,
        signal: &super::RawSignal,
        market_bundle: &super::MarketBundle,
    ) -> Result<bool> {
        let exchanges = signal.get_exchanges();
        let mut unhealthy_exchanges: Vec<ExchangeId> = Vec::new();

        for &exchange in &exchanges {
            if !market_bundle.is_healthy(exchange, &signal.symbol) {
                unhealthy_exchanges.push(exchange);
            }
        }

        if !unhealthy_exchanges.is_empty() {
            tracing::warn!(
                target: "exchange_health",
                "Signal {} rejected: unhealthy exchanges {:?}",
                signal.strategy_id, unhealthy_exchanges
            );
            return Ok(false);
        }

        Ok(true)
    }
    fn plan(
        &self,
        signal: &Signal,
        context: &super::ExecutionContext,
    ) -> Result<ExecutionInstruction> {
        let _ = (signal, context);
        Err(ArbitrageError::Configuration(
            "Strategy execution planning is deprecated - use ExecutionPreparer instead".to_string(),
        ))
    }
    fn config(&self) -> &super::StrategyConfig;
    fn update_config(&mut self, config: super::StrategyConfig) -> Result<()>;
}

pub trait ArbitrageStrategy: Strategy {
    fn expected_leg_count(&self) -> usize {
        2
    }

    fn validate_signal(
        &self,
        signal: &super::RawSignal,
        context: &super::FilterContext,
        market_bundle: &super::MarketBundle,
    ) -> Result<bool> {
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

        if !self.validate_exchange_health(signal, market_bundle)? {
            return Ok(false);
        }

        self.validate_inventory(signal, context)
    }

    fn validate_exchange_health(
        &self,
        signal: &super::RawSignal,
        market_bundle: &super::MarketBundle,
    ) -> Result<bool> {
        self.check_exchange_health(signal, market_bundle)
    }

    fn validate_inventory(
        &self,
        _signal: &super::RawSignal,
        _context: &super::FilterContext,
    ) -> Result<bool> {
        Ok(true)
    }
}
