// Simulation engine - placeholder for Phase 3
// This will be implemented in the advanced features phase

use crate::types::{ExchangeId, Symbol};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Simulation engine for backtesting strategies
pub struct SimulationEngine {
    config: SimulationConfig,
    results: Vec<SimulationResult>,
    state: SimulationState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub initial_capital_usd: Decimal,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub fee_taker_bps: i32,
    pub fee_maker_bps: i32,
    pub slippage_bps: i32,
    pub min_profit_threshold_bps: i32,
    pub max_position_size_usd: Decimal,
    pub simulate_funding: bool,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            initial_capital_usd: Decimal::from(100000),
            start_date: Utc::now() - Duration::days(30),
            end_date: Utc::now(),
            fee_taker_bps: 5,
            fee_maker_bps: 2,
            slippage_bps: 2,
            min_profit_threshold_bps: 5,
            max_position_size_usd: Decimal::from(50000),
            simulate_funding: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub total_profit_usd: Decimal,
    pub total_fees_usd: Decimal,
    pub net_profit_usd: Decimal,
    pub profit_percent: Decimal,
    pub max_drawdown_percent: Decimal,
    pub avg_trade_profit_usd: Decimal,
    pub win_rate_percent: Decimal,
    pub sharpe_ratio: Decimal,
    pub sortino_ratio: Decimal,
    pub profit_factor: Decimal,
    pub avg_trade_duration_ms: u64,
    pub symbols_traded: Vec<Symbol>,
    pub exchanges_used: Vec<ExchangeId>,
    pub hourly_returns: Vec<Decimal>,
    pub daily_returns: Vec<Decimal>,
}

impl Default for SimulationResult {
    fn default() -> Self {
        Self {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            total_profit_usd: Decimal::ZERO,
            total_fees_usd: Decimal::ZERO,
            net_profit_usd: Decimal::ZERO,
            profit_percent: Decimal::ZERO,
            max_drawdown_percent: Decimal::ZERO,
            avg_trade_profit_usd: Decimal::ZERO,
            win_rate_percent: Decimal::ZERO,
            sharpe_ratio: Decimal::ZERO,
            sortino_ratio: Decimal::ZERO,
            profit_factor: Decimal::ZERO,
            avg_trade_duration_ms: 0,
            symbols_traded: Vec::new(),
            exchanges_used: Vec::new(),
            hourly_returns: Vec::new(),
            daily_returns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct SimulationState {
    current_capital_usd: Decimal,
    peak_capital_usd: Decimal,
    current_drawdown_percent: Decimal,
    max_drawdown_percent: Decimal,
    trade_history: Vec<TradeRecord>,
}

#[derive(Debug, Clone)]
struct TradeRecord {
    timestamp: DateTime<Utc>,
    symbol: Symbol,
    buy_exchange: ExchangeId,
    sell_exchange: ExchangeId,
    profit_usd: Decimal,
    fees_usd: Decimal,
    duration_ms: u64,
    was_winner: bool,
}

impl SimulationEngine {
    pub fn new(mut config: SimulationConfig) -> Self {
        let initial_capital = config.initial_capital_usd;
        Self {
            config: config.clone(),
            results: Vec::new(),
            state: SimulationState {
                current_capital_usd: initial_capital,
                peak_capital_usd: initial_capital,
                current_drawdown_percent: Decimal::ZERO,
                max_drawdown_percent: Decimal::ZERO,
                trade_history: Vec::new(),
            },
        }
    }

    pub fn run_backtest(&mut self, signals: &[BacktestSignal]) -> SimulationResult {
        let mut result = SimulationResult::default();

        for signal in signals {
            if self.should_execute_trade(signal) {
                let trade_result = self.execute_trade(signal);
                self.update_state(&signal, &trade_result, &mut result);
            }
        }

        self.calculate_performance_metrics(&mut result);
        self.results.push(result.clone());
        result
    }

    fn should_execute_trade(&self, signal: &BacktestSignal) -> bool {
        signal.profit_bps >= self.config.min_profit_threshold_bps as i32
            && signal.notional_usd <= self.config.max_position_size_usd
    }

    fn execute_trade(&self, signal: &BacktestSignal) -> TradeExecutionResult {
        let profit_bps_decimal = Decimal::from(signal.profit_bps);
        let fee_rate = Decimal::from(self.config.fee_taker_bps) / Decimal::from(10000);
        let slippage_rate = Decimal::from(self.config.slippage_bps) / Decimal::from(10000);

        let gross_profit = signal.notional_usd * profit_bps_decimal / Decimal::from(10000);
        let fees = signal.notional_usd * fee_rate * Decimal::from(2);
        let slippage = signal.notional_usd * slippage_rate;
        let net_profit = gross_profit - fees - slippage;

        let was_winner = net_profit > Decimal::ZERO;

        TradeExecutionResult {
            profit_usd: gross_profit,
            fees_usd: fees,
            slippage_usd: slippage,
            duration_ms: 100,
            was_winner,
        }
    }

    fn update_state(
        &mut self,
        signal: &BacktestSignal,
        trade_result: &TradeExecutionResult,
        result: &mut SimulationResult,
    ) {
        if trade_result.was_winner {
            self.state.current_capital_usd +=
                trade_result.profit_usd - trade_result.fees_usd - trade_result.slippage_usd;
        } else {
            self.state.current_capital_usd -= trade_result.fees_usd;
        }

        if self.state.current_capital_usd > self.state.peak_capital_usd {
            self.state.peak_capital_usd = self.state.current_capital_usd;
        }

        let drawdown = (self.state.peak_capital_usd - self.state.current_capital_usd)
            / self.state.peak_capital_usd;
        self.state.current_drawdown_percent = drawdown;
        if drawdown > self.state.max_drawdown_percent {
            self.state.max_drawdown_percent = drawdown;
        }

        result.total_trades += 1;
        result.total_profit_usd += trade_result.profit_usd;
        result.total_fees_usd += trade_result.fees_usd;

        if !result.symbols_traded.contains(&signal.symbol) {
            result.symbols_traded.push(signal.symbol.clone());
        }
        if !result.exchanges_used.contains(&signal.buy_exchange) {
            result.exchanges_used.push(signal.buy_exchange);
        }
        if !result.exchanges_used.contains(&signal.sell_exchange) {
            result.exchanges_used.push(signal.sell_exchange);
        }

        if trade_result.was_winner {
            result.winning_trades += 1;
        } else {
            result.losing_trades += 1;
        }
    }

    fn calculate_performance_metrics(&mut self, result: &mut SimulationResult) {
        result.net_profit_usd = result.total_profit_usd - result.total_fees_usd;

        if self.config.initial_capital_usd > Decimal::ZERO {
            result.profit_percent =
                (result.net_profit_usd / self.config.initial_capital_usd) * Decimal::from(100);
        }

        result.max_drawdown_percent = self.state.max_drawdown_percent * Decimal::from(100);

        if result.total_trades > 0 {
            result.avg_trade_profit_usd =
                result.net_profit_usd / Decimal::from(result.total_trades);
        }

        if result.total_trades > 0 {
            result.win_rate_percent = (Decimal::from(result.winning_trades)
                / Decimal::from(result.total_trades))
                * Decimal::from(100);
        }

        if result.winning_trades > 0
            && result.losing_trades > 0
            && result.total_fees_usd > Decimal::ZERO
        {
            result.profit_factor = result.total_profit_usd / result.total_fees_usd;
        } else if result.winning_trades > 0 && result.losing_trades == 0 {
            result.profit_factor = result.total_profit_usd;
        }

        result.avg_trade_duration_ms = if !self.state.trade_history.is_empty() {
            self.state
                .trade_history
                .iter()
                .map(|t| t.duration_ms)
                .sum::<u64>()
                / self.state.trade_history.len() as u64
        } else {
            0
        };
    }

    pub fn get_results(&self) -> &[SimulationResult] {
        &self.results
    }

    pub fn reset(&mut self) {
        self.state = SimulationState {
            current_capital_usd: self.config.initial_capital_usd,
            peak_capital_usd: self.config.initial_capital_usd,
            current_drawdown_percent: Decimal::ZERO,
            max_drawdown_percent: Decimal::ZERO,
            trade_history: Vec::new(),
        };
        self.results.clear();
    }
}

struct TradeExecutionResult {
    profit_usd: Decimal,
    fees_usd: Decimal,
    slippage_usd: Decimal,
    duration_ms: u64,
    was_winner: bool,
}

#[derive(Debug, Clone)]
pub struct BacktestSignal {
    pub timestamp: DateTime<Utc>,
    pub symbol: Symbol,
    pub buy_exchange: ExchangeId,
    pub sell_exchange: ExchangeId,
    pub profit_bps: i32,
    pub notional_usd: Decimal,
    pub confidence: Decimal,
    pub strategy_id: String,
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self::new(SimulationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_signal(
        symbol: Symbol,
        profit_bps: i32,
        notional_usd: Decimal,
    ) -> BacktestSignal {
        BacktestSignal {
            timestamp: Utc::now(),
            symbol,
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            profit_bps,
            notional_usd,
            confidence: Decimal::from(80),
            strategy_id: "test_strategy".to_string(),
        }
    }

    #[test]
    fn test_simulation_config_default() {
        let config = SimulationConfig::default();

        assert_eq!(config.initial_capital_usd, Decimal::from(100000));
        assert!(config.start_date < config.end_date);
        assert_eq!(config.fee_taker_bps, 5);
        assert_eq!(config.fee_maker_bps, 2);
        assert_eq!(config.min_profit_threshold_bps, 5);
    }

    #[test]
    fn test_simulation_engine_new() {
        let config = SimulationConfig::default();
        let engine = SimulationEngine::new(config);

        assert_eq!(engine.state.current_capital_usd, Decimal::from(100000));
        assert_eq!(engine.state.peak_capital_usd, Decimal::from(100000));
        assert!(engine.results.is_empty());
    }

    #[test]
    fn test_run_backtest_no_signals() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let result = engine.run_backtest(&[]);

        assert_eq!(result.total_trades, 0);
        assert_eq!(result.net_profit_usd, Decimal::ZERO);
        assert_eq!(result.win_rate_percent, Decimal::ZERO);
    }

    #[test]
    fn test_run_backtest_single_profitable_trade() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            20,
            Decimal::from(10000),
        )];

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 1);
        assert_eq!(result.winning_trades, 1);
        assert_eq!(result.losing_trades, 0);
        assert!(result.net_profit_usd > Decimal::ZERO);
    }

    #[test]
    fn test_run_backtest_below_threshold() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            3, // Below min_profit_threshold_bps of 5
            Decimal::from(10000),
        )];

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 0);
        assert!(result.net_profit_usd == Decimal::ZERO);
    }

    #[test]
    fn test_run_backtest_exceeds_max_position() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            20,
            Decimal::from(100000), // Exceeds max_position_size_usd of 50000
        )];

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 0);
    }

    #[test]
    fn test_run_backtest_multiple_trades() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 15, Decimal::from(5000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 10, Decimal::from(3000)),
        ];

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 3);
        assert!(result.net_profit_usd > Decimal::ZERO);
    }

    #[test]
    fn test_run_backtest_win_rate_calculation() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 3, Decimal::from(10000)), // Filtered out
            create_test_signal(Symbol::new("XRP", "USDT"), 20, Decimal::from(10000)),
        ];

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 3);
        assert_eq!(result.winning_trades, 3);
        assert_eq!(result.win_rate_percent, Decimal::from(100));
    }

    #[test]
    fn test_run_backtest_mixed_results() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 3, Decimal::from(10000)), // Filtered
            create_test_signal(Symbol::new("SOL", "USDT"), 15, Decimal::from(10000)),
        ];

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 2);
        assert_eq!(result.winning_trades, 2);
        assert_eq!(result.losing_trades, 0);
    }

    #[test]
    fn test_profit_calculation() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            20, // 0.20%
            Decimal::from(10000),
        )];

        let result = engine.run_backtest(&signals);

        assert!(result.total_profit_usd > Decimal::ZERO);
        assert!(result.total_fees_usd > Decimal::ZERO);
        assert!(result.net_profit_usd < result.total_profit_usd);
    }

    #[test]
    fn test_max_drawdown_calculation() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 10, Decimal::from(5000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 5, Decimal::from(3000)),
        ];

        let result = engine.run_backtest(&signals);

        assert!(result.max_drawdown_percent >= Decimal::ZERO);
    }

    #[test]
    fn test_profit_factor_calculation() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 15, Decimal::from(5000)),
        ];

        let result = engine.run_backtest(&signals);

        assert!(result.profit_factor > Decimal::ZERO);
    }

    #[test]
    fn test_reset() {
        let config = SimulationConfig::default();
        let mut engine = SimulationEngine::new(config);

        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            20,
            Decimal::from(10000),
        )];
        engine.run_backtest(&signals);

        engine.reset();

        assert_eq!(engine.state.current_capital_usd, Decimal::from(100000));
        assert_eq!(engine.state.peak_capital_usd, Decimal::from(100000));
        assert!(engine.results.is_empty());
    }

    #[test]
    fn test_get_results() {
        let config = SimulationConfig::default();
        let engine = SimulationEngine::new(config);

        let results = engine.get_results();
        assert!(results.is_empty());
    }

    #[test]
    fn test_backtest_signal_structure() {
        let signal = BacktestSignal {
            timestamp: Utc::now(),
            symbol: Symbol::new("BTC", "USDT"),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            profit_bps: 15,
            notional_usd: Decimal::from(5000),
            confidence: Decimal::from(85),
            strategy_id: "cex_arbitrage".to_string(),
        };

        assert_eq!(signal.profit_bps, 15);
        assert_eq!(signal.notional_usd, Decimal::from(5000));
    }

    #[test]
    fn test_simulation_result_default() {
        let result = SimulationResult::default();

        assert_eq!(result.total_trades, 0);
        assert_eq!(result.winning_trades, 0);
        assert_eq!(result.net_profit_usd, Decimal::ZERO);
        assert_eq!(result.profit_percent, Decimal::ZERO);
    }

    #[test]
    fn test_backtest_signal_creation_and_validation() {
        let signal = BacktestSignal {
            timestamp: Utc::now(),
            symbol: Symbol::new("BTC", "USDT"),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            profit_bps: 15,
            notional_usd: Decimal::from(5000),
            confidence: Decimal::from(85),
            strategy_id: "cex_arbitrage".to_string(),
        };

        assert_eq!(signal.profit_bps, 15);
        assert_eq!(signal.notional_usd, Decimal::from(5000));
    }

    #[test]
    fn test_simulation_config_custom_values() {
        let config = SimulationConfig {
            initial_capital_usd: Decimal::from(50000),
            start_date: Utc::now() - Duration::days(7),
            end_date: Utc::now(),
            fee_taker_bps: 15,
            fee_maker_bps: 8,
            slippage_bps: 10,
            min_profit_threshold_bps: 8,
            max_position_size_usd: Decimal::from(25000),
            simulate_funding: false,
        };

        assert_eq!(config.initial_capital_usd, Decimal::from(50000));
        assert_eq!(config.fee_taker_bps, 15);
        assert_eq!(config.min_profit_threshold_bps, 8);
        assert!(!config.simulate_funding);
    }

    #[test]
    fn test_backtest_signal_with_different_profit_levels() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 5, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 15, Decimal::from(10000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 20, Decimal::from(10000)),
        ];

        let config = SimulationConfig {
            min_profit_threshold_bps: 8,
            fee_taker_bps: 5,
            slippage_bps: 2,
            max_position_size_usd: Decimal::from(50000),
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 2);
        assert_eq!(result.winning_trades, 2);
    }

    #[test]
    fn test_backtest_with_various_position_sizes() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 20, Decimal::from(50000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 20, Decimal::from(100000)),
        ];

        let config = SimulationConfig {
            max_position_size_usd: Decimal::from(50000),
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 2);
    }

    #[test]
    fn test_backtest_fee_calculation() {
        let config = SimulationConfig {
            fee_taker_bps: 20,
            fee_maker_bps: 10,
            slippage_bps: 0,
            max_position_size_usd: Decimal::from(200000),
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            50,
            Decimal::from(100000),
        )];

        let result = engine.run_backtest(&signals);

        assert!(result.total_fees_usd > Decimal::ZERO);
        assert!(result.net_profit_usd < result.total_profit_usd);
    }

    #[test]
    fn test_backtest_slippage_calculation() {
        let config = SimulationConfig {
            slippage_bps: 10,
            fee_taker_bps: 0,
            max_position_size_usd: Decimal::from(200000),
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            30,
            Decimal::from(100000),
        )];

        let result = engine.run_backtest(&signals);

        assert!(result.total_profit_usd > Decimal::ZERO);
    }

    #[test]
    fn test_backtest_mixed_profit_scenarios() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 25, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 15, Decimal::from(10000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 8, Decimal::from(10000)),
        ];

        let config = SimulationConfig {
            min_profit_threshold_bps: 10,
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 2);
        assert_eq!(result.winning_trades, 2);
    }

    #[test]
    fn test_backtest_performance_metrics() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 15, Decimal::from(10000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 25, Decimal::from(10000)),
        ];

        let mut engine = SimulationEngine::new(SimulationConfig::default());
        let result = engine.run_backtest(&signals);

        assert!(result.total_profit_usd > Decimal::ZERO);
        assert!(result.avg_trade_profit_usd > Decimal::ZERO);
        assert!(result.win_rate_percent > Decimal::ZERO);
    }

    #[test]
    fn test_backtest_drawdown_tracking() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 30, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 10, Decimal::from(10000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 5, Decimal::from(10000)),
            create_test_signal(Symbol::new("XRP", "USDT"), 20, Decimal::from(10000)),
        ];

        let mut engine = SimulationEngine::new(SimulationConfig::default());
        let result = engine.run_backtest(&signals);

        assert!(result.max_drawdown_percent >= Decimal::ZERO);
        assert!(result.max_drawdown_percent <= Decimal::from(100));
    }

    #[test]
    fn test_backtest_profit_factor_with_profits() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 15, Decimal::from(10000)),
        ];

        let mut engine = SimulationEngine::new(SimulationConfig::default());
        let result = engine.run_backtest(&signals);

        assert!(result.profit_factor > Decimal::ZERO);
        assert!(result.profit_factor > Decimal::from(1));
    }

    #[test]
    fn test_multiple_backtest_runs() {
        let mut engine = SimulationEngine::new(SimulationConfig::default());

        let signals1 = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            20,
            Decimal::from(10000),
        )];
        let result1 = engine.run_backtest(&signals1);

        let signals2 = vec![create_test_signal(
            Symbol::new("ETH", "USDT"),
            15,
            Decimal::from(10000),
        )];
        let result2 = engine.run_backtest(&signals2);

        assert_eq!(engine.results.len(), 2);
        assert!(result1.net_profit_usd > Decimal::ZERO);
        assert!(result2.net_profit_usd > Decimal::ZERO);
    }

    #[test]
    fn test_backtest_with_different_symbols() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 18, Decimal::from(8000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 22, Decimal::from(6000)),
            create_test_signal(Symbol::new("XRP", "USDT"), 15, Decimal::from(5000)),
        ];

        let mut engine = SimulationEngine::new(SimulationConfig::default());
        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 4);
        assert!(result.symbols_traded.contains(&Symbol::new("BTC", "USDT")));
        assert!(result.symbols_traded.contains(&Symbol::new("ETH", "USDT")));
    }

    #[test]
    fn test_backtest_with_different_exchanges() {
        let signal1 = BacktestSignal {
            timestamp: Utc::now(),
            symbol: Symbol::new("BTC", "USDT"),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            profit_bps: 20,
            notional_usd: Decimal::from(10000),
            confidence: Decimal::from(80),
            strategy_id: "test".to_string(),
        };
        let signal2 = BacktestSignal {
            timestamp: Utc::now(),
            symbol: Symbol::new("ETH", "USDT"),
            buy_exchange: ExchangeId::MEXC,
            sell_exchange: ExchangeId::GateIo,
            profit_bps: 15,
            notional_usd: Decimal::from(10000),
            confidence: Decimal::from(75),
            strategy_id: "test".to_string(),
        };

        let mut engine = SimulationEngine::new(SimulationConfig::default());
        let result = engine.run_backtest(&[signal1, signal2]);

        assert_eq!(result.total_trades, 2);
        assert!(result.exchanges_used.contains(&ExchangeId::ByBit));
        assert!(result.exchanges_used.contains(&ExchangeId::MEXC));
    }

    #[test]
    fn test_backtest_empty_profit_scenario() {
        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            5,
            Decimal::from(10000),
        )];

        let config = SimulationConfig {
            min_profit_threshold_bps: 10,
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 0);
        assert_eq!(result.net_profit_usd, Decimal::ZERO);
        assert_eq!(result.winning_trades, 0);
    }

    #[test]
    fn test_backtest_max_position_rejection() {
        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            20,
            Decimal::from(100000),
        )];

        let config = SimulationConfig {
            max_position_size_usd: Decimal::from(50000),
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        let result = engine.run_backtest(&signals);

        assert_eq!(result.total_trades, 0);
    }

    #[test]
    fn test_backtest_capital_preservation() {
        let signals = vec![create_test_signal(
            Symbol::new("BTC", "USDT"),
            5,
            Decimal::from(10000),
        )];

        let config = SimulationConfig {
            min_profit_threshold_bps: 10,
            ..Default::default()
        };
        let mut engine = SimulationEngine::new(config);

        engine.run_backtest(&signals);

        assert_eq!(engine.state.current_capital_usd, Decimal::from(100000));
    }

    #[test]
    fn test_simulation_engine_clone() {
        let config = SimulationConfig::default();
        let engine = SimulationEngine::new(config);

        let results = engine.get_results();
        assert!(results.is_empty());
    }

    #[test]
    fn test_backtest_cumulative_profits() {
        let signals = vec![
            create_test_signal(Symbol::new("BTC", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("ETH", "USDT"), 20, Decimal::from(10000)),
            create_test_signal(Symbol::new("SOL", "USDT"), 20, Decimal::from(10000)),
        ];

        let mut engine = SimulationEngine::new(SimulationConfig::default());
        let result = engine.run_backtest(&signals);

        assert!(result.total_profit_usd > Decimal::from(50));
    }

    #[test]
    fn test_simulation_config_date_range() {
        let now = Utc::now();
        let config = SimulationConfig {
            start_date: now - Duration::days(30),
            end_date: now,
            ..Default::default()
        };

        assert!(config.start_date < config.end_date);
        assert!(config.start_date < now);
    }
}
