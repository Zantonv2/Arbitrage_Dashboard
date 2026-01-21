//! Symbol discovery and market data analysis.
//!
//! This module provides services for discovering and monitoring trading pairs
//! across exchanges, with scoring and prioritization based on arbitrage potential.
//!
//! # Symbol Discovery Service
///
/// The [`SymbolDiscoveryService`] handles:
/// - Collecting market data from multiple exchanges
/// - Scoring symbols by arbitrage potential, liquidity, and stability
/// - Emitting events for symbol additions/removals
/// - Historical tracking for trend analysis
///
/// # Scoring System
///
/// Symbols are scored on multiple dimensions:
///
/// | Factor | Weight | Description |
/// |--------|--------|-------------|
/// | Arbitrage Potential | 40% | Price spread between exchanges |
/// | Liquidity Score | 30% | Order book depth |
/// | Stability Score | 20% | Spread consistency |
/// | Volume Score | 10% | 24h trading volume |
///
/// # Example
///
/// ```rust
/// use arbitrage_core::symbol_discovery::{
///     SymbolDiscoveryService, SymbolSelectionCriteria, MarketInfo, OrderBookDepth
/// };
/// use arbitrage_core::types::{ExchangeId, Symbol};
/// use rust_decimal::Decimal;
/// use chrono::Utc;
///
/// let criteria = SymbolSelectionCriteria::default();
/// let (mut service, mut receiver) = SymbolDiscoveryService::new(criteria);
///
/// let market_info = MarketInfo {
///     symbol: Symbol::new("BTC", "USDT"),
///     exchange: ExchangeId::Binance,
///     volume_24h_usd: Decimal::from(1000000000),
///     price_usd: Decimal::from(50000),
///     spread_bps: 10,
///     is_active: true,
///     timestamp: Utc::now(),
///     depth_analysis: OrderBookDepth {
///         level_1_volume_usd: Decimal::from(100000),
///         depth_01_percent_usd: Decimal::from(500000),
///         depth_05_percent_usd: Decimal::from(2000000),
///         max_order_size_usd: Decimal::from(50000),
///     },
/// };
///
/// // service.update_market_data(market_info).await;
/// ```
use crate::{
    types::{ExchangeId, Symbol},
    ArbitrageError, Result,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Market data for a trading pair with order book depth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    /// Trading symbol
    pub symbol: Symbol,
    /// Exchange
    pub exchange: ExchangeId,
    /// 24h trading volume in USD
    pub volume_24h_usd: Decimal,
    /// Current price in USD
    pub price_usd: Decimal,
    /// Spread in basis points
    pub spread_bps: u32,
    /// Whether the market is currently active
    pub is_active: bool,
    /// Data timestamp
    pub timestamp: DateTime<Utc>,
    /// Order book depth analysis
    pub depth_analysis: OrderBookDepth,
}

/// Order book depth analysis for liquidity assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookDepth {
    /// Volume at best bid/ask in USD
    pub level_1_volume_usd: Decimal,
    /// Volume within 0.1% of mid price in USD
    pub depth_01_percent_usd: Decimal,
    /// Volume within 0.5% of mid price in USD
    pub depth_05_percent_usd: Decimal,
    /// Largest single order size in USD
    pub max_order_size_usd: Decimal,
}

/// Criteria for selecting symbols to monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSelectionCriteria {
    /// Minimum 24h volume in USD
    pub min_volume_usd: Decimal,
    /// Maximum spread in basis points
    pub max_spread_bps: u32,
    /// Minimum number of exchanges listing the symbol
    pub min_exchanges: usize,
    /// Allowed quote currencies
    pub allowed_quotes: FxHashSet<String>,
    /// Maximum symbols to monitor
    pub max_symbols: usize,
    /// Minimum liquidity at best prices in USD
    pub min_level1_liquidity_usd: Decimal,
    /// Minimum depth within 0.1% in USD
    pub min_depth_01_percent_usd: Decimal,
    /// Minimum volume/volatility ratio
    pub min_stability_ratio: Decimal,
    /// Maximum data age in seconds
    pub max_data_age_seconds: u64,
}

impl Default for SymbolSelectionCriteria {
    fn default() -> Self {
        let mut allowed_quotes = FxHashSet::default();
        allowed_quotes.insert("USDT".to_string());
        allowed_quotes.insert("USDC".to_string());
        allowed_quotes.insert("BUSD".to_string());

        Self {
            min_volume_usd: Decimal::from(1_000_000),
            max_spread_bps: 50,
            min_exchanges: 2,
            allowed_quotes,
            max_symbols: 50,
            min_level1_liquidity_usd: Decimal::from(10_000),
            min_depth_01_percent_usd: Decimal::from(50_000),
            min_stability_ratio: Decimal::from(100_000),
            max_data_age_seconds: 300,
        }
    }
}

/// Events emitted by the discovery service.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// Symbol was added to the qualified list
    SymbolAdded { symbol: Symbol, reason: String },
    /// Symbol was removed from the qualified list
    SymbolRemoved { symbol: Symbol, reason: String },
    /// Selection criteria were updated
    CriteriaUpdated {
        new_criteria: SymbolSelectionCriteria,
    },
    /// Quality alert for a symbol
    QualityAlert { symbol: Symbol, issue: String },
}

/// Enhanced symbol discovery service with real-time updates.
///
/// Manages market data collection, scoring, and qualification for
/// arbitrage opportunities across multiple exchanges.
///
/// # Responsibilities
///
/// - Collect and store market data from exchanges
/// - Calculate comprehensive symbol scores
/// - Track symbol qualification status
/// - Emit events for status changes
/// - Maintain historical data for trend analysis
///
/// # Usage
///
/// ```rust
/// use arbitrage_core::symbol_discovery::{SymbolDiscoveryService, SymbolSelectionCriteria};
///
/// let criteria = SymbolSelectionCriteria::default();
/// let (service, _receiver) = SymbolDiscoveryService::new(criteria);
///
/// // Service is ready to receive market data updates
/// ```
#[derive(Debug, Clone)]
pub struct SymbolDiscoveryService {
    /// Selection criteria
    criteria: SymbolSelectionCriteria,
    /// Market data by (exchange, symbol)
    market_data: FxHashMap<(ExchangeId, Symbol), MarketInfo>,
    /// Qualified symbols meeting all criteria
    qualified_symbols: FxHashSet<Symbol>,
    /// Symbol scores
    symbol_scores: FxHashMap<Symbol, SymbolScore>,
    /// Historical data for trend analysis
    symbol_history: FxHashMap<Symbol, Vec<HistoricalPoint>>,
    /// Event channel
    event_sender: broadcast::Sender<DiscoveryEvent>,
}

/// Scoring system for symbol prioritization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolScore {
    /// The symbol
    pub symbol: Symbol,
    /// Expected arbitrage profit potential (%)
    pub arbitrage_potential: Decimal,
    /// Liquidity score (0-100)
    pub liquidity_score: Decimal,
    /// Stability score (0-100)
    pub stability_score: Decimal,
    /// Volume score (0-100)
    pub volume_score: Decimal,
    /// Weighted total score
    pub total_score: Decimal,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

/// Historical data point for trend analysis.
#[derive(Debug, Clone)]
pub struct HistoricalPoint {
    /// Data point timestamp
    pub timestamp: DateTime<Utc>,
    /// Volume in USD
    pub volume_usd: Decimal,
    /// Spread in basis points
    pub spread_bps: u32,
    /// Arbitrage potential at this point
    pub arbitrage_potential: Decimal,
}

impl SymbolDiscoveryService {
    /// Creates a new SymbolDiscoveryService.
    ///
    /// # Arguments
    ///
    /// * `criteria` - Selection criteria for symbol qualification
    ///
    /// # Returns
    ///
    /// Tuple of (service, event receiver)
    pub fn new(criteria: SymbolSelectionCriteria) -> (Self, broadcast::Receiver<DiscoveryEvent>) {
        let (event_sender, event_receiver) = broadcast::channel(1000);

        let service = Self {
            criteria,
            market_data: FxHashMap::default(),
            qualified_symbols: FxHashSet::default(),
            symbol_scores: FxHashMap::default(),
            symbol_history: FxHashMap::default(),
            event_sender,
        };

        (service, event_receiver)
    }

    /// Updates market data and re-evaluates symbol qualification.
    ///
    /// # Arguments
    ///
    /// * `market_info` - Market data to update
    ///
    /// # Returns
    ///
    /// `Ok(())` on success
    ///
    /// # Errors
    ///
    /// Returns error if data is too old.
    ///
    /// # Side Effects
    ///
    /// May emit `SymbolAdded` or `SymbolRemoved` events if qualification changes.
    pub async fn update_market_data(&mut self, market_info: MarketInfo) -> Result<()> {
        let symbol = market_info.symbol.clone();
        let exchange = market_info.exchange;

        let age_seconds = (Utc::now() - market_info.timestamp).num_seconds() as u64;
        if age_seconds > self.criteria.max_data_age_seconds {
            return Err(ArbitrageError::Validation(format!(
                "Market data too old: {}s > {}s for {}-{}",
                age_seconds,
                self.criteria.max_data_age_seconds,
                exchange,
                symbol.to_pair()
            )));
        }

        let key = (exchange, symbol.clone());
        let was_qualified = self.qualified_symbols.contains(&symbol);
        self.market_data.insert(key, market_info.clone());

        self.update_symbol_history(&symbol, &market_info);

        self.calculate_symbol_score(&symbol).await?;

        let is_qualified = self.is_symbol_qualified(&symbol)?;

        if !was_qualified && is_qualified {
            self.qualified_symbols.insert(symbol.clone());
            let _ = self.event_sender.send(DiscoveryEvent::SymbolAdded {
                symbol: symbol.clone(),
                reason: format!(
                    "Meets criteria: volume=${}, spread={}bps",
                    market_info.volume_24h_usd, market_info.spread_bps
                ),
            });
        } else if was_qualified && !is_qualified {
            self.qualified_symbols.remove(&symbol);
            let _ = self.event_sender.send(DiscoveryEvent::SymbolRemoved {
                symbol: symbol.clone(),
                reason: "No longer meets criteria".to_string(),
            });
        }

        Ok(())
    }

    /// Gets symbols prioritized by arbitrage potential.
    ///
    /// # Returns
    ///
    /// Vector of qualified symbols sorted by total score.
    pub fn get_arbitrage_symbols(&self) -> Result<Vec<Symbol>> {
        let mut scored_symbols: Vec<(Symbol, Decimal)> = self
            .symbol_scores
            .iter()
            .filter(|(symbol, _)| self.qualified_symbols.contains(symbol))
            .map(|(symbol, score)| (symbol.clone(), score.total_score))
            .collect();

        scored_symbols.sort_by(|a, b| b.1.cmp(&a.1));
        scored_symbols.truncate(self.criteria.max_symbols);

        Ok(scored_symbols
            .into_iter()
            .map(|(symbol, _)| symbol)
            .collect())
    }

    /// Calculates comprehensive symbol score.
    ///
    /// Scores are calculated based on:
    /// - Arbitrage potential (price spread)
    /// - Liquidity (order book depth)
    /// - Stability (spread consistency)
    /// - Volume (24h trading volume)
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to score
    ///
    /// # Returns
    ///
    /// `Ok(())` on success
    async fn calculate_symbol_score(&mut self, symbol: &Symbol) -> Result<()> {
        let markets: Vec<&MarketInfo> = self
            .market_data
            .values()
            .filter(|m| &m.symbol == symbol && m.is_active)
            .collect();

        if markets.len() < 2 {
            return Ok(());
        }

        let prices: Vec<Decimal> = markets.iter().map(|m| m.price_usd).collect();
        let min_price = prices.iter().min().copied().unwrap_or(Decimal::ZERO);
        let max_price = prices.iter().max().copied().unwrap_or(Decimal::ZERO);

        let arbitrage_potential = if min_price > Decimal::ZERO {
            ((max_price - min_price) / min_price) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        let avg_level1_liquidity: Decimal = markets
            .iter()
            .map(|m| m.depth_analysis.level_1_volume_usd)
            .sum::<Decimal>()
            / Decimal::from(markets.len());

        let liquidity_score = (avg_level1_liquidity / Decimal::from(100_000) * Decimal::from(100))
            .min(Decimal::from(100));

        let spreads: Vec<u32> = markets.iter().map(|m| m.spread_bps).collect();
        let avg_spread = spreads.iter().sum::<u32>() as f64 / spreads.len() as f64;
        let spread_variance: f64 = spreads
            .iter()
            .map(|&s| (s as f64 - avg_spread).powi(2))
            .sum::<f64>()
            / spreads.len() as f64;
        let stability_score = Decimal::from(100)
            - Decimal::try_from(spread_variance.sqrt()).unwrap_or(Decimal::from(100));

        let total_volume: Decimal = markets.iter().map(|m| m.volume_24h_usd).sum();
        let volume_score =
            (total_volume / Decimal::from(10_000_000) * Decimal::from(100)).min(Decimal::from(100));

        let total_score = arbitrage_potential * Decimal::new(4, 1)
            + liquidity_score * Decimal::new(3, 1)
            + stability_score * Decimal::new(2, 1)
            + volume_score * Decimal::new(1, 1);

        let score = SymbolScore {
            symbol: symbol.clone(),
            arbitrage_potential,
            liquidity_score,
            stability_score,
            volume_score,
            total_score,
            last_updated: Utc::now(),
        };

        self.symbol_scores.insert(symbol.clone(), score);
        Ok(())
    }

    /// Checks if a symbol meets all qualification criteria.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to check
    ///
    /// # Returns
    ///
    /// `Ok(true)` if qualified, `Ok(false)` otherwise.
    fn is_symbol_qualified(&self, symbol: &Symbol) -> Result<bool> {
        let markets: Vec<&MarketInfo> = self
            .market_data
            .values()
            .filter(|m| &m.symbol == symbol && m.is_active)
            .collect();

        if markets.len() < self.criteria.min_exchanges {
            return Ok(false);
        }

        if !self.criteria.allowed_quotes.contains(&symbol.quote) {
            return Ok(false);
        }

        for market in &markets {
            if market.volume_24h_usd < self.criteria.min_volume_usd {
                return Ok(false);
            }

            if market.spread_bps > self.criteria.max_spread_bps {
                return Ok(false);
            }

            if market.depth_analysis.level_1_volume_usd < self.criteria.min_level1_liquidity_usd {
                return Ok(false);
            }

            if market.depth_analysis.depth_01_percent_usd < self.criteria.min_depth_01_percent_usd {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Updates historical tracking for trend analysis.
    fn update_symbol_history(&mut self, symbol: &Symbol, market_info: &MarketInfo) {
        let history = self.symbol_history.entry(symbol.clone()).or_default();

        let current_arbitrage_potential = self
            .symbol_scores
            .get(symbol)
            .map(|s| s.arbitrage_potential)
            .unwrap_or(Decimal::ZERO);

        let point = HistoricalPoint {
            timestamp: market_info.timestamp,
            volume_usd: market_info.volume_24h_usd,
            spread_bps: market_info.spread_bps,
            arbitrage_potential: current_arbitrage_potential,
        };

        history.push(point);

        let cutoff = Utc::now() - chrono::Duration::hours(24);
        history.retain(|p| p.timestamp > cutoff);
    }

    /// Gets enhanced statistics for a symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol to get stats for
    ///
    /// # Returns
    ///
    /// Statistics if data exists, `None` otherwise.
    pub fn get_market_stats(&self, symbol: &Symbol) -> Option<EnhancedSymbolStats> {
        let markets: Vec<&MarketInfo> = self
            .market_data
            .values()
            .filter(|m| &m.symbol == symbol)
            .collect();

        if markets.is_empty() {
            return None;
        }

        let prices: Vec<Decimal> = markets.iter().map(|m| m.price_usd).collect();
        let min_price = prices.iter().min().copied()?;
        let max_price = prices.iter().max().copied()?;

        let total_volume: Decimal = markets.iter().map(|m| m.volume_24h_usd).sum();
        let avg_spread_bps =
            markets.iter().map(|m| m.spread_bps as u64).sum::<u64>() / markets.len() as u64;

        let total_level1_liquidity: Decimal = markets
            .iter()
            .map(|m| m.depth_analysis.level_1_volume_usd)
            .sum();
        let avg_depth_01: Decimal = markets
            .iter()
            .map(|m| m.depth_analysis.depth_01_percent_usd)
            .sum::<Decimal>()
            / Decimal::from(markets.len());

        Some(EnhancedSymbolStats {
            symbol: symbol.clone(),
            exchange_count: markets.len(),
            total_volume_usd: total_volume,
            price_range: PriceRange {
                min: min_price,
                max: max_price,
                spread_percent: if min_price > Decimal::ZERO {
                    ((max_price - min_price) / min_price) * Decimal::from(100)
                } else {
                    Decimal::ZERO
                },
            },
            avg_spread_bps: avg_spread_bps as u32,
            liquidity_metrics: LiquidityMetrics {
                total_level1_usd: total_level1_liquidity,
                avg_depth_01_percent_usd: avg_depth_01,
                liquidity_score: self
                    .symbol_scores
                    .get(symbol)
                    .map(|s| s.liquidity_score)
                    .unwrap_or(Decimal::ZERO),
            },
            score: self.symbol_scores.get(symbol).cloned(),
            trend: self.calculate_trend(symbol),
        })
    }

    /// Calculates trend analysis from historical data.
    fn calculate_trend(&self, symbol: &Symbol) -> Option<TrendAnalysis> {
        let history = self.symbol_history.get(symbol)?;
        if history.len() < 2 {
            return None;
        }

        let recent_points = &history[history.len().saturating_sub(10)..];
        let volume_trend = if recent_points.len() >= 2 {
            let first = &recent_points[0];
            let last = &recent_points[recent_points.len() - 1];
            ((last.volume_usd - first.volume_usd) / first.volume_usd) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        Some(TrendAnalysis {
            volume_change_24h_percent: volume_trend,
            avg_arbitrage_potential: recent_points
                .iter()
                .map(|p| p.arbitrage_potential)
                .sum::<Decimal>()
                / Decimal::from(recent_points.len()),
            trend_direction: if volume_trend > Decimal::new(5, 0) {
                TrendDirection::Rising
            } else if volume_trend < Decimal::new(-5, 0) {
                TrendDirection::Falling
            } else {
                TrendDirection::Stable
            },
        })
    }

    /// Updates selection criteria and re-evaluates all symbols.
    ///
    /// # Arguments
    ///
    /// * `new_criteria` - New selection criteria
    ///
    /// # Returns
    ///
    /// `Ok(())` on success
    ///
    /// # Side Effects
    ///
    /// May emit `CriteriaUpdated` event and multiple `SymbolAdded`/`SymbolRemoved` events.
    pub async fn update_criteria(&mut self, new_criteria: SymbolSelectionCriteria) -> Result<()> {
        self.criteria = new_criteria.clone();

        let symbols: Vec<Symbol> = self
            .market_data
            .values()
            .map(|m| m.symbol.clone())
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect();

        self.qualified_symbols.clear();

        for symbol in symbols {
            self.calculate_symbol_score(&symbol).await?;

            if self.is_symbol_qualified(&symbol)? {
                self.qualified_symbols.insert(symbol.clone());
            }
        }

        let _ = self
            .event_sender
            .send(DiscoveryEvent::CriteriaUpdated { new_criteria });

        Ok(())
    }
}

/// Enhanced statistics for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSymbolStats {
    /// The symbol
    pub symbol: Symbol,
    /// Number of exchanges with data
    pub exchange_count: usize,
    /// Total 24h volume in USD
    pub total_volume_usd: Decimal,
    /// Price range information
    pub price_range: PriceRange,
    /// Average spread in basis points
    pub avg_spread_bps: u32,
    /// Liquidity metrics
    pub liquidity_metrics: LiquidityMetrics,
    /// Current score if available
    pub score: Option<SymbolScore>,
    /// Trend analysis
    pub trend: Option<TrendAnalysis>,
}

/// Price range information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRange {
    /// Minimum price
    pub min: Decimal,
    /// Maximum price
    pub max: Decimal,
    /// Spread as percentage
    pub spread_percent: Decimal,
}

/// Liquidity metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityMetrics {
    /// Total level 1 liquidity in USD
    pub total_level1_usd: Decimal,
    /// Average depth within 0.1% in USD
    pub avg_depth_01_percent_usd: Decimal,
    /// Liquidity score (0-100)
    pub liquidity_score: Decimal,
}

/// Trend analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// 24h volume change percentage
    pub volume_change_24h_percent: Decimal,
    /// Average arbitrage potential
    pub avg_arbitrage_potential: Decimal,
    /// Trend direction
    pub trend_direction: TrendDirection,
}

/// Trend direction enumeration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    /// Volume increasing
    Rising,
    /// Volume decreasing
    Falling,
    /// Volume stable
    Stable,
}
