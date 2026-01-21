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
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use std::ops::Deref;

// ============================================================================
// LRU Cache Implementation
// ============================================================================

const _MAX_CACHE_SIZE: usize = 10000;

struct LruCache<K: Clone + Eq + std::hash::Hash, V> {
    max_size: usize,
    map: DashMap<K, V>,
    access_order: Mutex<VecDeque<K>>,
    size: AtomicUsize,
    evictions: AtomicU64,
}

impl<K: Clone + Eq + std::hash::Hash + std::fmt::Debug, V> LruCache<K, V> {
    fn new(max_size: usize) -> Self {
        Self {
            max_size,
            map: DashMap::new(),
            access_order: Mutex::new(VecDeque::new()),
            size: AtomicUsize::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    fn get<Q: std::hash::Hash + Eq>(&self, key: &Q) -> Option<impl Deref<Target = V> + '_>
    where
        K: std::borrow::Borrow<Q>,
    {
        if let Some(entry) = self.map.get(key) {
            let key_cloned = entry.key().clone();
            let mut access = self.access_order.lock().unwrap();
            if let Some(pos) = access.iter().position(|k| k == &key_cloned) {
                access.remove(pos);
            }
            access.push_back(key_cloned);
            Some(entry)
        } else {
            None
        }
    }

    fn insert(&self, key: K, value: V) {
        let mut access = self.access_order.lock().unwrap();
        let is_new = !self.map.contains_key(&key);

        if is_new {
            // Evict entries if we're at capacity before adding
            while self.size.load(Ordering::SeqCst) >= self.max_size {
                if let Some(oldest) = access.pop_front() {
                    self.map.remove(&oldest);
                    self.size.fetch_sub(1, Ordering::SeqCst);
                    let ev_count = self.evictions.fetch_add(1, Ordering::SeqCst);
                    debug!("Evicted LRU entry: {:?}", oldest);
                    if ev_count > 0 && ev_count % 1000 == 0 {
                        warn!("High cache eviction rate: {} total evictions", ev_count);
                    }
                }
            }
            self.size.fetch_add(1, Ordering::SeqCst);
        } else {
            // Update existing entry - move to back of access order
            if let Some(pos) = access.iter().position(|k| k == &key) {
                access.remove(pos);
            }
        }

        self.map.insert(key.clone(), value);
        access.push_back(key);
    }

    fn contains_key<Q: std::hash::Hash + Eq>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
    {
        self.map.contains_key(key)
    }

    fn len(&self) -> usize {
        self.size.load(Ordering::SeqCst)
    }

    fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::SeqCst)
    }

    fn entry_count(&self) -> u64 {
        self.map.len() as u64
    }

    fn invalidate<Q: std::hash::Hash + Eq>(&self, key: &Q)
    where
        K: std::borrow::Borrow<Q>,
    {
        let mut access = self.access_order.lock().unwrap();
        if let Some(k) = self.map.get(key).map(|e| e.key().clone()) {
            self.map.remove(key);
            if let Some(pos) = access.iter().position(|x| x == &k) {
                access.remove(pos);
            }
            self.size.fetch_sub(1, Ordering::SeqCst);
        }
    }

    fn iter(&self) -> dashmap::iter::Iter<'_, K, V> {
        self.map.iter()
    }

    fn retain<F: FnMut(&K, &V) -> bool>(&self, mut f: F) {
        let mut access = self.access_order.lock().unwrap();
        let keys_to_remove: Vec<K> = self
            .map
            .iter()
            .filter(|entry| !f(entry.key(), entry.value()))
            .map(|entry| entry.key().clone())
            .collect();

        for key in &keys_to_remove {
            self.map.remove(key);
            if let Some(pos) = access.iter().position(|k| k == key) {
                access.remove(pos);
            }
        }
        self.size.store(self.map.len(), Ordering::SeqCst);
    }
}

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
    pub cache_evictions: u64,
    pub cache_eviction_rate: f64,
    pub memory_pressure_percent: usize,
    pub last_detection_time: Option<DateTime<Utc>>,
}

/// Cache statistics for monitoring memory pressure and eviction patterns
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub current_size: Arc<AtomicUsize>,
    pub max_size: usize,
    pub total_evictions: Arc<AtomicU64>,
    pub memory_usage_estimate_bytes: u64,
    pub is_under_memory_pressure: bool,
    pub eviction_alert_threshold: u64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            current_size: Arc::new(AtomicUsize::new(0)),
            max_size: 10000,
            total_evictions: Arc::new(AtomicU64::new(0)),
            memory_usage_estimate_bytes: 0,
            is_under_memory_pressure: false,
            eviction_alert_threshold: 1000,
        }
    }
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
    order_books: Arc<DashMap<(ExchangeId, Arc<Symbol>), Arc<OrderBook>>>,
    tickers: Arc<DashMap<(ExchangeId, Arc<Symbol>), Arc<Ticker>>>,
    funding_rates: Arc<DashMap<(ExchangeId, Arc<Symbol>), Arc<FundingRate>>>,

    // Signal deduplication cache with LRU eviction
    signal_cache: Arc<LruCache<OpportunityKey, CachedSignal>>,

    // Cache statistics for monitoring
    cache_stats: Arc<CacheStats>,
    recent_evictions_counter: Arc<AtomicU64>,

    // Signal broadcasting
    signal_sender: broadcast::Sender<Signal>,

    // Core modules
    #[allow(dead_code)]
    normalizer: Arc<Normalizer>,
    confidence_scorer: Arc<ConfidenceScorer>,
    size_calculator: Arc<SizeCalculator>,
    #[allow(dead_code)]
    execution_preparer: Arc<ExecutionPreparer>,
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
        let (signal_sender, signal_receiver) = broadcast::channel(1000);

        let cache_max_size = config.cache.signal_cache_max_size;
        let cache_stats = Arc::new(CacheStats::default());
        let recent_evictions_counter = Arc::new(AtomicU64::new(0));
        let _recent_evictions_clone = recent_evictions_counter.clone();
        let _cache_stats_clone = cache_stats.clone();
        let cache_stats_for_engine = cache_stats.clone();

        let signal_cache = LruCache::new(cache_max_size);

        let engine = Self {
            order_books: Arc::new(DashMap::new()),
            tickers: Arc::new(DashMap::new()),
            funding_rates: Arc::new(DashMap::new()),
            signal_cache: Arc::new(signal_cache),
            cache_stats: cache_stats_for_engine,
            recent_evictions_counter,
            signal_sender,
            normalizer,
            confidence_scorer,
            size_calculator,
            execution_preparer,
            storage,
            config,
            execution_mode: ExecutionMode::default(),
            profit_change_threshold_bps: 5,
            signal_ttl: Duration::seconds(300),
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

        let mut validated_signals = Vec::new();
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
    /// 1. Strategy filtering (strategy-specific rules)
    /// 2. Deduplication check
    /// 3. Fee calculation (net profit after fees)
    /// 4. Risk validation (exposure limits, inventory)
    /// 5. Size calculation (order book depth analysis)
    /// 6. Confidence scoring (multi-factor scoring)
    /// 7. Threshold check (min profit, min confidence)
    /// 8. Storage and emission
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

        // Stage 3: Fee calculation
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
        let was_present = self.signal_cache.get(key).is_some();

        self.signal_cache.insert(
            key.clone(),
            CachedSignal {
                last_updated: Utc::now(),
                profit_bps,
            },
        );

        // Track cache size manually since entry_count() can be unreliable
        if !was_present {
            let current = self.cache_stats.current_size.fetch_add(1, Ordering::SeqCst);
            tracing::debug!("Cache entry added: {:?}, new size: {}", key, current + 1);
        }
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

    /// Get engine statistics
    pub fn get_stats(&self) -> EngineStats {
        let mut unique_symbols = FxHashSet::default();
        for entry in self.order_books.iter() {
            unique_symbols.insert(Arc::clone(&entry.key().1));
        }

        let cache_size = self.get_cache_size();
        let total_evictions = self.signal_cache.evictions();
        let recent_evictions = self.recent_evictions_counter.swap(0, Ordering::SeqCst);

        let memory_usage_estimate = cache_size as u64 * std::mem::size_of::<CachedSignal>() as u64;
        let memory_limit = self.config.cache.signal_cache_memory_limit_mb as u64 * 1024 * 1024;
        let memory_pressure_percent = if memory_limit > 0 {
            ((memory_usage_estimate as f64 / memory_limit as f64) * 100.0) as usize
        } else {
            0
        };

        EngineStats {
            order_books_count: self.order_books.len(),
            tickers_count: self.tickers.len(),
            funding_rates_count: self.funding_rates.len(),
            cached_signals_count: cache_size,
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
            cache_evictions: total_evictions,
            cache_eviction_rate: if total_evictions > 0 {
                recent_evictions as f64 / total_evictions as f64
            } else {
                0.0
            },
            memory_pressure_percent,
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

        // Clean expired signals from cache by iterating and removing stale entries
        let ttl_seconds = self.signal_ttl.num_seconds();
        let now = Utc::now();

        let mut keys_to_remove = Vec::new();
        for item in self.signal_cache.iter() {
            let key = item.key();
            let cached = item.value();
            let age = now - cached.last_updated;
            if age.num_seconds() > ttl_seconds {
                keys_to_remove.push(key.clone());
            }
        }

        for key in keys_to_remove {
            self.signal_cache.invalidate(&key);
        }

        // Update cache stats
        self.cache_stats
            .current_size
            .store(self.signal_cache.len(), Ordering::SeqCst);
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

    /// Get cache statistics for monitoring
    pub fn get_cache_stats(&self) -> CacheStats {
        let cache_size = self.signal_cache.len();
        let memory_usage_estimate = cache_size as u64 * std::mem::size_of::<CachedSignal>() as u64;
        let memory_limit = self.config.cache.signal_cache_memory_limit_mb as u64 * 1024 * 1024;

        let memory_pressure_percent = if memory_limit > 0 {
            ((memory_usage_estimate as f64 / memory_limit as f64) * 100.0) as usize
        } else {
            0
        };

        let is_under_memory_pressure =
            memory_pressure_percent >= self.config.cache.memory_pressure_threshold_percent as usize;

        CacheStats {
            current_size: Arc::new(AtomicUsize::new(cache_size)),
            max_size: self.config.cache.signal_cache_max_size,
            total_evictions: Arc::new(AtomicU64::new(self.signal_cache.evictions())),
            memory_usage_estimate_bytes: memory_usage_estimate,
            is_under_memory_pressure,
            eviction_alert_threshold: self.config.cache.eviction_alert_threshold,
        }
    }

    /// Check and report memory pressure status
    pub fn check_memory_pressure(&self) -> bool {
        let stats = self.get_cache_stats();
        if stats.is_under_memory_pressure {
            warn!(
                "Memory pressure detected: {}% (limit: {}%)",
                stats.memory_usage_estimate_bytes,
                self.config.cache.memory_pressure_threshold_percent
            );
        }
        stats.is_under_memory_pressure
    }

    /// Get the current cache size
    pub fn get_cache_size(&self) -> usize {
        self.signal_cache.len()
    }

    /// Get the configured max cache size
    pub fn get_max_cache_size(&self) -> usize {
        self.config.cache.signal_cache_max_size
    }

    pub fn update_signal_cache_for_test(&self, key: &OpportunityKey, profit_bps: i32) {
        let size_before = self.signal_cache.len();
        self.update_signal_cache(
            key,
            &Signal::new(
                key.symbol.clone(),
                key.buy_exchange,
                key.sell_exchange,
                Decimal::from(0),
                Decimal::from(0),
                Utc::now(),
            ),
            profit_bps,
        );
        let size_after = self.signal_cache.len();
        tracing::debug!(
            "Cache update: key={:?}, profit_bps={}, size_before={}, size_after={}",
            key,
            profit_bps,
            size_before,
            size_after
        );
    }

    pub fn process_should_emit_for_test(&self, key: &OpportunityKey, profit_bps: i32) -> bool {
        self.should_emit_signal(key, profit_bps)
    }
}
