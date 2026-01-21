use crate::{
    types::{ExchangeId, Symbol},
    ArbitrageError, Result,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Market data for a trading pair with order book depth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub symbol: Symbol,
    pub exchange: ExchangeId,
    pub volume_24h_usd: Decimal,
    pub price_usd: Decimal,
    pub spread_bps: u32, // Spread in basis points
    pub is_active: bool,
    pub timestamp: DateTime<Utc>,
    // NEW: Order book depth analysis
    pub depth_analysis: OrderBookDepth,
}

/// Order book depth analysis for liquidity assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookDepth {
    /// Available volume at best bid/ask
    pub level_1_volume_usd: Decimal,
    /// Volume available within 0.1% of mid price
    pub depth_01_percent_usd: Decimal,
    /// Volume available within 0.5% of mid price  
    pub depth_05_percent_usd: Decimal,
    /// Largest single order size in USD
    pub max_order_size_usd: Decimal,
}

/// Enhanced criteria for selecting symbols with liquidity requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSelectionCriteria {
    /// Minimum 24h volume in USD
    pub min_volume_usd: Decimal,
    /// Maximum spread in basis points (100 bps = 1%)
    pub max_spread_bps: u32,
    /// Minimum number of exchanges that must list the symbol
    pub min_exchanges: usize,
    /// Quote currencies to consider (e.g., USDT, USDC, BTC)
    pub allowed_quotes: FxHashSet<String>,
    /// Maximum number of symbols to monitor (resource limit)
    pub max_symbols: usize,
    // NEW: Liquidity requirements
    /// Minimum liquidity at level 1 (best bid/ask) in USD
    pub min_level1_liquidity_usd: Decimal,
    /// Minimum depth within 0.1% of mid price in USD
    pub min_depth_01_percent_usd: Decimal,
    /// Minimum volatility-adjusted volume (volume / volatility)
    pub min_stability_ratio: Decimal,
    /// Data freshness requirement in seconds
    pub max_data_age_seconds: u64,
}

impl Default for SymbolSelectionCriteria {
    fn default() -> Self {
        let mut allowed_quotes = FxHashSet::default();
        allowed_quotes.insert("USDT".to_string());
        allowed_quotes.insert("USDC".to_string());
        allowed_quotes.insert("BUSD".to_string());

        Self {
            min_volume_usd: Decimal::from(1_000_000), // $1M daily volume
            max_spread_bps: 50,                       // 0.5% max spread
            min_exchanges: 2,                         // Must be on at least 2 exchanges
            allowed_quotes,
            max_symbols: 50,                                 // Monitor top 50 symbols
            min_level1_liquidity_usd: Decimal::from(10_000), // $10K at best prices
            min_depth_01_percent_usd: Decimal::from(50_000), // $50K within 0.1%
            min_stability_ratio: Decimal::from(100_000),     // Volume/volatility ratio
            max_data_age_seconds: 300,                       // 5 minutes max age
        }
    }
}

/// Events emitted by the discovery service
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    SymbolAdded {
        symbol: Symbol,
        reason: String,
    },
    SymbolRemoved {
        symbol: Symbol,
        reason: String,
    },
    CriteriaUpdated {
        new_criteria: SymbolSelectionCriteria,
    },
    QualityAlert {
        symbol: Symbol,
        issue: String,
    },
}

/// Enhanced symbol discovery service with real-time updates and liquidity analysis
pub struct SymbolDiscoveryService {
    criteria: SymbolSelectionCriteria,
    market_data: FxHashMap<(ExchangeId, Symbol), MarketInfo>,
    qualified_symbols: FxHashSet<Symbol>,
    symbol_scores: FxHashMap<Symbol, SymbolScore>,
    symbol_history: FxHashMap<Symbol, Vec<HistoricalPoint>>,
    event_sender: broadcast::Sender<DiscoveryEvent>,
}

/// Scoring system for symbol prioritization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolScore {
    pub symbol: Symbol,
    pub arbitrage_potential: Decimal, // Expected profit potential
    pub liquidity_score: Decimal,     // 0-100 based on depth
    pub stability_score: Decimal,     // 0-100 based on spread consistency
    pub volume_score: Decimal,        // 0-100 based on volume ranking
    pub total_score: Decimal,         // Weighted combination
    pub last_updated: DateTime<Utc>,
}

/// Historical data point for trend analysis
#[derive(Debug, Clone)]
pub struct HistoricalPoint {
    pub timestamp: DateTime<Utc>,
    pub volume_usd: Decimal,
    pub spread_bps: u32,
    pub arbitrage_potential: Decimal,
}

impl SymbolDiscoveryService {
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

    /// Update market data with enhanced liquidity analysis
    pub async fn update_market_data(&mut self, market_info: MarketInfo) -> Result<()> {
        let symbol = market_info.symbol.clone();
        let exchange = market_info.exchange;

        // Check data freshness
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

        // Store market data
        let key = (exchange, symbol.clone());
        let was_qualified = self.qualified_symbols.contains(&symbol);
        self.market_data.insert(key, market_info.clone());

        // Update historical tracking
        self.update_symbol_history(&symbol, &market_info);

        // Recalculate symbol score
        self.calculate_symbol_score(&symbol).await?;

        // Check if qualification status changed
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

    /// Get symbols prioritized by arbitrage potential
    pub fn get_arbitrage_symbols(&self) -> Result<Vec<Symbol>> {
        let mut scored_symbols: Vec<(Symbol, Decimal)> = self
            .symbol_scores
            .iter()
            .filter(|(symbol, _)| self.qualified_symbols.contains(symbol))
            .map(|(symbol, score)| (symbol.clone(), score.total_score))
            .collect();

        // Sort by total score (highest first)
        scored_symbols.sort_by(|a, b| b.1.cmp(&a.1));
        scored_symbols.truncate(self.criteria.max_symbols);

        Ok(scored_symbols
            .into_iter()
            .map(|(symbol, _)| symbol)
            .collect())
    }

    /// Calculate comprehensive symbol score
    async fn calculate_symbol_score(&mut self, symbol: &Symbol) -> Result<()> {
        let markets: Vec<&MarketInfo> = self
            .market_data
            .values()
            .filter(|m| &m.symbol == symbol && m.is_active)
            .collect();

        if markets.len() < 2 {
            return Ok(()); // Need at least 2 exchanges for arbitrage
        }

        // Calculate arbitrage potential (price spread between exchanges)
        let prices: Vec<Decimal> = markets.iter().map(|m| m.price_usd).collect();
        let min_price = prices.iter().min().copied().unwrap_or(Decimal::ZERO);
        let max_price = prices.iter().max().copied().unwrap_or(Decimal::ZERO);

        let arbitrage_potential = if min_price > Decimal::ZERO {
            ((max_price - min_price) / min_price) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // Calculate liquidity score (0-100)
        let avg_level1_liquidity: Decimal = markets
            .iter()
            .map(|m| m.depth_analysis.level_1_volume_usd)
            .sum::<Decimal>()
            / Decimal::from(markets.len());

        let liquidity_score = (avg_level1_liquidity / Decimal::from(100_000) * Decimal::from(100))
            .min(Decimal::from(100));

        // Calculate stability score (inverse of spread variance)
        let spreads: Vec<u32> = markets.iter().map(|m| m.spread_bps).collect();
        let avg_spread = spreads.iter().sum::<u32>() as f64 / spreads.len() as f64;
        let spread_variance: f64 = spreads
            .iter()
            .map(|&s| (s as f64 - avg_spread).powi(2))
            .sum::<f64>()
            / spreads.len() as f64;
        let stability_score = Decimal::from(100)
            - Decimal::try_from(spread_variance.sqrt()).unwrap_or(Decimal::from(100));

        // Calculate volume score (relative to all symbols)
        let total_volume: Decimal = markets.iter().map(|m| m.volume_24h_usd).sum();
        let volume_score =
            (total_volume / Decimal::from(10_000_000) * Decimal::from(100)).min(Decimal::from(100));

        // Weighted total score
        let total_score = arbitrage_potential * Decimal::new(4, 1) +  // 40% weight
            liquidity_score * Decimal::new(3, 1) +      // 30% weight  
            stability_score * Decimal::new(2, 1) +      // 20% weight
            volume_score * Decimal::new(1, 1); // 10% weight

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

    /// Check if symbol meets all qualification criteria
    fn is_symbol_qualified(&self, symbol: &Symbol) -> Result<bool> {
        let markets: Vec<&MarketInfo> = self
            .market_data
            .values()
            .filter(|m| &m.symbol == symbol && m.is_active)
            .collect();

        if markets.len() < self.criteria.min_exchanges {
            return Ok(false);
        }

        // Check quote currency
        if !self.criteria.allowed_quotes.contains(&symbol.quote) {
            return Ok(false);
        }

        // Check all markets meet criteria
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

    /// Update historical tracking for trend analysis
    fn update_symbol_history(&mut self, symbol: &Symbol, market_info: &MarketInfo) {
        let history = self.symbol_history.entry(symbol.clone()).or_default();

        // Calculate current arbitrage potential
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

        // Keep only last 24 hours of data
        let cutoff = Utc::now() - chrono::Duration::hours(24);
        history.retain(|p| p.timestamp > cutoff);
    }

    /// Get symbol statistics with enhanced metrics
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

        // Enhanced liquidity metrics
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

    /// Calculate trend analysis from historical data
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

    /// Update selection criteria and trigger re-evaluation
    pub async fn update_criteria(&mut self, new_criteria: SymbolSelectionCriteria) -> Result<()> {
        self.criteria = new_criteria.clone();

        // Re-evaluate all symbols
        let symbols: Vec<Symbol> = self
            .market_data
            .values()
            .map(|m| m.symbol.clone())
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect();

        // Clear current qualified symbols and re-evaluate
        self.qualified_symbols.clear();

        for symbol in symbols {
            // Recalculate symbol score
            self.calculate_symbol_score(&symbol).await?;

            // Check if symbol meets new criteria
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSymbolStats {
    pub symbol: Symbol,
    pub exchange_count: usize,
    pub total_volume_usd: Decimal,
    pub price_range: PriceRange,
    pub avg_spread_bps: u32,
    pub liquidity_metrics: LiquidityMetrics,
    pub score: Option<SymbolScore>,
    pub trend: Option<TrendAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRange {
    pub min: Decimal,
    pub max: Decimal,
    pub spread_percent: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityMetrics {
    pub total_level1_usd: Decimal,
    pub avg_depth_01_percent_usd: Decimal,
    pub liquidity_score: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub volume_change_24h_percent: Decimal,
    pub avg_arbitrage_potential: Decimal,
    pub trend_direction: TrendDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    Rising,
    Falling,
    Stable,
}
