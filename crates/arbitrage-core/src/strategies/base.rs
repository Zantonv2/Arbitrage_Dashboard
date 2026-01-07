use crate::{ArbitrageError, Result, Signal, OrderBook, ExecutionInstruction, ExchangeId, Symbol, Side, OrderType};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Core strategy trait that all arbitrage strategies must implement
pub trait Strategy: Send + Sync {
    /// Strategy identifier for logging and configuration
    fn id(&self) -> &'static str;
    
    /// Human-readable strategy name
    fn name(&self) -> &'static str;
    
    /// Detect arbitrage opportunities from market data
    fn detect(&self, market_data: &MarketBundle) -> Result<Vec<RawSignal>>;
    
    /// Filter signals based on strategy-specific criteria
    fn filter(&self, signal: &RawSignal, context: &FilterContext) -> Result<bool>;
    
    /// Plan execution for validated signals
    /// 
    /// NOTE: This method is deprecated in favor of using ExecutionPreparer directly.
    /// Strategies should focus on signal detection, not execution planning.
    fn plan(&self, signal: &Signal, context: &ExecutionContext) -> Result<ExecutionInstruction> {
        let _ = (signal, context);
        Err(ArbitrageError::Configuration(
            "Strategy execution planning is deprecated - use ExecutionPreparer instead".to_string()
        ))
    }
    
    /// Get strategy configuration parameters
    fn config(&self) -> &StrategyConfig;
    
    /// Update strategy configuration
    fn update_config(&mut self, config: StrategyConfig) -> Result<()>;
}

/// Bundle of market data for strategy analysis
#[derive(Debug, Clone)]
pub struct MarketBundle {
    pub order_books: HashMap<(ExchangeId, Symbol), OrderBook>,
    pub funding_rates: HashMap<(ExchangeId, Symbol), FundingRate>,
    pub tickers: HashMap<(ExchangeId, Symbol), Ticker>,
    pub timestamp: DateTime<Utc>,
}

impl MarketBundle {
    pub fn new() -> Self {
        Self {
            order_books: HashMap::new(),
            funding_rates: HashMap::new(),
            tickers: HashMap::new(),
            timestamp: Utc::now(),
        }
    }
    
    pub fn add_order_book(&mut self, order_book: OrderBook) {
        self.order_books.insert((order_book.exchange, order_book.symbol.clone()), order_book);
    }
    
    pub fn add_funding_rate(&mut self, funding_rate: FundingRate) {
        self.funding_rates.insert((funding_rate.exchange, funding_rate.symbol.clone()), funding_rate);
    }
    
    pub fn add_ticker(&mut self, ticker: Ticker) {
        self.tickers.insert((ticker.exchange, ticker.symbol.clone()), ticker);
    }
    
    pub fn get_order_book(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<&OrderBook> {
        self.order_books.get(&(exchange, symbol.clone()))
    }
    
    pub fn get_funding_rate(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<&FundingRate> {
        self.funding_rates.get(&(exchange, symbol.clone()))
    }
    
    pub fn get_ticker(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<&Ticker> {
        self.tickers.get(&(exchange, symbol.clone()))
    }
    
    /// Get all order books for a specific symbol across all exchanges
    pub fn get_order_books_for_symbol(&self, symbol: &Symbol) -> Vec<&OrderBook> {
        self.order_books
            .iter()
            .filter(|((_, s), _)| s == symbol)
            .map(|(_, ob)| ob)
            .collect()
    }
    
    /// Get all tickers for a specific symbol across all exchanges
    pub fn get_tickers_for_symbol(&self, symbol: &Symbol) -> Vec<&Ticker> {
        self.tickers
            .iter()
            .filter(|((_, s), _)| s == symbol)
            .map(|(_, t)| t)
            .collect()
    }
}

impl Default for MarketBundle {
    fn default() -> Self {
        Self::new()
    }
}

/// Raw signal detected by a strategy before filtering and scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSignal {
    pub strategy_id: String,
    pub symbol: Symbol,
    pub legs: Vec<TradeLeg>,
    pub expected_profit_bps: i32,
    pub basis_bps: Option<i32>,
    pub confidence_factors: ConfidenceFactors,
    pub metadata: HashMap<String, serde_json::Value>,
    pub detected_at: DateTime<Utc>,
}

impl RawSignal {
    pub fn new(strategy_id: impl Into<String>, symbol: Symbol) -> Self {
        Self {
            strategy_id: strategy_id.into(),
            symbol,
            legs: Vec::new(),
            expected_profit_bps: 0,
            basis_bps: None,
            confidence_factors: ConfidenceFactors::default(),
            metadata: HashMap::new(),
            detected_at: Utc::now(),
        }
    }
    
    pub fn add_leg(&mut self, leg: TradeLeg) {
        self.legs.push(leg);
    }
    
    pub fn set_profit_bps(&mut self, profit_bps: i32) {
        self.expected_profit_bps = profit_bps;
    }
    
    pub fn add_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata.insert(key.into(), value);
    }
}

/// Individual trade leg in an arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeLeg {
    pub exchange: ExchangeId,
    pub symbol: Symbol,
    pub side: Side,
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_type: OrderType,
}

impl TradeLeg {
    pub fn new(
        exchange: ExchangeId,
        symbol: Symbol,
        side: Side,
        price: Decimal,
        quantity: Decimal,
    ) -> Self {
        Self {
            exchange,
            symbol,
            side,
            price,
            quantity,
            order_type: OrderType::Market,
        }
    }
    
    pub fn with_order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = order_type;
        self
    }
}

/// Factors contributing to signal confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceFactors {
    pub depth_score: Decimal,
    pub volatility_score: Decimal,
    pub reliability_score: Decimal,
    pub spread_stability: Decimal,
    pub freshness_score: Decimal,
}

impl Default for ConfidenceFactors {
    fn default() -> Self {
        Self {
            depth_score: Decimal::ZERO,
            volatility_score: Decimal::ZERO,
            reliability_score: Decimal::ZERO,
            spread_stability: Decimal::ZERO,
            freshness_score: Decimal::ZERO,
        }
    }
}

/// Context for signal filtering
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
            max_exposure: Decimal::from(10000), // Default $10k max exposure
            allowed_exchanges: vec![
                ExchangeId::OKX,
                ExchangeId::ByBit,
                ExchangeId::MEXC,
                ExchangeId::GateIo,
            ],
            risk_limits: RiskLimits::default(),
            fee_schedules: HashMap::new(),
            max_latency_ms: 500, // 500ms max latency
            min_notional_usd: Decimal::from(10), // $10 minimum
            inventory_limits: HashMap::new(),
        }
    }
    
    pub fn get_fee_schedule(&self, exchange: ExchangeId) -> Option<&FeeSchedule> {
        self.fee_schedules.get(&exchange)
    }
    
    pub fn can_sell(&self, exchange: ExchangeId, asset: &str, quantity: Decimal) -> bool {
        let available = self.inventory_limits
            .get(&(exchange, asset.to_string()))
            .copied()
            .unwrap_or_else(|| Decimal::ZERO);
        available >= quantity
    }
}

/// Context for execution planning
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub available_balances: HashMap<(ExchangeId, String), Decimal>,
    pub fee_schedules: HashMap<ExchangeId, FeeSchedule>,
    pub slippage_tolerance: Decimal,
    pub max_order_size: Decimal,
}

impl ExecutionContext {
    pub fn new() -> Self {
        // Safe decimal creation using Decimal::new(mantissa, scale)
        let default_slippage = Decimal::new(1, 3); // 0.001 = 0.1%
        
        Self {
            available_balances: HashMap::new(),
            fee_schedules: HashMap::new(),
            slippage_tolerance: default_slippage,
            max_order_size: Decimal::from(1000), // Default $1k max order
        }
    }
    
    pub fn get_balance(&self, exchange: ExchangeId, asset: &str) -> Decimal {
        self.available_balances
            .get(&(exchange, asset.to_string()))
            .copied()
            .unwrap_or_else(|| Decimal::ZERO)
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Risk limits for strategy execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: Decimal,
    pub max_daily_volume: Decimal,
    pub max_drawdown: Decimal,
    pub stop_loss_threshold: Decimal,
}

impl Default for RiskLimits {
    fn default() -> Self {
        // Safe decimal creation using Decimal::new(mantissa, scale)
        let max_drawdown = Decimal::new(5, 2); // 0.05 = 5%
        let stop_loss_threshold = Decimal::new(2, 2); // 0.02 = 2%
        
        Self {
            max_position_size: Decimal::from(5000),
            max_daily_volume: Decimal::from(50000),
            max_drawdown,
            stop_loss_threshold,
        }
    }
}

/// Strategy configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub enabled: bool,
    pub min_profit_bps: i32,
    pub max_exposure: Decimal,
    pub confidence_threshold: Decimal,
    pub risk_limits: RiskLimits,
    /// Strategy-specific parameters stored as JSON values
    /// Examples: max_latency_ms, vwap_quantity_usd, inventory_based, etc.
    pub custom_params: HashMap<String, serde_json::Value>,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        let confidence_threshold = Decimal::new(7, 1); // 0.7
        
        Self {
            enabled: true,
            min_profit_bps: 10, // 0.1% minimum profit
            max_exposure: Decimal::from(10000),
            confidence_threshold, // 70%
            risk_limits: RiskLimits::default(),
            custom_params: HashMap::new(),
        }
    }
}

/// Funding rate information for perpetual contracts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    pub exchange: ExchangeId,
    pub symbol: Symbol,
    pub rate: Decimal,
    pub next_funding: DateTime<Utc>,
    pub predicted_rate: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

impl FundingRate {
    pub fn new(exchange: ExchangeId, symbol: Symbol, rate: Decimal, next_funding: DateTime<Utc>) -> Self {
        Self {
            exchange,
            symbol,
            rate,
            next_funding,
            predicted_rate: None,
            timestamp: Utc::now(),
        }
    }
    
    pub fn annualized_rate(&self) -> Result<Decimal> {
        // Funding typically occurs every 8 hours (3 times per day)
        let daily_rate = self.rate.checked_mul(Decimal::from(3))
            .ok_or_else(|| ArbitrageError::Calculation("Funding rate overflow".to_string()))?;
        
        let annual_rate = daily_rate.checked_mul(Decimal::from(365))
            .ok_or_else(|| ArbitrageError::Calculation("Annual rate overflow".to_string()))?;
            
        Ok(annual_rate)
    }
}

/// Ticker information with price and volume data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticker {
    pub exchange: ExchangeId,
    pub symbol: Symbol,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last: Decimal,
    pub volume_24h: Decimal,
    pub change_24h: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl Ticker {
    pub fn new(exchange: ExchangeId, symbol: Symbol, bid: Decimal, ask: Decimal, last: Decimal) -> Self {
        Self {
            exchange,
            symbol,
            bid,
            ask,
            last,
            volume_24h: Decimal::ZERO,
            change_24h: Decimal::ZERO,
            timestamp: Utc::now(),
        }
    }
    
    pub fn spread(&self) -> Decimal {
        self.ask - self.bid
    }
    
    pub fn mid_price(&self) -> Decimal {
        (self.bid + self.ask) / Decimal::from(2)
    }
    
    pub fn spread_bps(&self) -> Result<Decimal> {
        let mid = self.mid_price();
        if mid.is_zero() {
            return Err(ArbitrageError::Calculation("Zero mid price".to_string()));
        }
        
        let spread_ratio = self.spread().checked_div(mid)
            .ok_or_else(|| ArbitrageError::Calculation("Division by zero in spread calculation".to_string()))?;
            
        Ok(spread_ratio * Decimal::from(10000)) // Convert to basis points
    }
}

/// Fee schedule for an exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeSchedule {
    pub exchange: ExchangeId,
    pub maker_fee: Decimal,
    pub taker_fee: Decimal,
    pub tier: Option<String>,
}

impl FeeSchedule {
    pub fn new(exchange: ExchangeId, maker_fee: Decimal, taker_fee: Decimal) -> Self {
        Self {
            exchange,
            maker_fee,
            taker_fee,
            tier: None,
        }
    }
    
    pub fn calculate_fee(&self, notional: Decimal, is_maker: bool) -> Result<Decimal> {
        let rate = if is_maker { self.maker_fee } else { self.taker_fee };
        
        notional.checked_mul(rate)
            .ok_or_else(|| ArbitrageError::Calculation("Fee calculation overflow".to_string()))
    }
}