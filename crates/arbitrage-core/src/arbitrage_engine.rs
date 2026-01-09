use crate::{
    normalizer::Normalizer,
    confidence_scorer::ConfidenceScorer,
    storage::StorageService,
    strategies::{StrategyRegistry, MarketBundle, FilterContext, RawSignal, Ticker, FundingRate},
    types::{ExchangeId, OrderBook, Signal, Symbol},
    config::Config,
    ArbitrageError, Result,
};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, warn, info};

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

/// Core arbitrage computation engine with strategy integration
pub struct ArbitrageEngine {
    /// Order book cache: (exchange, symbol) -> OrderBook
    order_books: Arc<DashMap<(ExchangeId, Symbol), OrderBook>>,
    /// Ticker cache: (exchange, symbol) -> Ticker
    tickers: Arc<DashMap<(ExchangeId, Symbol), Ticker>>,
    /// Funding rate cache: (exchange, symbol) -> FundingRate
    funding_rates: Arc<DashMap<(ExchangeId, Symbol), FundingRate>>,
    /// Recent signals for deduplication
    signal_cache: Arc<DashMap<OpportunityKey, CachedSignal>>,
    /// Signal broadcaster
    signal_sender: broadcast::Sender<Signal>,
    /// Normalizer for symbol/fee handling
    normalizer: Arc<Normalizer>,
    /// Confidence scorer for signal validation
    confidence_scorer: Arc<ConfidenceScorer>,
    /// Storage service for persistence
    storage: Arc<StorageService>,
    /// Configuration
    config: Config,
    /// Profit change threshold for deduplication
    profit_change_threshold: Decimal,
}

impl ArbitrageEngine {
    pub fn new(
        config: Config,
        normalizer: Arc<Normalizer>,
        confidence_scorer: Arc<ConfidenceScorer>,
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
            storage,
            config,
            profit_change_threshold: Decimal::new(5, 2), // 5% = 0.05
        };

        Ok((engine, signal_receiver))
    }

    /// Update order book and trigger strategy-based arbitrage computation
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

        Ok(())
    }

    /// Update ticker data
    pub async fn update_ticker(&self, ticker: Ticker) -> Result<()> {
        let key = (ticker.exchange, ticker.symbol.clone());
        
        debug!(
            "Updating ticker for {} on {} - price: {}",
            ticker.symbol,
            ticker.exchange,
            ticker.last
        );

        self.tickers.insert(key, ticker);
        Ok(())
    }

    /// Update funding rate data
    pub async fn update_funding_rate(&self, funding_rate: FundingRate) -> Result<()> {
        let key = (funding_rate.exchange, funding_rate.symbol.clone());
        
        debug!(
            "Updating funding rate for {} on {} - rate: {}",
            funding_rate.symbol,
            funding_rate.exchange,
            funding_rate.rate
        );

        self.funding_rates.insert(key, funding_rate);
        Ok(())
    }

    /// Run strategy detection across all registered strategies
    pub async fn detect_opportunities(&self, registry: &StrategyRegistry) -> Result<Vec<Signal>> {
        let market_bundle = self.create_market_bundle()?;
        let filter_context = self.create_filter_context()?;
        
        let mut all_signals = Vec::new();
        
        // Get all enabled strategies
        let strategies = registry.get_enabled();
        
        info!("Running {} strategies for opportunity detection", strategies.len());
        
        for strategy in strategies {
            let strategy_id = strategy.id();
            debug!("Running strategy: {}", strategy_id);
            
            // Detect raw signals
            let raw_signals: Vec<RawSignal> = match strategy.detect(&market_bundle) {
                Ok(signals) => signals,
                Err(e) => {
                    warn!("Strategy {} detection failed: {}", strategy_id, e);
                    continue;
                }
            };
            
            debug!("Strategy {} detected {} raw signals", strategy_id, raw_signals.len());
            println!("Strategy {} detected {} raw signals", strategy_id, raw_signals.len());
            
            // Filter and convert signals
            for (i, raw_signal) in raw_signals.into_iter().enumerate() {
                println!("Processing raw signal {}: {:?}", i, raw_signal.strategy_id);
                
                // Apply strategy-specific filtering
                let is_valid = match strategy.filter(&raw_signal, &filter_context) {
                    Ok(valid) => {
                        println!("Signal {} filter result: {}", i, valid);
                        valid
                    },
                    Err(e) => {
                        println!("Signal {} filtering failed: {}", i, e);
                        warn!("Strategy {} filtering failed: {}", strategy_id, e);
                        continue;
                    }
                };
                
                if !is_valid {
                    println!("Signal {} filtered out by strategy {}", i, strategy_id);
                    debug!("Signal filtered out by strategy {}", strategy_id);
                    continue;
                }
                
                println!("Signal {} passed filtering, converting to Signal", i);
                
                // Convert RawSignal to Signal
                let signal = match self.convert_raw_signal_to_signal(raw_signal).await {
                    Ok(s) => {
                        println!("Signal {} converted successfully", i);
                        s
                    },
                    Err(e) => {
                        println!("Signal {} conversion failed: {}", i, e);
                        continue;
                    }
                };
                
                // Apply deduplication
                let should_emit = match self.should_emit_signal(&signal).await {
                    Ok(emit) => {
                        println!("Signal {} deduplication result: {}", i, emit);
                        emit
                    },
                    Err(e) => {
                        println!("Signal {} deduplication failed: {}", i, e);
                        continue;
                    }
                };
                
                if should_emit {
                    println!("Signal {} added to final results", i);
                    all_signals.push(signal);
                } else {
                    println!("Signal {} filtered out by deduplication", i);
                }
            }
        }
        
        info!("Total signals after filtering and deduplication: {}", all_signals.len());
        
        // Store signals in database (without await since store_signal is not async)
        for signal in &all_signals {
            if let Err(e) = self.storage.store_signal(
                signal.clone(),
                Decimal::from(75), // Default confidence score
                crate::storage::SignalStatus::Detected,
            ) {
                warn!("Failed to store signal {}: {}", signal.id, e);
            }
        }
        
        // Broadcast signals
        for signal in &all_signals {
            if let Err(e) = self.signal_sender.send(signal.clone()) {
                warn!("Failed to broadcast signal {}: {}", signal.id, e);
            }
        }
        
        Ok(all_signals)
    }

    /// Create market bundle from current market data
    fn create_market_bundle(&self) -> Result<MarketBundle> {
        let mut bundle = MarketBundle::new();
        
        // Add all order books
        for entry in self.order_books.iter() {
            let order_book = entry.value().clone();
            bundle.add_order_book(order_book);
        }
        
        // Add all tickers
        for entry in self.tickers.iter() {
            let ticker: Ticker = entry.value().clone();
            bundle.add_ticker(ticker);
        }
        
        // Add all funding rates
        for entry in self.funding_rates.iter() {
            let funding_rate: FundingRate = entry.value().clone();
            bundle.add_funding_rate(funding_rate);
        }
        
        debug!("Created market bundle with {} order books, {} tickers, {} funding rates",
               self.order_books.len(), self.tickers.len(), self.funding_rates.len());
        
        Ok(bundle)
    }

    /// Create filter context from configuration
    fn create_filter_context(&self) -> Result<FilterContext> {
        let min_profit_bps = (self.config.trading.min_profit_threshold_percent * Decimal::from(10000))
            .to_i32()
            .ok_or_else(|| ArbitrageError::Calculation("Invalid min profit threshold".to_string()))?;
            
        let context = FilterContext::new(min_profit_bps);
        
        Ok(context)
    }

    /// Convert RawSignal to Signal
    async fn convert_raw_signal_to_signal(&self, raw_signal: RawSignal) -> Result<Signal> {
        // For now, create a basic signal from the first two legs
        if raw_signal.legs.len() < 2 {
            return Err(ArbitrageError::Validation("Signal must have at least 2 legs".to_string()));
        }
        
        let buy_leg = &raw_signal.legs[0];
        let sell_leg = &raw_signal.legs[1];
        
        let signal = Signal::new(
            raw_signal.symbol,
            buy_leg.exchange,
            sell_leg.exchange,
            buy_leg.price,
            sell_leg.price,
        );
        
        Ok(signal)
    }

    /// Check if signal should be emitted (deduplication logic)
    async fn should_emit_signal(&self, signal: &Signal) -> Result<bool> {
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
            time_since_last.num_milliseconds() > self.config.trading.signal_deduplication_window_ms as i64 ||
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
        }

        Ok(should_emit)
    }

    /// Get current order book for exchange/symbol
    pub fn get_order_book(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<OrderBook> {
        self.order_books.get(&(exchange, symbol.clone())).map(|entry| entry.clone())
    }

    /// Get current ticker for exchange/symbol
    pub fn get_ticker(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<Ticker> {
        self.tickers.get(&(exchange, symbol.clone())).map(|entry| entry.value().clone())
    }

    /// Get current funding rate for exchange/symbol
    pub fn get_funding_rate(&self, exchange: ExchangeId, symbol: &Symbol) -> Option<FundingRate> {
        self.funding_rates.get(&(exchange, symbol.clone())).map(|entry| entry.value().clone())
    }

    /// Get all active order books
    pub fn get_all_order_books(&self) -> Vec<OrderBook> {
        self.order_books.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get all active tickers
    pub fn get_all_tickers(&self) -> Vec<Ticker> {
        self.tickers.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get all active funding rates
    pub fn get_all_funding_rates(&self) -> Vec<FundingRate> {
        self.funding_rates.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Clean up expired signals from cache
    pub async fn cleanup_expired_signals(&self) {
        let now = Utc::now();
        let cleanup_threshold = Duration::milliseconds(self.config.trading.signal_deduplication_window_ms as i64 * 2);

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
        let stale_threshold = Duration::milliseconds(self.config.trading.stale_orderbook_threshold_ms as i64);
        
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
            tickers_count: self.tickers.len(),
            funding_rates_count: self.funding_rates.len(),
            cached_signals_count: self.signal_cache.len(),
            min_profit_threshold: self.config.trading.min_profit_threshold_percent,
            stale_books_count,
            fresh_books_count,
            active_symbols_count: active_symbols.len(),
            dedup_window_ms: self.config.trading.signal_deduplication_window_ms,
            stale_threshold_ms: self.config.trading.stale_orderbook_threshold_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageEngineStats {
    pub order_books_count: usize,
    pub tickers_count: usize,
    pub funding_rates_count: usize,
    pub cached_signals_count: usize,
    pub min_profit_threshold: Decimal,
    pub stale_books_count: usize,
    pub fresh_books_count: usize,
    pub active_symbols_count: usize,
    pub dedup_window_ms: u64,
    pub stale_threshold_ms: u64,
}