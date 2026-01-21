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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

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
    order_books: Arc<DashMap<(ExchangeId, Arc<Symbol>), Arc<OrderBook>>>,
    tickers: Arc<DashMap<(ExchangeId, Arc<Symbol>), Arc<Ticker>>>,
    funding_rates: Arc<DashMap<(ExchangeId, Arc<Symbol>), Arc<FundingRate>>>,

    // Signal deduplication cache
    signal_cache: Arc<DashMap<OpportunityKey, CachedSignal>>,

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

        let engine = Self {
            order_books: Arc::new(DashMap::new()),
            tickers: Arc::new(DashMap::new()),
            funding_rates: Arc::new(DashMap::new()),
            signal_cache: Arc::new(DashMap::new()),
            signal_sender,
            normalizer,
            confidence_scorer,
            size_calculator,
            execution_preparer,
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
    /// 1. Strategy-specific filtering
    /// 2. Deduplication check
    /// 3. Exchange health validation
    /// 4. Fee calculation (net profit after fees)
    /// 5. Risk validation (exposure limits, inventory)
    /// 6. Size calculation (order book depth analysis)
    /// 7. Confidence scoring (multi-factor scoring)
    /// 8. Threshold check (min profit, min confidence)
    /// 9. Storage and emission
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

    /// Handle statistics retrieval failure with logging and metrics
    fn handle_stats_retrieval_failure(&self, error_message: &str) {
        self.increment_stats_retrieval_failure();
        error!(
            target: "statistics",
            "Statistics retrieval failed: {}", error_message
        );
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

    /// Increment counter for statistics retrieval failures
    fn increment_stats_retrieval_failure(&self) {
        self.increment_stat("statistics_retrieval_failures");
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
