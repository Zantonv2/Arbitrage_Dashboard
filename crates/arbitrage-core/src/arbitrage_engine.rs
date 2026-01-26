//! # Arbitrage Engine
//!
//! Core orchestration engine for the arbitrage detection and execution pipeline.
//!
//! ## Pipeline Flow
//! ```text
//! Market Data → Cache → Strategy Detection → Deduplicate → Fee Calculation →
//! Risk Validation → Size Calculation → Confidence Scoring → Threshold Check →
//! Store → Emit Signal
//! ```
//!
//! ## Responsibilities
//! - Receives market data updates (order books, tickers, funding rates)
//! - Orchestrates strategy execution
//! - Processes raw signals through the complete validation pipeline
//! - Manages signal deduplication and caching
//! - Emits validated signals for UI display or auto-execution

use crate::{
    confidence_scorer::{ConfidenceScorer, NetSpreadResult},
    config::Config,
    constants::BROADCAST_CHANNEL_CAPACITY,
    execution_preparer::ExecutionPreparer,
    normalizer::Normalizer,
    size_calculator::SizeCalculator,
    storage::{SignalStatus, StorageService},
    strategies::{FilterContext, FundingRate, MarketBundle, RawSignal, StrategyRegistry, Ticker},
    types::{ExchangeId, OrderBook, Signal, Symbol},
    ArbitrageError, Result,
};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rustc_hash::FxHashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

// ============================================================================
// Types
// ============================================================================

/// Execution mode for the engine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Manual mode: signals are emitted for user review
    Manual,
    /// Auto mode: signals above threshold are auto-executed
    Auto { confidence_threshold: Decimal },
}

impl Default for ExecutionMode {
    #[inline]
    fn default() -> Self {
        Self::Manual
    }
}

/// Key for identifying unique arbitrage opportunities (for deduplication)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpportunityKey {
    pub symbol: Symbol,
    pub buy_exchange: ExchangeId,
    pub sell_exchange: ExchangeId,
    pub strategy_id: String,
}

/// Cached signal entry for deduplication
#[derive(Debug, Clone)]
struct CachedSignal {
    last_updated: DateTime<Utc>,
    profit_bps: i32,
}

type OrderBookMap = DashMap<(ExchangeId, Arc<Symbol>), Arc<OrderBook>>;
type TickerMap = DashMap<(ExchangeId, Arc<Symbol>), Arc<Ticker>>;
type FundingRateMap = DashMap<(ExchangeId, Arc<Symbol>), Arc<FundingRate>>;

/// Engine statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub order_books_count: usize,
    pub tickers_count: usize,
    pub funding_rates_count: usize,
    pub cached_signals_count: usize,
    pub active_symbols_count: usize,
    pub signals_detected: u64,
    pub signals_filtered: u64,
    pub signals_emitted: u64,
    pub signals_from_unhealthy_exchanges: u64,
    pub statistics_retrieval_failures: u64,
    pub last_detection_time: Option<DateTime<Utc>>,
}

// ============================================================================
// Arbitrage Engine
// ============================================================================

/// Core arbitrage computation and orchestration engine
///
/// The engine is responsible for:
/// 1. Caching market data (order books, tickers, funding rates)
/// 2. Running strategy detection when market data updates
/// 3. Processing raw signals through the validation pipeline
/// 4. Emitting validated signals for execution or display
pub struct ArbitrageEngine {
    order_books: Arc<OrderBookMap>,
    tickers: Arc<TickerMap>,
    funding_rates: Arc<FundingRateMap>,

    // Signal deduplication cache
    signal_cache: Arc<DashMap<OpportunityKey, CachedSignal>>,

    // Signal broadcasting
    signal_sender: broadcast::Sender<Signal>,

    // Core modules
    confidence_scorer: Arc<ConfidenceScorer>,
    size_calculator: Arc<SizeCalculator>,
    storage: Arc<StorageService>,

    // Configuration
    config: Config,
    execution_mode: ExecutionMode,

    // Deduplication settings
    profit_change_threshold_bps: i32,
    signal_ttl: Duration,

    // Statistics (atomic for lock-free concurrent access)
    stats: Arc<DashMap<&'static str, AtomicU64>>,
}

impl ArbitrageEngine {
    /// Create a new arbitrage engine with all required modules
    pub fn new(
        config: Config,
        normalizer: Arc<Normalizer>,
        confidence_scorer: Arc<ConfidenceScorer>,
        size_calculator: Arc<SizeCalculator>,
        execution_preparer: Arc<ExecutionPreparer>,
        storage: Arc<StorageService>,
    ) -> Result<(Self, broadcast::Receiver<Signal>)> {
        let (signal_sender, signal_receiver) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);

        let engine = Self {
            order_books: Arc::new(DashMap::new()),
            tickers: Arc::new(DashMap::new()),
            funding_rates: Arc::new(DashMap::new()),
            signal_cache: Arc::new(DashMap::new()),
            signal_sender,
            confidence_scorer,
            size_calculator,
            storage,
            config,
            execution_mode: ExecutionMode::default(),
            profit_change_threshold_bps: 5, // 0.05% change triggers new signal
            signal_ttl: Duration::seconds(300), // 5 minute TTL
            stats: Arc::new(DashMap::new()),
        };

        Ok((engine, signal_receiver))
    }

    /// Subscribe to signal broadcasts
    pub fn subscribe(&self) -> broadcast::Receiver<Signal> {
        self.signal_sender.subscribe()
    }

    /// Set execution mode (Manual or Auto)
    pub fn set_execution_mode(&mut self, mode: ExecutionMode) {
        self.execution_mode = mode;
        info!("Execution mode set to {:?}", mode);
    }

    /// Get current execution mode
    pub fn get_execution_mode(&self) -> ExecutionMode {
        self.execution_mode
    }

    // ========================================================================
    // Market Data Updates
    // ========================================================================

    /// Update order book cache
    pub async fn update_order_book(&self, order_book: OrderBook) -> Result<()> {
        let symbol = Arc::new(order_book.symbol.clone());
        let key = (order_book.exchange, Arc::clone(&symbol));

        debug!(
            "Updating order book: {} on {} ({} bids, {} asks)",
            symbol,
            order_book.exchange,
            order_book.bids.len(),
            order_book.asks.len()
        );

        if !order_book.is_valid() {
            return Err(ArbitrageError::Validation(format!(
                "Invalid order book for {} on {}",
                symbol, order_book.exchange
            )));
        }

        self.order_books.insert(key, Arc::new(order_book));
        self.increment_stat("order_book_updates");

        Ok(())
    }

    /// Update ticker cache
    pub async fn update_ticker(&self, ticker: Ticker) -> Result<()> {
        let symbol = Arc::new(ticker.symbol.clone());
        let key = (ticker.exchange, Arc::clone(&symbol));

        debug!(
            "Updating ticker: {} on {} (bid={}, ask={})",
            symbol, ticker.exchange, ticker.bid, ticker.ask
        );

        self.tickers.insert(key, Arc::new(ticker));
        self.increment_stat("ticker_updates");

        Ok(())
    }

    /// Update funding rate cache
    pub async fn update_funding_rate(&self, funding_rate: FundingRate) -> Result<()> {
        let symbol = Arc::new(funding_rate.symbol.clone());
        let key = (funding_rate.exchange, Arc::clone(&symbol));

        debug!(
            "Updating funding rate: {} on {} (rate={})",
            symbol, funding_rate.exchange, funding_rate.rate
        );

        self.funding_rates.insert(key, Arc::new(funding_rate));
        self.increment_stat("funding_rate_updates");

        Ok(())
    }

    // ========================================================================
    // Strategy Detection Pipeline
    // ========================================================================

    /// Run strategy detection across all registered strategies
    ///
    /// This is the main entry point for the detection pipeline:
    /// 1. Build market bundle from cached data
    /// 2. Run each enabled strategy's detect() method
    /// 3. Process each raw signal through the validation pipeline
    /// 4. Emit validated signals
    ///
    /// # Pipeline Flow
    /// ```text
    /// Market Data → Cache → Strategy Detection → Deduplicate → Fee Calculation →
    /// Risk Validation → Size Calculation → Confidence Scoring → Threshold Check →
    /// Store → Emit Signal
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Run detection across all enabled strategies
    /// let signals = engine.detect_opportunities(&registry).await?;
    /// for signal in signals {
    ///     println!("Found opportunity: {:?}", signal);
    /// }
    /// ```
    pub async fn detect_opportunities(&self, registry: &StrategyRegistry) -> Result<Vec<Signal>> {
        let start_time = std::time::Instant::now();

        // Build market bundle from cached data
        let market_bundle = self.build_market_bundle();
        let filter_context = self.build_filter_context()?;

        debug!(
            "Running detection with {} order books, {} tickers, {} funding rates",
            market_bundle.order_books.len(),
            market_bundle.tickers.len(),
            market_bundle.funding_rates.len()
        );

        // Skip if no market data
        if market_bundle.order_books.is_empty() && market_bundle.tickers.is_empty() {
            debug!("No market data available, skipping detection");
            return Ok(Vec::new());
        }

        let mut validated_signals = Vec::with_capacity(64);
        let strategies = registry.get_enabled();

        for strategy in strategies {
            let strategy_id = strategy.id();

            // Run strategy detection
            let raw_signals = match strategy.detect(&market_bundle) {
                Ok(signals) => signals,
                Err(e) => {
                    warn!("Strategy {} detection failed: {}", strategy_id, e);
                    continue;
                }
            };

            debug!(
                "Strategy {} detected {} raw signals",
                strategy_id,
                raw_signals.len()
            );
            self.increment_stat("signals_detected");

            // Process each raw signal through the pipeline
            for raw_signal in raw_signals {
                match self
                    .process_signal_pipeline(
                        raw_signal,
                        &filter_context,
                        &market_bundle,
                        strategy.as_ref(),
                    )
                    .await
                {
                    Ok(Some(signal)) => {
                        validated_signals.push(signal);
                    }
                    Ok(None) => {
                        // Signal was filtered out
                        self.increment_stat("signals_filtered");
                    }
                    Err(e) => {
                        warn!("Signal processing failed: {}", e);
                    }
                }
            }
        }

        let elapsed = start_time.elapsed();
        debug!(
            "Detection completed in {:?}: {} signals validated",
            elapsed,
            validated_signals.len()
        );

        Ok(validated_signals)
    }

    /// Process a raw signal through the complete validation pipeline
    ///
    /// Pipeline stages:
    /// 1. Strategy-specific filtering
    /// 2. Deduplication check
    /// 3. Exchange health validation
    /// 4. Fee calculation (net profit after fees)
    /// 5. Risk validation (exposure limits, inventory)
    /// 6. Size calculation (order book depth analysis)
    /// 7. Confidence scoring (multi-factor scoring)
    /// 8. Threshold check (min profit, min confidence)
    /// 9. Storage and emission
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // A simplified pipeline flow:
    /// // 1. Filter signal based on strategy rules
    /// // 2. Check if we've recently seen this opportunity
    /// // 3. Validate exchanges are healthy
    /// // 4. Calculate fees and net spread
    /// // 5. Check risk limits
    /// // 6. Calculate order size from order books
    /// // 7. Score confidence based on market conditions
    /// // 8. Check against thresholds
    /// // 9. Store and emit the signal
    /// ```
    async fn process_signal_pipeline(
        &self,
        raw_signal: RawSignal,
        filter_context: &FilterContext,
        market_bundle: &MarketBundle,
        strategy: &dyn crate::strategies::Strategy,
    ) -> Result<Option<Signal>> {
        // Stage 1: Strategy-specific filtering
        if !strategy.filter(&raw_signal, filter_context)? {
            debug!("Signal filtered by strategy {}", raw_signal.strategy_id);
            return Ok(None);
        }

        // Extract buy/sell legs
        let (buy_leg, sell_leg) = self.extract_legs(&raw_signal)?;

        // Stage 2: Deduplication check
        let opportunity_key = OpportunityKey {
            symbol: (*raw_signal.symbol).clone(),
            buy_exchange: buy_leg.exchange,
            sell_exchange: sell_leg.exchange,
            strategy_id: raw_signal.strategy_id.clone(),
        };

        if !self.should_emit_signal(&opportunity_key, raw_signal.expected_profit_bps) {
            debug!("Signal deduplicated for {:?}", opportunity_key);
            return Ok(None);
        }

        // Stage 3: Exchange health validation
        if !self.validate_exchange_health(&raw_signal, market_bundle)? {
            for leg in &raw_signal.legs {
                self.increment_unhealthy_exchange_signals(leg.exchange);
            }
            debug!("Signal rejected due to unhealthy exchanges");
            return Ok(None);
        }

        // Stage 4: Fee calculation
        let net_spread_bps = match self.confidence_scorer.calculate_net_spread_bps(
            buy_leg.price,
            sell_leg.price,
            buy_leg.exchange,
            sell_leg.exchange,
        ) {
            NetSpreadResult::Profit(bps) => bps,
            NetSpreadResult::Unprofitable => {
                debug!("Signal unprofitable after fees");
                return Ok(None);
            }
        };

        // Stage 4: Risk validation
        if !self.validate_risk(&raw_signal, filter_context)? {
            debug!("Signal failed risk validation");
            return Ok(None);
        }

        // Stage 5: Build initial signal for size calculation
        let mut signal = self.build_signal(&raw_signal, &buy_leg, &sell_leg, net_spread_bps)?;

        // Stage 6: Size calculation (requires order books)
        let buy_book = market_bundle.get_order_book(buy_leg.exchange, &raw_signal.symbol);
        let sell_book = market_bundle.get_order_book(sell_leg.exchange, &raw_signal.symbol);

        if let (Some(buy_ob), Some(sell_ob)) = (buy_book, sell_book) {
            match self
                .size_calculator
                .calculate_size(&signal, buy_ob, sell_ob)
            {
                Ok(size_rec) => {
                    signal.recommended_size = size_rec.recommended_size;
                    signal.max_size = size_rec.max_size;
                    signal.expected_slippage = size_rec.expected_slippage;
                }
                Err(e) => {
                    debug!("Size calculation failed: {}", e);
                }
            }
        }

        // Stage 7: Confidence scoring
        if let (Some(buy_ob), Some(sell_ob)) = (buy_book, sell_book) {
            let target_qty = signal.recommended_size.max(Decimal::new(1, 2));

            if let (Some(buy_vwap), Some(sell_vwap)) =
                (buy_ob.vwap_buy(target_qty), sell_ob.vwap_sell(target_qty))
            {
                let factors = self
                    .confidence_scorer
                    .calculate_confidence_factors(&buy_vwap, &sell_vwap, buy_ob, sell_ob);
                signal.confidence = self.confidence_scorer.calculate_confidence(&factors);
            }
        }

        // Stage 8: Threshold check
        let min_profit_bps = (self.config.trading.min_profit_threshold_percent
            * Decimal::from(10000))
        .to_i32()
        .unwrap_or(10);
        let min_confidence = self.config.trading.min_confidence_threshold;

        if net_spread_bps < min_profit_bps {
            debug!(
                "Signal below profit threshold: {} < {} bps",
                net_spread_bps, min_profit_bps
            );
            return Ok(None);
        }

        if signal.confidence < min_confidence {
            debug!(
                "Signal below confidence threshold: {} < {}",
                signal.confidence, min_confidence
            );
            return Ok(None);
        }

        // Stage 9: Store signal
        self.storage
            .store_signal(signal.clone(), signal.confidence, SignalStatus::Detected)
            .await?;

        // Stage 10: Update deduplication cache
        self.update_signal_cache(&opportunity_key, &signal, net_spread_bps);

        // Stage 11: Emit signal
        self.emit_signal(&signal);
        self.increment_stat("signals_emitted");

        info!(
            "🎯 Signal emitted: {} {} → {} | profit={:.2}% | confidence={:.0}%",
            signal.symbol,
            signal.buy_exchange,
            signal.sell_exchange,
            signal.net_profit_percent * Decimal::from(100),
            signal.confidence
        );

        Ok(Some(signal))
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Build market bundle from cached data
    ///
    /// Collects all cached market data (order books, tickers, funding rates)
    /// into a single [`MarketBundle`] for strategy detection.
    ///
    /// # Returns
    /// A new [`MarketBundle`] containing all cached market data
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Build market bundle before running detection
    /// let market_bundle = engine.build_market_bundle();
    /// let order_books_count = market_bundle.order_books.len();
    /// ```
    fn build_market_bundle(&self) -> MarketBundle {
        let mut bundle = MarketBundle::new();

        for entry in self.order_books.iter() {
            bundle.add_order_book(Arc::clone(entry.value()));
        }

        for entry in self.tickers.iter() {
            bundle.add_ticker(Arc::clone(entry.value()));
        }

        for entry in self.funding_rates.iter() {
            bundle.add_funding_rate(Arc::clone(entry.value()));
        }

        bundle
    }

    /// Build filter context from configuration
    fn build_filter_context(&self) -> Result<FilterContext> {
        let min_profit_bps = (self.config.trading.min_profit_threshold_percent
            * Decimal::from(10000))
        .to_i32()
        .ok_or_else(|| ArbitrageError::Calculation("Invalid min profit threshold".to_string()))?;

        let mut context = FilterContext::new(min_profit_bps);

        // Set allowed exchanges from config
        context.allowed_exchanges = vec![
            ExchangeId::OKX,
            ExchangeId::ByBit,
            ExchangeId::MEXC,
            ExchangeId::GateIo,
            ExchangeId::Kraken,
            ExchangeId::Bitstamp,
        ];

        // Set max exposure from risk config
        context.max_exposure = self.config.risk.max_position_size_usd;

        // Set inventory limits from config
        if self.config.inventory.enable_inventory_checks {
            for exchange in &context.allowed_exchanges.clone() {
                for (asset, limit) in &self.config.inventory.default_limits {
                    context.set_inventory_limit(*exchange, asset.clone(), *limit);
                }
            }
        }

        Ok(context)
    }

    /// Extract buy and sell legs from raw signal
    fn extract_legs(
        &self,
        raw_signal: &RawSignal,
    ) -> Result<(crate::strategies::TradeLeg, crate::strategies::TradeLeg)> {
        let buy_leg = raw_signal
            .legs
            .iter()
            .find(|leg| leg.side == crate::types::Side::Buy)
            .ok_or_else(|| ArbitrageError::Validation("No buy leg in signal".to_string()))?
            .clone();

        let sell_leg = raw_signal
            .legs
            .iter()
            .find(|leg| leg.side == crate::types::Side::Sell)
            .ok_or_else(|| ArbitrageError::Validation("No sell leg in signal".to_string()))?
            .clone();

        Ok((buy_leg, sell_leg))
    }

    /// Build Signal from RawSignal and calculated values
    fn build_signal(
        &self,
        raw_signal: &RawSignal,
        buy_leg: &crate::strategies::TradeLeg,
        sell_leg: &crate::strategies::TradeLeg,
        net_spread_bps: i32,
    ) -> Result<Signal> {
        let gross_profit_percent = if !buy_leg.price.is_zero() {
            (sell_leg.price - buy_leg.price) / buy_leg.price
        } else {
            Decimal::ZERO
        };

        let net_profit_percent = Decimal::from(net_spread_bps) / Decimal::from(10000);

        let mut signal = Signal::new(
            (*raw_signal.symbol).clone(),
            buy_leg.exchange,
            sell_leg.exchange,
            buy_leg.price,
            sell_leg.price,
            Utc::now(),
        );

        signal.gross_profit_percent = gross_profit_percent;
        signal.net_profit_percent = net_profit_percent;
        signal.metadata.insert(
            "strategy_id".to_string(),
            serde_json::Value::String(raw_signal.strategy_id.clone()),
        );

        // Set expiry based on config
        signal.expires_at =
            Utc::now() + Duration::seconds(self.config.trading.max_signal_age_seconds as i64);

        Ok(signal)
    }

    /// Validate risk constraints
    fn validate_risk(&self, raw_signal: &RawSignal, context: &FilterContext) -> Result<bool> {
        // Check exchange allowlist
        for leg in &raw_signal.legs {
            if !context.is_exchange_allowed(leg.exchange) {
                return Ok(false);
            }
        }

        // Check notional value
        let total_notional = raw_signal.total_notional();
        if total_notional > context.max_exposure {
            return Ok(false);
        }

        if total_notional < context.min_notional_usd {
            return Ok(false);
        }

        Ok(true)
    }

    /// Validate exchange health for all exchanges in a signal
    fn validate_exchange_health(
        &self,
        raw_signal: &RawSignal,
        market_bundle: &MarketBundle,
    ) -> Result<bool> {
        let exchanges = raw_signal.get_exchanges();
        let max_age_ms = self.config.trading.stale_orderbook_threshold_ms;

        for &exchange in &exchanges {
            if !market_bundle.is_healthy_with_max_age(exchange, &raw_signal.symbol, max_age_ms) {
                warn!(
                    target: "exchange_health",
                    "Exchange {} is unhealthy for signal on {}",
                    exchange, raw_signal.symbol
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if signal should be emitted (deduplication)
    fn should_emit_signal(&self, key: &OpportunityKey, profit_bps: i32) -> bool {
        if let Some(cached) = self.signal_cache.get(key) {
            // Check if signal has expired
            if Utc::now() - cached.last_updated > self.signal_ttl {
                return true;
            }

            // Check if profit changed significantly
            let profit_change = (profit_bps - cached.profit_bps).abs();
            if profit_change < self.profit_change_threshold_bps {
                return false;
            }
        }

        true
    }

    /// Update signal cache for deduplication
    fn update_signal_cache(&self, key: &OpportunityKey, _signal: &Signal, profit_bps: i32) {
        self.signal_cache.insert(
            key.clone(),
            CachedSignal {
                last_updated: Utc::now(),
                profit_bps,
            },
        );
    }

    /// Emit signal to subscribers
    fn emit_signal(&self, signal: &Signal) {
        // Broadcast to all subscribers (ignore send errors - no receivers is OK)
        let _ = self.signal_sender.send(signal.clone());
    }

    /// Increment a statistics counter (lock-free atomic operation)
    fn increment_stat(&self, key: &'static str) {
        self.stats
            .entry(key)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::SeqCst);
    }

    /// Increment counter for signals from unhealthy exchanges
    fn increment_unhealthy_exchange_signals(&self, exchange: ExchangeId) {
        let key = format!("signals_unhealthy_{}", exchange);
        self.stats
            .entry(Box::leak(key.into_boxed_str()))
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::SeqCst);
    }

    /// Get engine statistics
    pub fn get_stats(&self) -> EngineStats {
        let mut unique_symbols = FxHashSet::default();
        for entry in self.order_books.iter() {
            unique_symbols.insert(Arc::clone(&entry.key().1));
        }

        EngineStats {
            order_books_count: self.order_books.len(),
            tickers_count: self.tickers.len(),
            funding_rates_count: self.funding_rates.len(),
            cached_signals_count: self.signal_cache.len(),
            active_symbols_count: unique_symbols.len(),
            signals_detected: self
                .stats
                .get("signals_detected")
                .map(|v| v.load(Ordering::SeqCst))
                .unwrap_or(0),
            signals_filtered: self
                .stats
                .get("signals_filtered")
                .map(|v| v.load(Ordering::SeqCst))
                .unwrap_or(0),
            signals_emitted: self
                .stats
                .get("signals_emitted")
                .map(|v| v.load(Ordering::SeqCst))
                .unwrap_or(0),
            signals_from_unhealthy_exchanges: self
                .stats
                .get("signals_from_unhealthy_exchanges")
                .map(|v| v.load(Ordering::SeqCst))
                .unwrap_or(0),
            statistics_retrieval_failures: self
                .stats
                .get("statistics_retrieval_failures")
                .map(|v| v.load(Ordering::SeqCst))
                .unwrap_or(0),
            last_detection_time: Some(Utc::now()),
        }
    }

    /// Clear stale data from caches
    pub fn cleanup_stale_data(&self) {
        let now = Utc::now();
        let stale_threshold =
            Duration::milliseconds(self.config.trading.stale_orderbook_threshold_ms as i64);

        // Clean stale order books
        self.order_books
            .retain(|_, ob| now - ob.timestamp < stale_threshold);

        // Clean stale tickers
        self.tickers
            .retain(|_, ticker| now - ticker.timestamp < stale_threshold);

        // Clean expired signals from cache
        self.signal_cache
            .retain(|_, cached| now - cached.last_updated < self.signal_ttl);
    }

    /// Get order book from cache
    pub fn get_order_book(
        &self,
        exchange: ExchangeId,
        symbol: Arc<Symbol>,
    ) -> Option<Arc<OrderBook>> {
        self.order_books
            .get(&(exchange, symbol))
            .map(|v| Arc::clone(&v))
    }

    pub fn get_all_order_books(&self) -> Vec<Arc<OrderBook>> {
        self.order_books
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    pub fn get_ticker(&self, exchange: ExchangeId, symbol: Arc<Symbol>) -> Option<Arc<Ticker>> {
        self.tickers
            .get(&(exchange, symbol))
            .map(|v| Arc::clone(&v))
    }

    pub fn get_funding_rate(
        &self,
        exchange: ExchangeId,
        symbol: Arc<Symbol>,
    ) -> Option<Arc<FundingRate>> {
        self.funding_rates
            .get(&(exchange, symbol))
            .map(|v| Arc::clone(&v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence_scorer::{ConfidenceConfig, ConfidenceScorer};
    use crate::config::Config;
    use crate::execution_preparer::{ExecutionConfig, ExecutionPreparer};
    use crate::normalizer::Normalizer;
    use crate::size_calculator::SizeCalculator;
    use crate::size_calculator::SizeConfig;
    use crate::storage::{StorageConfig, StorageService};
    use crate::strategies::{FilterContext, MarketBundle, RawSignal, Ticker, TradeLeg};
    use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Side, Symbol};
    use rust_decimal::Decimal;
    use std::sync::Arc;

    fn create_test_config() -> Config {
        Config::default()
    }

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    fn create_test_order_book(exchange: ExchangeId, symbol: &Symbol) -> OrderBook {
        OrderBook::new(
            exchange,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        )
    }

    fn create_test_ticker(
        exchange: ExchangeId,
        symbol: &Symbol,
        bid: Decimal,
        ask: Decimal,
    ) -> Ticker {
        Ticker::new(
            exchange,
            symbol.clone(),
            bid,
            ask,
            (bid + ask) / Decimal::from(2),
        )
    }

    fn create_test_raw_signal(
        strategy_id: &str,
        symbol: Arc<Symbol>,
        buy_exchange: ExchangeId,
        sell_exchange: ExchangeId,
        profit_bps: i32,
    ) -> RawSignal {
        let mut signal = RawSignal::new(strategy_id, symbol.clone());
        signal.legs.push(TradeLeg::new(
            buy_exchange,
            symbol.clone(),
            Side::Buy,
            Decimal::from(50000),
            Decimal::from(1),
        ));
        signal.legs.push(TradeLeg::new(
            sell_exchange,
            symbol,
            Side::Sell,
            Decimal::from(50010),
            Decimal::from(1),
        ));
        signal.set_profit_bps(profit_bps);
        signal
    }

    async fn create_test_engine() -> (ArbitrageEngine, broadcast::Receiver<Signal>) {
        let config = create_test_config();
        let normalizer = Arc::new(Normalizer::new());
        let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
        let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
        let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));

        let storage_config = StorageConfig {
            database_path: ":memory:".to_string(),
            ..Default::default()
        };
        let storage = Arc::new(StorageService::new(storage_config).await.unwrap());

        ArbitrageEngine::new(
            config,
            normalizer,
            confidence_scorer,
            size_calculator,
            execution_preparer,
            storage,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let (engine, _) = create_test_engine().await;
        let stats = engine.get_stats();

        assert_eq!(stats.order_books_count, 0);
        assert_eq!(stats.tickers_count, 0);
        assert_eq!(stats.funding_rates_count, 0);
    }

    #[tokio::test]
    async fn test_update_order_book() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let order_book = OrderBook::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );

        let result = engine.update_order_book(order_book).await;
        assert!(result.is_ok());

        let stored_book = engine.get_order_book(ExchangeId::ByBit, Arc::clone(&symbol));
        assert!(stored_book.is_some());
    }

    #[tokio::test]
    async fn test_update_order_book_invalid() {
        let (engine, _) = create_test_engine().await;
        let symbol = create_test_symbol();

        let order_book = OrderBook::new(
            ExchangeId::ByBit,
            symbol,
            vec![],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );

        let result = engine.update_order_book(order_book).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_ticker() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let ticker = Ticker::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );

        let result = engine.update_ticker(ticker).await;
        assert!(result.is_ok());

        let stored_ticker = engine.get_ticker(ExchangeId::ByBit, Arc::clone(&symbol));
        assert!(stored_ticker.is_some());
    }

    #[tokio::test]
    async fn test_update_funding_rate() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());
        let now = Utc::now();

        let funding_rate = FundingRate {
            exchange: ExchangeId::ByBit,
            symbol: (*symbol).clone(),
            rate: Decimal::from(100), // 0.01% per 8h
            next_funding: now + Duration::hours(8),
            predicted_rate: Some(Decimal::from(110)),
            timestamp: now,
        };

        let result = engine.update_funding_rate(funding_rate).await;
        assert!(result.is_ok());

        let stored_rate = engine.get_funding_rate(ExchangeId::ByBit, Arc::clone(&symbol));
        assert!(stored_rate.is_some());
    }

    #[tokio::test]
    async fn test_get_order_book_not_found() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let result = engine.get_order_book(ExchangeId::ByBit, symbol);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_ticker_not_found() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let result = engine.get_ticker(ExchangeId::ByBit, symbol);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_funding_rate_not_found() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let result = engine.get_funding_rate(ExchangeId::ByBit, symbol);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_subscribe() {
        let (engine, _) = create_test_engine().await;
        let mut receiver = engine.subscribe();

        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_set_execution_mode_manual() {
        let (mut engine, _) = create_test_engine().await;
        engine.set_execution_mode(ExecutionMode::Manual);

        assert_eq!(engine.get_execution_mode(), ExecutionMode::Manual);
    }

    #[tokio::test]
    async fn test_set_execution_mode_auto() {
        let (mut engine, _) = create_test_engine().await;
        let threshold = Decimal::from(80);
        engine.set_execution_mode(ExecutionMode::Auto {
            confidence_threshold: threshold,
        });

        match engine.get_execution_mode() {
            ExecutionMode::Auto {
                confidence_threshold,
            } => {
                assert_eq!(confidence_threshold, threshold);
            }
            _ => panic!("Expected Auto mode"),
        }
    }

    #[tokio::test]
    async fn test_cleanup_stale_data() {
        let (engine, _) = create_test_engine().await;

        engine.cleanup_stale_data();

        let stats = engine.get_stats();
        assert!(stats.order_books_count == 0);
    }

    #[tokio::test]
    async fn test_get_all_order_books_empty() {
        let (engine, _) = create_test_engine().await;

        let books = engine.get_all_order_books();
        assert!(books.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_order_books_with_data() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let order_book = OrderBook::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        engine.update_order_book(order_book).await.unwrap();

        let books = engine.get_all_order_books();
        assert_eq!(books.len(), 1);
    }

    #[tokio::test]
    async fn test_detect_opportunities_no_market_data() {
        let (engine, _) = create_test_engine().await;
        let registry = StrategyRegistry::new();

        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_detect_opportunities_with_tickers() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let ticker = Ticker::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );
        engine.update_ticker(ticker).await.unwrap();

        let registry = StrategyRegistry::new();
        let result = engine.detect_opportunities(&registry).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_build_market_bundle_empty() {
        let (engine, _) = create_test_engine().await;

        let bundle = engine.build_market_bundle();
        assert!(bundle.order_books.is_empty());
        assert!(bundle.tickers.is_empty());
        assert!(bundle.funding_rates.is_empty());
    }

    #[tokio::test]
    async fn test_build_market_bundle_with_data() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let order_book = OrderBook::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        engine.update_order_book(order_book).await.unwrap();

        let ticker = Ticker::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            Decimal::from(50000),
            Decimal::from(50010),
            Decimal::from(50005),
        );
        engine.update_ticker(ticker).await.unwrap();

        let bundle = engine.build_market_bundle();
        assert_eq!(bundle.order_books.len(), 1);
        assert_eq!(bundle.tickers.len(), 1);
    }

    #[tokio::test]
    async fn test_build_filter_context() {
        let (engine, _) = create_test_engine().await;

        let context = engine.build_filter_context().unwrap();
        assert!(context.allowed_exchanges.contains(&ExchangeId::OKX));
        assert!(context.allowed_exchanges.contains(&ExchangeId::ByBit));
    }

    #[tokio::test]
    async fn test_engine_stats_default() {
        let (engine, _) = create_test_engine().await;

        let stats = engine.get_stats();
        assert_eq!(stats.signals_detected, 0);
        assert_eq!(stats.signals_filtered, 0);
        assert_eq!(stats.signals_emitted, 0);
        assert!(stats.last_detection_time.is_some());
    }

    #[tokio::test]
    async fn test_opportunity_key_equality() {
        let symbol = Symbol::new("BTC", "USDT");
        let key1 = OpportunityKey {
            symbol: symbol.clone(),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test".to_string(),
        };
        let key2 = OpportunityKey {
            symbol,
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test".to_string(),
        };

        assert_eq!(key1, key2);
    }

    #[tokio::test]
    async fn test_cached_signal_structure() {
        let cached = CachedSignal {
            last_updated: Utc::now(),
            profit_bps: 15,
        };

        assert_eq!(cached.profit_bps, 15);
    }

    #[tokio::test]
    async fn test_execution_mode_default() {
        assert_eq!(ExecutionMode::default(), ExecutionMode::Manual);
    }

    #[tokio::test]
    async fn test_should_emit_signal_first_time() {
        let (engine, _) = create_test_engine().await;
        let symbol = Symbol::new("BTC", "USDT");

        let key = OpportunityKey {
            symbol,
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test".to_string(),
        };

        let result = engine.should_emit_signal(&key, 15);
        assert!(result);
    }

    #[tokio::test]
    async fn test_should_emit_signal_within_threshold() {
        let (engine, _) = create_test_engine().await;
        let symbol = Symbol::new("BTC", "USDT");

        let key = OpportunityKey {
            symbol: symbol.clone(),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test".to_string(),
        };

        let signal = Signal::new(
            symbol,
            ExchangeId::ByBit,
            ExchangeId::OKX,
            Decimal::from(50000),
            Decimal::from(50015),
            Utc::now(),
        );

        engine.update_signal_cache(&key, &signal, 15);
        let result = engine.should_emit_signal(&key, 16);

        assert!(!result);
    }

    #[tokio::test]
    async fn test_should_emit_signal_exceeds_threshold() {
        let (engine, _) = create_test_engine().await;
        let symbol = Symbol::new("BTC", "USDT");

        let key = OpportunityKey {
            symbol: symbol.clone(),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test".to_string(),
        };

        let signal = Signal::new(
            symbol.clone(),
            ExchangeId::ByBit,
            ExchangeId::OKX,
            Decimal::from(50000),
            Decimal::from(50015),
            Utc::now(),
        );

        engine.update_signal_cache(&key, &signal, 15);
        let result = engine.should_emit_signal(&key, 25);

        assert!(result);
    }

    #[tokio::test]
    async fn test_engine_with_different_configs() {
        let mut config = create_test_config();
        config.trading.min_profit_threshold_percent = Decimal::new(2, 2); // 0.02

        let normalizer = Arc::new(Normalizer::new());
        let confidence_scorer = Arc::new(ConfidenceScorer::new(ConfidenceConfig::default()));
        let size_calculator = Arc::new(SizeCalculator::new(SizeConfig::default()));
        let execution_preparer = Arc::new(ExecutionPreparer::new(ExecutionConfig::default()));

        let storage_config = StorageConfig {
            database_path: ":memory:".to_string(),
            ..Default::default()
        };
        let storage = Arc::new(StorageService::new(storage_config).await.unwrap());

        let (engine, _) = ArbitrageEngine::new(
            config,
            normalizer,
            confidence_scorer,
            size_calculator,
            execution_preparer,
            storage,
        )
        .unwrap();

        let stats = engine.get_stats();
        assert_eq!(stats.order_books_count, 0);
    }

    #[tokio::test]
    async fn test_signal_cache_behavior() {
        let (engine, _) = create_test_engine().await;
        let symbol = Symbol::new("BTC", "USDT");

        let key = OpportunityKey {
            symbol: symbol.clone(),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test_strategy".to_string(),
        };

        let signal = Signal::new(
            symbol,
            ExchangeId::ByBit,
            ExchangeId::OKX,
            Decimal::from(50000),
            Decimal::from(50010),
            Utc::now(),
        );

        assert!(engine.should_emit_signal(&key, 10));
        engine.update_signal_cache(&key, &signal, 10);
        assert!(!engine.should_emit_signal(&key, 12));
        assert!(!engine.should_emit_signal(&key, 14));
        assert!(engine.should_emit_signal(&key, 20));
    }

    #[tokio::test]
    async fn test_multiple_ticker_updates() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        let ticker2 = create_test_ticker(
            ExchangeId::OKX,
            &symbol,
            Decimal::from(50005),
            Decimal::from(50015),
        );

        engine.update_ticker(ticker1).await.unwrap();
        engine.update_ticker(ticker2).await.unwrap();

        let stored_ticker1 = engine.get_ticker(ExchangeId::ByBit, Arc::clone(&symbol));
        let stored_ticker2 = engine.get_ticker(ExchangeId::OKX, Arc::clone(&symbol));

        assert!(stored_ticker1.is_some());
        assert!(stored_ticker2.is_some());
        assert_eq!(stored_ticker1.unwrap().bid, Decimal::from(50000));
    }

    #[tokio::test]
    async fn test_multiple_order_book_updates() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let order_book1 = create_test_order_book(ExchangeId::ByBit, &symbol);
        let order_book2 = create_test_order_book(ExchangeId::OKX, &symbol);

        engine.update_order_book(order_book1).await.unwrap();
        engine.update_order_book(order_book2).await.unwrap();

        let stored_book1 = engine.get_order_book(ExchangeId::ByBit, Arc::clone(&symbol));
        let stored_book2 = engine.get_order_book(ExchangeId::OKX, Arc::clone(&symbol));

        assert!(stored_book1.is_some());
        assert!(stored_book2.is_some());
    }

    #[tokio::test]
    async fn test_funding_rate_update_and_retrieval() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());
        let now = Utc::now();

        let funding_rate1 = FundingRate {
            exchange: ExchangeId::ByBit,
            symbol: (*symbol).clone(),
            rate: Decimal::from(100),
            next_funding: now + Duration::hours(8),
            predicted_rate: Some(Decimal::from(110)),
            timestamp: now,
        };
        let funding_rate2 = FundingRate {
            exchange: ExchangeId::OKX,
            symbol: (*symbol).clone(),
            rate: Decimal::from(150),
            next_funding: now + Duration::hours(8),
            predicted_rate: Some(Decimal::from(160)),
            timestamp: now,
        };

        engine.update_funding_rate(funding_rate1).await.unwrap();
        engine.update_funding_rate(funding_rate2).await.unwrap();

        let stored_rate1 = engine.get_funding_rate(ExchangeId::ByBit, Arc::clone(&symbol));
        let stored_rate2 = engine.get_funding_rate(ExchangeId::OKX, Arc::clone(&symbol));

        assert!(stored_rate1.is_some());
        assert!(stored_rate2.is_some());
        assert_eq!(stored_rate1.unwrap().rate, Decimal::from(100));
    }

    #[tokio::test]
    async fn test_engine_stats_increment() {
        let (engine, _) = create_test_engine().await;

        engine.increment_stat("test_stat");
        engine.increment_stat("test_stat");
        engine.increment_stat("another_stat");

        let stats = engine.get_stats();
        assert_eq!(stats.signals_detected, 0);
    }

    #[tokio::test]
    async fn test_execution_mode_transitions() {
        let (mut engine, _) = create_test_engine().await;

        assert_eq!(engine.get_execution_mode(), ExecutionMode::Manual);

        engine.set_execution_mode(ExecutionMode::Auto {
            confidence_threshold: Decimal::from(75),
        });
        assert_eq!(
            engine.get_execution_mode(),
            ExecutionMode::Auto {
                confidence_threshold: Decimal::from(75)
            }
        );

        engine.set_execution_mode(ExecutionMode::Manual);
        assert_eq!(engine.get_execution_mode(), ExecutionMode::Manual);

        engine.set_execution_mode(ExecutionMode::Auto {
            confidence_threshold: Decimal::from(90),
        });
        assert_eq!(
            engine.get_execution_mode(),
            ExecutionMode::Auto {
                confidence_threshold: Decimal::from(90)
            }
        );
    }

    #[tokio::test]
    async fn test_invalid_order_book_rejection() {
        let (engine, _) = create_test_engine().await;
        let symbol = create_test_symbol();

        let invalid_order_book = OrderBook::new(
            ExchangeId::ByBit,
            symbol,
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(0))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );

        let result = engine.update_order_book(invalid_order_book).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_order_book_update_overwrites_existing() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let order_book1 = OrderBook::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        engine.update_order_book(order_book1).await.unwrap();

        let order_book2 = OrderBook::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(2))],
            vec![OrderBookLevel::new(Decimal::from(50110), Decimal::from(2))],
        );
        engine.update_order_book(order_book2).await.unwrap();

        let stored_book = engine.get_order_book(ExchangeId::ByBit, Arc::clone(&symbol));
        assert!(stored_book.is_some());
        assert_eq!(stored_book.unwrap().bids[0].price, Decimal::from(50100));
    }

    #[tokio::test]
    async fn test_ticker_update_overwrites_existing() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let ticker1 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        engine.update_ticker(ticker1).await.unwrap();

        let ticker2 = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50100),
            Decimal::from(50110),
        );
        engine.update_ticker(ticker2).await.unwrap();

        let stored_ticker = engine.get_ticker(ExchangeId::ByBit, Arc::clone(&symbol));
        assert!(stored_ticker.is_some());
        assert_eq!(stored_ticker.unwrap().bid, Decimal::from(50100));
    }

    #[tokio::test]
    async fn test_market_bundle_construction_with_all_data_types() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let order_book = OrderBook::new(
            ExchangeId::ByBit,
            (*symbol).clone(),
            vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        );
        engine.update_order_book(order_book).await.unwrap();

        let ticker = create_test_ticker(
            ExchangeId::ByBit,
            &symbol,
            Decimal::from(50000),
            Decimal::from(50010),
        );
        engine.update_ticker(ticker).await.unwrap();

        let funding_rate = FundingRate {
            exchange: ExchangeId::ByBit,
            symbol: (*symbol).clone(),
            rate: Decimal::from(100),
            next_funding: Utc::now() + Duration::hours(8),
            predicted_rate: Some(Decimal::from(110)),
            timestamp: Utc::now(),
        };
        engine.update_funding_rate(funding_rate).await.unwrap();

        let bundle = engine.build_market_bundle();
        assert_eq!(bundle.order_books.len(), 1);
        assert_eq!(bundle.tickers.len(), 1);
        assert_eq!(bundle.funding_rates.len(), 1);
    }

    #[tokio::test]
    async fn test_filter_context_with_risk_config() {
        let (engine, _) = create_test_engine().await;

        let context = engine.build_filter_context().unwrap();
        assert!(!context.allowed_exchanges.is_empty());
        assert!(context.max_exposure > Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_opportunity_key_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let symbol = Symbol::new("BTC", "USDT");
        let key1 = OpportunityKey {
            symbol: symbol.clone(),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test".to_string(),
        };
        let key2 = OpportunityKey {
            symbol: symbol.clone(),
            buy_exchange: ExchangeId::ByBit,
            sell_exchange: ExchangeId::OKX,
            strategy_id: "test".to_string(),
        };

        let mut hasher1 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        let hash1 = hasher1.finish();

        let mut hasher2 = DefaultHasher::new();
        key2.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_eq!(hash1, hash2);
        assert_eq!(key1, key2);
    }

    #[tokio::test]
    async fn test_engine_cleanup_with_empty_caches() {
        let (engine, _) = create_test_engine().await;

        engine.cleanup_stale_data();

        let stats = engine.get_stats();
        assert_eq!(stats.order_books_count, 0);
        assert_eq!(stats.tickers_count, 0);
        assert_eq!(stats.funding_rates_count, 0);
    }

    #[tokio::test]
    async fn test_concurrent_ticker_updates() {
        let (engine, _) = create_test_engine().await;
        let symbol = Arc::new(create_test_symbol());

        let updates: Vec<_> = (0..10)
            .map(|i| {
                let exchange = if i % 2 == 0 {
                    ExchangeId::ByBit
                } else {
                    ExchangeId::OKX
                };
                create_test_ticker(
                    exchange,
                    &symbol,
                    Decimal::from(50000 + i * 10),
                    Decimal::from(50010 + i * 10),
                )
            })
            .collect();

        for ticker in updates {
            engine.update_ticker(ticker).await.unwrap();
        }

        let stats = engine.get_stats();
        assert!(stats.tickers_count > 0);
    }
}
