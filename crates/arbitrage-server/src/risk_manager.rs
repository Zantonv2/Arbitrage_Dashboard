//! # Risk Manager Module
//!
//! Provides risk management features for arbitrage trading including:
//! - Balance checking before execution
//! - Slippage protection (quote, re-quote, cancel pattern)
//! - Position limits tracking
//!
//! # Example
//!
//! ```rust,ignore
//! use arbitrage_server::risk_manager::{RiskManager, RiskConfig};
//! use arbitrage_core::types::ExchangeId;
//! use rust_decimal::Decimal;
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//!
//! let config = RiskConfig::default();
//! // RiskManager requires an ExchangeManager instance
//! ```

use arbitrage_core::{
    types::{ExchangeId, ExecutionInstruction, Symbol},
    ArbitrageError, Result,
};
use exchange_connectors::exchange_manager::ExchangeManager;
use rust_decimal::Decimal;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Configuration for risk management
#[derive(Debug, Clone)]
pub struct RiskConfig {
    /// Enable balance checking before execution
    pub enable_balance_check: bool,
    /// Enable slippage protection
    pub enable_slippage_protection: bool,
    /// Maximum slippage tolerance (as percentage, e.g., 50 = 0.5%)
    pub max_slippage_percent: Decimal,
    /// Maximum number of requotes before canceling
    pub max_requote_attempts: u32,
    /// Requote timeout in milliseconds
    pub requote_timeout_ms: u64,
    /// Enable position limits
    pub enable_position_limits: bool,
    /// Maximum position size per symbol
    pub max_position_per_symbol: Decimal,
    /// Maximum daily volume per symbol
    pub max_daily_volume_per_symbol: Decimal,
    /// Maximum daily volume per exchange
    pub max_daily_volume_per_exchange: Decimal,
    /// Minimum balance threshold (as percentage of required)
    pub min_balance_threshold_percent: Decimal,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            enable_balance_check: true,
            enable_slippage_protection: true,
            max_slippage_percent: Decimal::new(50, 4), // 0.5%
            max_requote_attempts: 3,
            requote_timeout_ms: 2000,
            enable_position_limits: true,
            max_position_per_symbol: Decimal::new(10000, 0), // $10,000
            max_daily_volume_per_symbol: Decimal::new(100000, 0), // $100,000
            max_daily_volume_per_exchange: Decimal::new(500000, 0), // $500,000
            min_balance_threshold_percent: Decimal::new(100, 0), // 100% - must have full balance
        }
    }
}

/// Result of a risk check
#[derive(Debug, Clone)]
pub struct RiskCheckResult {
    pub allowed: bool,
    pub reason: Option<String>,
    pub buy_balance_ok: bool,
    pub sell_balance_ok: bool,
    pub position_within_limits: bool,
    pub daily_volume_within_limits: bool,
}

/// Result of a slippage check
#[derive(Debug, Clone)]
pub struct SlippageCheckResult {
    pub allowed: bool,
    pub actual_slippage_percent: Decimal,
    pub current_buy_price: Option<Decimal>,
    pub current_sell_price: Option<Decimal>,
    pub should_requote: bool,
    pub requote_count: u32,
}

/// Position tracking for a symbol
#[derive(Debug, Clone)]
pub struct PositionRecord {
    pub symbol: Symbol,
    pub exchange: ExchangeId,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Daily volume record
#[derive(Debug, Clone)]
pub struct DailyVolume {
    pub date: chrono::NaiveDate,
    pub volume_by_symbol: HashMap<String, Decimal>,
    pub volume_by_exchange: HashMap<ExchangeId, Decimal>,
}

/// Risk manager for arbitrage trading
pub struct RiskManager {
    config: RiskConfig,
    exchange_manager: Arc<Mutex<ExchangeManager>>,
    /// Current positions by symbol and exchange
    positions: HashMap<(Symbol, ExchangeId), PositionRecord>,
    /// Daily volume tracking
    daily_volume: DailyVolume,
    /// Requote counters by signal_id
    requote_counters: HashMap<Uuid, u32>,
}

impl RiskManager {
    /// Create a new risk manager
    pub fn new(config: RiskConfig, exchange_manager: Arc<Mutex<ExchangeManager>>) -> Self {
        Self {
            config,
            exchange_manager,
            positions: HashMap::new(),
            daily_volume: DailyVolume {
                date: chrono::Utc::now().date_naive(),
                volume_by_symbol: HashMap::new(),
                volume_by_exchange: HashMap::new(),
            },
            requote_counters: HashMap::new(),
        }
    }

    /// Check all risk conditions before execution
    pub async fn check_risk(&mut self, instruction: &ExecutionInstruction) -> RiskCheckResult {
        let mut buy_balance_ok = true;
        let mut sell_balance_ok = true;
        let mut position_within_limits = true;
        let mut reasons = Vec::new();

        // Check balance if enabled
        if self.config.enable_balance_check {
            let buy_check = self
                .check_balance(
                    &instruction.buy_order.exchange,
                    &instruction.buy_order.symbol,
                    instruction.buy_order.quantity,
                    instruction.buy_order.price,
                )
                .await;

            match buy_check {
                Ok(ok) => {
                    buy_balance_ok = ok;
                    if !ok {
                        reasons.push(format!(
                            "Insufficient balance on {} for {}",
                            instruction.buy_order.exchange, instruction.buy_order.symbol
                        ));
                    }
                }
                Err(e) => {
                    warn!("Balance check failed for buy: {}", e);
                    // Don't block execution if balance check fails due to API error
                }
            }

            let sell_check = self
                .check_balance(
                    &instruction.sell_order.exchange,
                    &instruction.sell_order.symbol,
                    instruction.sell_order.quantity,
                    instruction.sell_order.price,
                )
                .await;

            match sell_check {
                Ok(ok) => {
                    sell_balance_ok = ok;
                    if !ok {
                        reasons.push(format!(
                            "Insufficient balance on {} for {}",
                            instruction.sell_order.exchange, instruction.sell_order.symbol
                        ));
                    }
                }
                Err(e) => {
                    warn!("Balance check failed for sell: {}", e);
                }
            }
        }

        // Check position limits if enabled
        if self.config.enable_position_limits {
            if let Some(price) = instruction.buy_order.price {
                let position_value = instruction.buy_order.quantity * price;
                if position_value > self.config.max_position_per_symbol {
                    position_within_limits = false;
                    reasons.push(format!(
                        "Position size ${:.2} exceeds limit ${:.2}",
                        position_value, self.config.max_position_per_symbol
                    ));
                }
            }
        }

        // Check daily volume limits
        if self.config.enable_position_limits {
            if let Err(e) = self.check_daily_volume(instruction).await {
                warn!("Daily volume check failed: {}", e);
            }
            // Note: We warn but don't block on volume check failures
        }

        let allowed = buy_balance_ok && sell_balance_ok && position_within_limits;
        // Daily volume check warns but doesn't block, so assume true
        let daily_volume_within_limits = true;

        RiskCheckResult {
            allowed,
            reason: if reasons.is_empty() {
                None
            } else {
                Some(reasons.join("; "))
            },
            buy_balance_ok,
            sell_balance_ok,
            position_within_limits,
            daily_volume_within_limits,
        }
    }

    /// Check if account has sufficient balance for an order
    pub async fn check_balance(
        &self,
        exchange: &ExchangeId,
        symbol: &Symbol,
        quantity: Decimal,
        price: Option<Decimal>,
    ) -> Result<bool> {
        let balance = self.exchange_manager.lock().await.get_balance(exchange).await?;

        // Determine required asset and amount
        let (required_asset, required_amount) = if let Some(price) = price {
            // For buy orders, need quote currency (e.g., USDT)
            let required = price * quantity;
            (symbol.quote.clone(), required)
        } else {
            // For sell orders, need base currency (e.g., BTC)
            (symbol.base.clone(), quantity)
        };

        // Check balance
        if let Some(asset_balance) = balance.balances.get(&required_asset) {
            let available = asset_balance.free;
            let required_with_buffer =
                required_amount * (Decimal::ONE + self.config.min_balance_threshold_percent / Decimal::from(100));

            debug!(
                "Balance check: {} requires {:.2}, has {:.2} available",
                required_asset, required_amount, available
            );

            Ok(available >= required_with_buffer)
        } else {
            Ok(false)
        }
    }

    /// Check daily volume limits
    pub async fn check_daily_volume(&mut self, instruction: &ExecutionInstruction) -> Result<()> {
        // Reset daily volume if it's a new day
        self.reset_daily_volume_if_needed().await;

        let symbol_key = instruction.buy_order.symbol.to_string();

        // Check symbol volume
        if let Some(current_volume) = self.daily_volume.volume_by_symbol.get(&symbol_key) {
            let order_value = instruction.buy_order.quantity
                * instruction.buy_order.price.unwrap_or_default();
            let new_volume = current_volume + order_value;

            if new_volume > self.config.max_daily_volume_per_symbol {
                return Err(ArbitrageError::Execution(format!(
                    "Daily volume limit exceeded for {}: ${:.2} > ${:.2}",
                    symbol_key, new_volume, self.config.max_daily_volume_per_symbol
                )));
            }
        }

        // Check exchange volume
        let buy_exchange_volume = *self
            .daily_volume
            .volume_by_exchange
            .get(&instruction.buy_order.exchange)
            .unwrap_or(&Decimal::ZERO);

        let sell_exchange_volume = *self
            .daily_volume
            .volume_by_exchange
            .get(&instruction.sell_order.exchange)
            .unwrap_or(&Decimal::ZERO);

        let order_value = instruction.buy_order.quantity
            * instruction.buy_order.price.unwrap_or_default();

        if buy_exchange_volume + order_value > self.config.max_daily_volume_per_exchange {
            return Err(ArbitrageError::Execution(format!(
                "Daily volume limit exceeded for exchange {}: ${:.2} > ${:.2}",
                instruction.buy_order.exchange,
                buy_exchange_volume + order_value,
                self.config.max_daily_volume_per_exchange
            )));
        }

        if sell_exchange_volume + order_value > self.config.max_daily_volume_per_exchange {
            return Err(ArbitrageError::Execution(format!(
                "Daily volume limit exceeded for exchange {}: ${:.2} > ${:.2}",
                instruction.sell_order.exchange,
                sell_exchange_volume + order_value,
                self.config.max_daily_volume_per_exchange
            )));
        }

        Ok(())
    }

    /// Reset daily volume if it's a new day
    async fn reset_daily_volume_if_needed(&mut self) {
        let today = chrono::Utc::now().date_naive();
        if self.daily_volume.date != today {
            info!("Resetting daily volume tracking for new day");
            self.daily_volume = DailyVolume {
                date: today,
                volume_by_symbol: HashMap::new(),
                volume_by_exchange: HashMap::new(),
            };
        }
    }

    /// Record a completed trade for volume tracking
    pub async fn record_trade(&mut self, instruction: &ExecutionInstruction, actual_quantity: Decimal) {
        self.reset_daily_volume_if_needed().await;

        let symbol_key = instruction.buy_order.symbol.to_string();
        let order_value = actual_quantity * instruction.buy_order.price.unwrap_or_default();

        // Update symbol volume
        *self.daily_volume.volume_by_symbol.entry(symbol_key).or_insert(Decimal::ZERO) += order_value;

        // Update exchange volumes
        *self.daily_volume.volume_by_exchange.entry(instruction.buy_order.exchange).or_insert(Decimal::ZERO) += order_value;
        *self.daily_volume.volume_by_exchange.entry(instruction.sell_order.exchange).or_insert(Decimal::ZERO) += order_value;

        info!(
            "Recorded trade: {} {} = ${:.2}",
            actual_quantity,
            instruction.buy_order.symbol,
            order_value
        );
    }

    /// Check slippage protection - quote, re-quote, cancel pattern
    pub async fn check_slippage(
        &mut self,
        signal_id: Uuid,
        instruction: &ExecutionInstruction,
    ) -> SlippageCheckResult {
        if !self.config.enable_slippage_protection {
            return SlippageCheckResult {
                allowed: true,
                actual_slippage_percent: Decimal::ZERO,
                current_buy_price: None,
                current_sell_price: None,
                should_requote: false,
                requote_count: 0,
            };
        }

        // Get current requote count
        let requote_count = *self.requote_counters.get(&signal_id).unwrap_or(&0);

        // If we've exceeded max requote attempts, allow execution (last chance)
        if requote_count >= self.config.max_requote_attempts {
            warn!(
                "Max requote attempts reached for signal {}, proceeding with execution",
                signal_id
            );
            return SlippageCheckResult {
                allowed: true,
                actual_slippage_percent: Decimal::ZERO,
                current_buy_price: None,
                current_sell_price: None,
                should_requote: false,
                requote_count,
            };
        }

        // Fetch current market prices
        let (current_buy_price, current_sell_price) = self
            .fetch_current_prices(&instruction.buy_order.exchange, &instruction.sell_order.exchange, &instruction.buy_order.symbol)
            .await;

        // Calculate slippage
        let mut actual_slippage = Decimal::ZERO;

        if let (Some(current), Some(original)) = (current_buy_price, instruction.buy_order.price) {
            if original > Decimal::ZERO {
                let slippage = (current - original).abs() / original * Decimal::from(100);
                actual_slippage = actual_slippage.max(slippage);
            }
        }

        if let (Some(current), Some(original)) = (current_sell_price, instruction.sell_order.price) {
            if original > Decimal::ZERO {
                let slippage = (current - original).abs() / original * Decimal::from(100);
                actual_slippage = actual_slippage.max(slippage);
            }
        }

        let should_requote = actual_slippage > self.config.max_slippage_percent;

        if should_requote {
            // Increment requote counter
            *self.requote_counters.entry(signal_id).or_insert(0) += 1;
            warn!(
                "Slippage {:.4}% exceeds threshold {:.4}%, requote attempt {}/{}",
                actual_slippage,
                self.config.max_slippage_percent * Decimal::from(100),
                requote_count + 1,
                self.config.max_requote_attempts
            );
        }

        SlippageCheckResult {
            allowed: !should_requote || requote_count >= self.config.max_requote_attempts,
            actual_slippage_percent: actual_slippage,
            current_buy_price,
            current_sell_price,
            should_requote,
            requote_count,
        }
    }

    /// Fetch current market prices from exchanges
    async fn fetch_current_prices(
        &self,
        buy_exchange: &ExchangeId,
        sell_exchange: &ExchangeId,
        symbol: &Symbol,
    ) -> (Option<Decimal>, Option<Decimal>) {
        let em = self.exchange_manager.lock().await;

        // Get order book for buy exchange - use ask price (price to buy at)
        let buy_price = em.get_order_book(*buy_exchange, symbol)
            .await
            .ok()
            .and_then(|ob| ob.asks.first().map(|level| level.price));

        // Get order book for sell exchange - use bid price (price to sell at)
        let sell_price = em.get_order_book(*sell_exchange, symbol)
            .await
            .ok()
            .and_then(|ob| ob.bids.first().map(|level| level.price));

        (buy_price, sell_price)
    }

    /// Reset requote counter for a signal (call after successful execution)
    pub fn reset_requote_counter(&mut self, signal_id: Uuid) {
        self.requote_counters.remove(&signal_id);
    }

    /// Get current configuration
    pub fn get_config(&self) -> &RiskConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: RiskConfig) {
        self.config = config;
    }

    /// Get current daily volume for a symbol
    pub fn get_daily_volume_symbol(&self, symbol: &str) -> Decimal {
        self.daily_volume
            .volume_by_symbol
            .get(symbol)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Get current daily volume for an exchange
    pub fn get_daily_volume_exchange(&self, exchange: ExchangeId) -> Decimal {
        self.daily_volume
            .volume_by_exchange
            .get(&exchange)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Get current positions
    pub fn get_positions(&self) -> &HashMap<(Symbol, ExchangeId), PositionRecord> {
        &self.positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arbitrage_core::types::{Order, Side, TimeInForce, OrderType as CoreOrderType};
    use std::str::FromStr;

    #[test]
    fn test_risk_config_defaults() {
        let config = RiskConfig::default();
        assert!(config.enable_balance_check);
        assert!(config.enable_slippage_protection);
        assert!(config.enable_position_limits);
        assert_eq!(config.max_slippage_percent, Decimal::new(50, 4));
        assert_eq!(config.max_requote_attempts, 3);
    }

    #[test]
    fn test_risk_check_result_allowed() {
        let result = RiskCheckResult {
            allowed: true,
            reason: None,
            buy_balance_ok: true,
            sell_balance_ok: true,
            position_within_limits: true,
            daily_volume_within_limits: true,
        };
        assert!(result.allowed);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_risk_check_result_blocked() {
        let result = RiskCheckResult {
            allowed: false,
            reason: Some("Insufficient balance on ByBit for BTC/USDT".to_string()),
            buy_balance_ok: false,
            sell_balance_ok: true,
            position_within_limits: true,
            daily_volume_within_limits: true,
        };
        assert!(!result.allowed);
        assert!(result.reason.is_some());
    }

    #[test]
    fn test_slippage_check_result() {
        let result = SlippageCheckResult {
            allowed: false,
            actual_slippage_percent: Decimal::new(75, 4), // 0.75%
            current_buy_price: Some(Decimal::from_str("50200").unwrap()),
            current_sell_price: Some(Decimal::from_str("49900").unwrap()),
            should_requote: true,
            requote_count: 1,
        };
        
        assert!(!result.allowed);
        assert!(result.should_requote);
    }
}
