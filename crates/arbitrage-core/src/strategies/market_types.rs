use crate::{ExchangeId, OrderBook, OrderType, Result, Side, Symbol};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MarketBundle {
    pub order_books: HashMap<(ExchangeId, Arc<Symbol>), Arc<OrderBook>>,
    pub funding_rates: HashMap<(ExchangeId, Arc<Symbol>), Arc<FundingRate>>,
    pub tickers: HashMap<(ExchangeId, Arc<Symbol>), Arc<Ticker>>,
    pub timestamp: DateTime<Utc>,
}

impl MarketBundle {
    const EXPECTED_SYMBOLS_PER_EXCHANGE: usize = 256;
    const EXPECTED_EXCHANGES: usize = 8;

    pub fn new() -> Self {
        let num_exchanges = Self::EXPECTED_EXCHANGES;
        let symbols_per_exchange = Self::EXPECTED_SYMBOLS_PER_EXCHANGE;
        let total_capacity = num_exchanges * symbols_per_exchange;

        Self {
            order_books: HashMap::with_capacity(total_capacity),
            funding_rates: HashMap::with_capacity(total_capacity),
            tickers: HashMap::with_capacity(total_capacity),
            timestamp: Utc::now(),
        }
    }

    pub fn add_order_book(&mut self, order_book: Arc<OrderBook>) {
        self.order_books.insert(
            (order_book.exchange, Arc::new(order_book.symbol.clone())),
            Arc::clone(&order_book),
        );
    }

    pub fn add_funding_rate(&mut self, funding_rate: Arc<FundingRate>) {
        self.funding_rates.insert(
            (funding_rate.exchange, Arc::new(funding_rate.symbol.clone())),
            Arc::clone(&funding_rate),
        );
    }

    pub fn add_ticker(&mut self, ticker: Arc<Ticker>) {
        self.tickers.insert(
            (ticker.exchange, Arc::new(ticker.symbol.clone())),
            Arc::clone(&ticker),
        );
    }

    pub fn get_order_book(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<&Arc<OrderBook>> {
        self.order_books.get(&(exchange, Arc::new(symbol.clone())))
    }

    pub fn get_funding_rate(
        &self,
        exchange: ExchangeId,
        symbol: &Symbol,
    ) -> Option<&Arc<FundingRate>> {
        self.funding_rates
            .get(&(exchange, Arc::new(symbol.clone())))
    }

    pub fn get_ticker(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<&Arc<Ticker>> {
        self.tickers.get(&(exchange, Arc::new(symbol.clone())))
    }

    pub fn get_exchanges_for_symbol(&self, symbol: &Symbol) -> Vec<ExchangeId> {
        let target_symbol = Arc::new(symbol.clone());
        self.order_books
            .keys()
            .filter(|(_, s)| *s == target_symbol)
            .map(|(exchange, _)| *exchange)
            .collect()
    }

    pub fn get_all_symbols(&self) -> Vec<Arc<Symbol>> {
        let capacity = self.order_books.len() + self.tickers.len() + self.funding_rates.len();
        let mut symbols: Vec<Arc<Symbol>> = Vec::with_capacity(capacity);

        symbols.extend(
            self.order_books
                .keys()
                .map(|(_, symbol)| Arc::clone(symbol)),
        );

        symbols.extend(self.tickers.keys().map(|(_, symbol)| Arc::clone(symbol)));

        symbols.extend(
            self.funding_rates
                .keys()
                .map(|(_, symbol)| Arc::clone(symbol)),
        );

        symbols.sort_by_key(|a| a.to_pair());
        symbols.dedup();
        symbols
    }

    pub fn has_data(&self, exchange: ExchangeId, symbol: &Symbol) -> bool {
        self.order_books
            .contains_key(&(exchange, Arc::new(symbol.clone())))
    }

    pub fn get_data_age(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<Duration> {
        self.get_order_book(exchange, symbol)
            .map(|ob| self.timestamp - ob.timestamp)
    }

    pub fn get_order_books_for_symbol(&self, symbol: &Symbol) -> Vec<&Arc<OrderBook>> {
        let target_symbol = Arc::new(symbol.clone());
        self.order_books
            .iter()
            .filter(|((_, s), _)| *s == target_symbol)
            .map(|(_, ob)| ob)
            .collect()
    }

    pub fn get_tickers_for_symbol(&self, symbol: &Symbol) -> Vec<&Arc<Ticker>> {
        let target_symbol = Arc::new(symbol.clone());
        self.tickers
            .iter()
            .filter(|((_, s), _)| *s == target_symbol)
            .map(|(_, t)| t)
            .collect()
    }

    pub fn is_healthy(&self, exchange: ExchangeId, symbol: &Symbol) -> bool {
        self.is_healthy_with_max_age(exchange, symbol, DEFAULT_MAX_DATA_AGE_MS)
    }

    pub fn is_healthy_with_max_age(
        &self,
        exchange: ExchangeId,
        symbol: &Symbol,
        max_age_ms: u64,
    ) -> bool {
        let max_age = Duration::milliseconds(max_age_ms as i64);

        if let Some(age) = self.get_data_age(exchange, symbol) {
            return age <= max_age;
        }

        false
    }

    pub fn get_exchange_health_status(
        &self,
        symbol: &Symbol,
        max_age_ms: u64,
    ) -> Vec<(ExchangeId, bool)> {
        let exchanges = self.get_exchanges_for_symbol(symbol);
        exchanges
            .into_iter()
            .map(|ex| (ex, self.is_healthy_with_max_age(ex, symbol, max_age_ms)))
            .collect()
    }
}

impl Default for MarketBundle {
    fn default() -> Self {
        Self::new()
    }
}

const DEFAULT_MAX_DATA_AGE_MS: u64 = 5000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSignal {
    pub strategy_id: String,
    pub symbol: Arc<Symbol>,
    pub legs: Vec<TradeLeg>,
    pub expected_profit_bps: i32,
    pub basis_bps: Option<i32>,
    pub confidence_factors: ConfidenceFactors,
    pub metadata: HashMap<String, Value>,
    pub detected_at: DateTime<Utc>,
}

impl RawSignal {
    const EXPECTED_LEGS: usize = 4;
    const EXPECTED_METADATA: usize = 8;

    pub fn new(strategy_id: impl Into<String>, symbol: Arc<Symbol>) -> Self {
        Self {
            strategy_id: strategy_id.into(),
            symbol,
            legs: Vec::with_capacity(Self::EXPECTED_LEGS),
            expected_profit_bps: 0,
            basis_bps: None,
            confidence_factors: ConfidenceFactors::default(),
            metadata: HashMap::with_capacity(Self::EXPECTED_METADATA),
            detected_at: Utc::now(),
        }
    }

    pub fn add_leg(&mut self, leg: TradeLeg) {
        self.legs.push(leg);
    }

    pub fn add_buy_leg(mut self, exchange: ExchangeId, price: Decimal, quantity: Decimal) -> Self {
        let leg = TradeLeg::new(
            exchange,
            Arc::clone(&self.symbol),
            Side::Buy,
            price,
            quantity,
        );
        self.legs.push(leg);
        self
    }

    pub fn add_sell_leg(mut self, exchange: ExchangeId, price: Decimal, quantity: Decimal) -> Self {
        let leg = TradeLeg::new(
            exchange,
            Arc::clone(&self.symbol),
            Side::Sell,
            price,
            quantity,
        );
        self.legs.push(leg);
        self
    }

    pub fn set_profit_bps(&mut self, profit_bps: i32) {
        self.expected_profit_bps = profit_bps;
    }

    pub fn with_profit_bps(mut self, bps: i32) -> Self {
        self.expected_profit_bps = bps;
        self
    }

    pub fn add_metadata(&mut self, key: impl Into<String>, value: Value) {
        self.metadata.insert(key.into(), value);
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn is_valid(&self) -> bool {
        !self.legs.is_empty()
            && self
                .legs
                .iter()
                .all(|leg| !leg.price.is_zero() && !leg.quantity.is_zero())
            && self.expected_profit_bps > 0
    }

    pub fn total_notional(&self) -> Decimal {
        self.legs.iter().map(|leg| leg.price * leg.quantity).sum()
    }

    pub fn get_exchanges(&self) -> Vec<ExchangeId> {
        let mut exchanges: Vec<ExchangeId> = self.legs.iter().map(|leg| leg.exchange).collect();
        exchanges.sort_by_key(|a| a.to_string());
        exchanges.dedup();
        exchanges
    }

    pub fn involves_exchange(&self, exchange: ExchangeId) -> bool {
        self.legs.iter().any(|leg| leg.exchange == exchange)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeLeg {
    pub exchange: ExchangeId,
    pub symbol: Arc<Symbol>,
    pub side: Side,
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_type: OrderType,
}

impl TradeLeg {
    pub fn new(
        exchange: ExchangeId,
        symbol: Arc<Symbol>,
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

use crate::ArbitrageError;

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
    pub fn new(
        exchange: ExchangeId,
        symbol: Symbol,
        rate: Decimal,
        next_funding: DateTime<Utc>,
    ) -> Self {
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
        let daily_rate = self
            .rate
            .checked_mul(Decimal::from(3))
            .ok_or_else(|| ArbitrageError::Calculation("Funding rate overflow".to_string()))?;

        let annual_rate = daily_rate
            .checked_mul(Decimal::from(365))
            .ok_or_else(|| ArbitrageError::Calculation("Annual rate overflow".to_string()))?;

        Ok(annual_rate)
    }

    pub fn is_valid(&self) -> bool {
        let min_rate = Decimal::new(-100, 2);
        let max_rate = Decimal::new(100, 2);
        self.rate >= min_rate && self.rate <= max_rate
    }

    pub fn is_positive(&self) -> bool {
        self.rate > Decimal::ZERO
    }

    pub fn time_to_funding(&self) -> i64 {
        (self.next_funding - Utc::now()).num_seconds()
    }
}

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
    pub fn new(
        exchange: ExchangeId,
        symbol: Symbol,
        bid: Decimal,
        ask: Decimal,
        last: Decimal,
    ) -> Self {
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

    pub fn mid_price(&self) -> Result<Decimal> {
        if self.bid.is_zero() && self.ask.is_zero() {
            return Err(ArbitrageError::Calculation(
                "Both bid and ask are zero".to_string(),
            ));
        }

        Ok((self.bid + self.ask) / Decimal::from(2))
    }

    pub fn spread_bps(&self) -> Result<Decimal> {
        let mid = self.mid_price()?;

        let spread_ratio = self.spread().checked_div(mid).ok_or_else(|| {
            ArbitrageError::Calculation("Division by zero in spread calculation".to_string())
        })?;

        Ok(spread_ratio * Decimal::from(10000))
    }

    pub fn is_valid(&self) -> bool {
        self.bid < self.ask && !self.bid.is_zero() && !self.ask.is_zero()
    }
}
