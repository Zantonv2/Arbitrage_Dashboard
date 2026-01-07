use crate::{
    normalizer::Normalizer,
    confidence_scorer::ConfidenceScorer,
    types::{ExchangeId, OrderBook, Signal, Symbol},
    ArbitrageError, Result,
};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Key for identifying unique arbitrage opportunities
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpportunityKey {
    pub symbol: Symbol,
    pub buy_exchange: ExchangeId,
    pub sell_exchange: ExchangeId,
}

/// Cached signal for deduplication
#[derive(Debug, Clone)]
struct CachedSignal {
    signal: Signal,
    last_updated: DateTime<Utc>,
}

/// Core arbitrage computation engine
pub struct ArbitrageEngine {
    /// Order book cache: (exchange, symbol) -> OrderBook
    order_books: Arc<DashMap<(ExchangeId, Symbol), OrderBook>>,
    /// Recent signals for deduplication
    signal_cache: Arc<DashMap<OpportunityKey, CachedSignal>>,
    /// Signal broadcaster
    signal_sender: broadcast::Sender<Signal>,
    /// Normalizer for symbol/fee handling
    normalizer: Arc<Normalizer>,
    /// Confidence scorer for signal validation
    confidence_scorer: Arc<ConfidenceScorer>,
    /// Configuration
    min_profit_threshold: Decimal,
    stale_threshold_ms: u64,
    dedup_window_ms: u64,
    profit_change_threshold: Decimal,
}

impl ArbitrageEngine {
    pub fn new(
        normalizer: Arc<Normalizer>,
        confidence_scorer: Arc<ConfidenceScorer>,
        min_profit_threshold: Decimal,
        stale_threshold_ms: u64,
        dedup_window_ms: u64,
    ) -> (Self, broadcast::Receiver<Signal>) {
        let (signal_sender, signal_receiver) = broadcast::channel(1000);
        
        let engine = Self {
            order_books: Arc::new(DashMap::new()),
            signal_cache: Arc::new(DashMap::new()),
            signal_sender,
            normalizer,
            confidence_scorer,
            min_profit_threshold,
            stale_threshold_ms,
            dedup_window_ms,
            profit_change_threshold: Decimal::new(5, 2), // 5% = 0.05
        };

        (engine, signal_receiver)
    }

    /// Update order book and trigger arbitrage computation
    pub async fn update_order_book(&self, order_book: OrderBook) -> Result<()> {
        let key = (order_book.exchange, order_book.symbol.clone());
        
        debug!(
            "Updating order book for {} on {} with {} bids, {} asks",
            order_book.symbol,
            order_book.exchange,
            order_book.bids.len(),
            order_book.asks.len()
        );

        // Store the order book
        self.order_books.insert(key, order_book.clone());

        // Compute arbitrage opportunities for this symbol
        self.compute_arbitrage_for_symbol(&order_book.symbol).await?;

        Ok(())
    }

    /// Get current order book for exchange/symbol
    pub fn get_order_book(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<OrderBook> {
        self.order_books.get(&(exchange, symbol.clone())).map(|entry| entry.clone())
    }

    /// Get all active order books
    pub fn get_all_order_books(&self) -> Vec<OrderBook> {
        self.order_books.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Compute arbitrage opportunities for a specific symbol
    async fn compute_arbitrage_for_symbol(&self, symbol: &Symbol) -> Result<()> {
        let now = Utc::now();
        let stale_threshold = Duration::milliseconds(self.stale_threshold_ms as i64);

        // Get all order books for this symbol
        let mut books = Vec::new();
        for entry in self.order_books.iter() {
            let ((exchange, book_symbol), order_book) = entry.pair();
            if book_symbol == symbol {
                // Check if order book is stale
                if now - order_book.timestamp > stale_threshold {
                    warn!(
                        "Skipping stale order book for {} on {} (age: {}ms)",
                        symbol,
                        exchange,
                        (now - order_book.timestamp).num_milliseconds()
                    );
                    continue;
                }
                books.push(order_book.clone());
            }
        }

        if books.len() < 2 {
            debug!("Not enough order books for arbitrage computation: {}", books.len());
            return Ok(());
        }

        // Compute all possible arbitrage combinations
        for i in 0..books.len() {
            for j in 0..books.len() {
                if i == j {
                    continue;
                }

                let buy_book = &books[i];
                let sell_book = &books[j];

                if let Some(signal) = self.compute_signal(buy_book, sell_book).await? {
                    self.process_signal(signal).await?;
                }
            }
        }

        Ok(())
    }

    /// Compute a single arbitrage signal between two order books using ConfidenceScorer
    async fn compute_signal(&self, buy_book: &OrderBook, sell_book: &OrderBook) -> Result<Option<Signal>> {
        // Get best prices
        let best_ask = buy_book.best_ask().ok_or_else(|| {
            ArbitrageError::Calculation("No asks in buy order book".to_string())
        })?;
        
        let best_bid = sell_book.best_bid().ok_or_else(|| {
            ArbitrageError::Calculation("No bids in sell order book".to_string())
        })?;

        let buy_price = best_ask.price;
        let sell_price = best_bid.price;

        // Early exit if no arbitrage opportunity
        if buy_price >= sell_price {
            return Ok(None);
        }

        // Use ConfidenceScorer for fee-aware profit calculation
        let net_spread_bps = self.confidence_scorer.calculate_net_spread_bps(
            buy_price,
            sell_price,
            buy_book.exchange,
            sell_book.exchange,
        )?;

        // Convert BPS to percentage for comparison with threshold
        let net_profit_percent = Decimal::from(net_spread_bps) / Decimal::from(100);

        // Check if profit meets minimum threshold
        if net_profit_percent < self.min_profit_threshold {
            debug!(
                "Signal below threshold: {:.4}% < {:.4}% for {}-{} vs {}-{}",
                net_profit_percent,
                self.min_profit_threshold,
                buy_book.exchange,
                buy_book.symbol,
                sell_book.exchange,
                sell_book.symbol
            );
            return Ok(None);
        }

        // Check data freshness using ConfidenceScorer
        if !self.confidence_scorer.is_data_fresh(buy_book.timestamp) ||
           !self.confidence_scorer.is_data_fresh(sell_book.timestamp) {
            debug!(
                "Stale data detected for {}-{} arbitrage opportunity",
                buy_book.exchange,
                sell_book.exchange
            );
            return Ok(None);
        }

        // Create signal with proper profit calculation
        let signal = Signal::new(
            buy_book.symbol.clone(),
            buy_book.exchange,
            sell_book.exchange,
            buy_price,
            sell_price,
        );

        debug!(
            "Arbitrage opportunity detected: Buy {} at {} for {}, Sell at {} for {} (Net: {:.4}%)",
            signal.symbol,
            buy_book.exchange,
            buy_price,
            sell_book.exchange,
            sell_price,
            net_profit_percent
        );

        Ok(Some(signal))
    }

    /// Process and potentially emit a signal
    async fn process_signal(&self, signal: Signal) -> Result<()> {
        let opportunity_key = OpportunityKey {
            symbol: signal.symbol.clone(),
            buy_exchange: signal.buy_exchange,
            sell_exchange: signal.sell_exchange,
        };

        let now = Utc::now();
        let should_emit = if let Some(cached) = self.signal_cache.get(&opportunity_key) {
            let time_since_last = now - cached.last_updated;
            let profit_change = (signal.net_profit_percent - cached.signal.net_profit_percent).abs();

            // Emit if outside deduplication window or significant profit change
            time_since_last.num_milliseconds() > self.dedup_window_ms as i64 ||
            profit_change > self.profit_change_threshold
        } else {
            true // First time seeing this opportunity
        };

        if should_emit {
            // Update cache
            self.signal_cache.insert(opportunity_key, CachedSignal {
                signal: signal.clone(),
                last_updated: now,
            });

            // Emit signal
            if let Err(e) = self.signal_sender.send(signal.clone()) {
                warn!("Failed to broadcast signal: {}", e);
            }

            debug!("Emitted signal: {}", signal.id);
        }

        Ok(())
    }

    /// Clean up expired signals from cache
    pub async fn cleanup_expired_signals(&self) {
        let now = Utc::now();
        let cleanup_threshold = Duration::milliseconds(self.dedup_window_ms as i64 * 2);

        self.signal_cache.retain(|_, cached| {
            now - cached.last_updated < cleanup_threshold
        });

        debug!("Cleaned up expired signals, cache size: {}", self.signal_cache.len());
    }

    /// Get signal receiver for subscribing to signals
    pub fn subscribe(&self) -> broadcast::Receiver<Signal> {
        self.signal_sender.subscribe()
    }

    /// Get current cache statistics with enhanced metrics
    pub fn get_stats(&self) -> ArbitrageEngineStats {
        let now = Utc::now();
        let stale_threshold = Duration::milliseconds(self.stale_threshold_ms as i64);
        
        // Count stale order books
        let mut stale_books_count = 0;
        let mut fresh_books_count = 0;
        
        for entry in self.order_books.iter() {
            let (_, order_book) = entry.pair();
            if now - order_book.timestamp > stale_threshold {
                stale_books_count += 1;
            } else {
                fresh_books_count += 1;
            }
        }
        
        // Count active symbols
        let mut active_symbols = std::collections::HashSet::new();
        for entry in self.order_books.iter() {
            let ((_, symbol), _) = entry.pair();
            active_symbols.insert(symbol.clone());
        }
        
        ArbitrageEngineStats {
            order_books_count: self.order_books.len(),
            cached_signals_count: self.signal_cache.len(),
            min_profit_threshold: self.min_profit_threshold,
            stale_books_count,
            fresh_books_count,
            active_symbols_count: active_symbols.len(),
            dedup_window_ms: self.dedup_window_ms,
            stale_threshold_ms: self.stale_threshold_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageEngineStats {
    pub order_books_count: usize,
    pub cached_signals_count: usize,
    pub min_profit_threshold: Decimal,
    pub stale_books_count: usize,
    pub fresh_books_count: usize,
    pub active_symbols_count: usize,
    pub dedup_window_ms: u64,
    pub stale_threshold_ms: u64,
}