use crate::{
    normalizer::Normalizer,
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
    /// Configuration
    min_profit_threshold: Decimal,
    stale_threshold_ms: u64,
    dedup_window_ms: u64,
    profit_change_threshold: Decimal,
}

impl ArbitrageEngine {
    pub fn new(
        normalizer: Arc<Normalizer>,
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
            min_profit_threshold,
            stale_threshold_ms,
            dedup_window_ms,
            profit_change_threshold: Decimal::from_str_exact("0.05").unwrap(), // 5% change threshold
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

    /// Compute a single arbitrage signal between two order books
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

        // Calculate gross profit percentage
        if buy_price >= sell_price {
            return Ok(None); // No arbitrage opportunity
        }

        let gross_profit_percent = ((sell_price - buy_price) / buy_price) * Decimal::from(100);

        // Calculate net profit after fees
        let buy_fee_schedule = self.normalizer.get_fee_schedule(buy_book.exchange);
        let sell_fee_schedule = self.normalizer.get_fee_schedule(sell_book.exchange);

        let buy_fee = buy_fee_schedule
            .map(|fs| fs.taker_fee)
            .unwrap_or(Decimal::ZERO);
        let sell_fee = sell_fee_schedule
            .map(|fs| fs.taker_fee)
            .unwrap_or(Decimal::ZERO);

        let total_fee_percent = (buy_fee + sell_fee) * Decimal::from(100);
        let net_profit_percent = gross_profit_percent - total_fee_percent;

        // Check if profit meets threshold
        if net_profit_percent < self.min_profit_threshold {
            return Ok(None);
        }

        // Calculate absolute profit for a base quantity (1 unit)
        let base_quantity = Decimal::ONE;
        let gross_profit_absolute = (sell_price - buy_price) * base_quantity;
        let fee_absolute = (buy_price * buy_fee + sell_price * sell_fee) * base_quantity;
        let net_profit_absolute = gross_profit_absolute - fee_absolute;

        // Create signal
        let mut signal = Signal::new(
            buy_book.symbol.clone(),
            buy_book.exchange,
            sell_book.exchange,
            buy_price,
            sell_price,
        );

        signal.gross_profit_percent = gross_profit_percent;
        signal.net_profit_percent = net_profit_percent;
        signal.net_profit_absolute = net_profit_absolute;

        // Estimate execution time (placeholder - would be based on exchange latencies)
        signal.estimated_execution_time_ms = 500; // 500ms default

        debug!(
            "Computed signal: {} buy {} @ {} sell {} @ {} profit {:.2}%",
            signal.symbol,
            signal.buy_exchange,
            signal.buy_price,
            signal.sell_exchange,
            signal.sell_price,
            signal.net_profit_percent
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

    /// Get current cache statistics
    pub fn get_stats(&self) -> ArbitrageEngineStats {
        ArbitrageEngineStats {
            order_books_count: self.order_books.len(),
            cached_signals_count: self.signal_cache.len(),
            min_profit_threshold: self.min_profit_threshold,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageEngineStats {
    pub order_books_count: usize,
    pub cached_signals_count: usize,
    pub min_profit_threshold: Decimal,
}