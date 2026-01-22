use crate::{
    symbol_discovery::{MarketInfo, SymbolDiscoveryService, SymbolSelectionCriteria},
    types::{ExchangeId, Symbol},
    Result,
};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, Instant};

/// Configuration for symbol management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolManagerConfig {
    /// Static symbols that are always monitored
    pub core_symbols: Vec<Symbol>,
    /// Enable dynamic discovery
    pub enable_discovery: bool,
    /// How often to refresh discovered symbols (in seconds)
    pub discovery_refresh_interval: u64,
    /// Criteria for dynamic symbol selection
    pub discovery_criteria: SymbolSelectionCriteria,
}

impl Default for SymbolManagerConfig {
    fn default() -> Self {
        Self {
            core_symbols: vec![
                Symbol::new("BTC", "USDT"),
                Symbol::new("ETH", "USDT"),
                Symbol::new("SOL", "USDT"),
                Symbol::new("BNB", "USDT"),
                Symbol::new("XRP", "USDT"),
            ],
            enable_discovery: true,
            discovery_refresh_interval: 3600, // 1 hour
            discovery_criteria: SymbolSelectionCriteria::default(),
        }
    }
}

/// Manages which symbols to monitor for arbitrage
pub struct SymbolManager {
    config: SymbolManagerConfig,
    discovery_service: SymbolDiscoveryService,
    active_symbols: FxHashSet<Symbol>,
    last_discovery_update: Option<Instant>,
}

impl SymbolManager {
    pub fn new(config: SymbolManagerConfig) -> Self {
        let (discovery_service, _) = SymbolDiscoveryService::new(config.discovery_criteria.clone());
        let mut active_symbols = FxHashSet::default();

        // Add core symbols
        for symbol in &config.core_symbols {
            active_symbols.insert(symbol.clone());
        }

        Self {
            config,
            discovery_service,
            active_symbols,
            last_discovery_update: None,
        }
    }

    /// Get all symbols currently being monitored
    pub fn get_active_symbols(&self) -> Vec<Symbol> {
        self.active_symbols.iter().cloned().collect()
    }

    /// Update market data and potentially refresh symbol list
    pub async fn update_market_data(&mut self, market_info: MarketInfo) -> Result<()> {
        self.discovery_service
            .update_market_data(market_info)
            .await?;

        // Check if we need to refresh discovered symbols
        if self.should_refresh_discovery() {
            self.refresh_discovered_symbols().await?;
        }

        Ok(())
    }

    /// Force refresh of discovered symbols
    pub async fn refresh_discovered_symbols(&mut self) -> Result<()> {
        if !self.config.enable_discovery {
            return Ok(());
        }

        let discovered_symbols = self.discovery_service.get_arbitrage_symbols()?;

        // Start with core symbols
        let mut new_active_symbols = FxHashSet::default();
        for symbol in &self.config.core_symbols {
            new_active_symbols.insert(symbol.clone());
        }

        // Add discovered symbols
        for symbol in discovered_symbols {
            new_active_symbols.insert(symbol);
        }

        let added_symbols: Vec<_> = new_active_symbols
            .difference(&self.active_symbols)
            .cloned()
            .collect();
        let removed_symbols: Vec<_> = self
            .active_symbols
            .difference(&new_active_symbols)
            .cloned()
            .collect();

        self.active_symbols = new_active_symbols;
        self.last_discovery_update = Some(Instant::now());

        if !added_symbols.is_empty() {
            tracing::info!(
                "Added {} new symbols for monitoring: {:?}",
                added_symbols.len(),
                added_symbols
            );
        }

        if !removed_symbols.is_empty() {
            tracing::info!(
                "Removed {} symbols from monitoring: {:?}",
                removed_symbols.len(),
                removed_symbols
            );
        }

        Ok(())
    }

    /// Check if a symbol is currently being monitored
    pub fn is_symbol_active(&self, symbol: &Symbol) -> bool {
        self.active_symbols.contains(symbol)
    }

    /// Get statistics for all active symbols
    pub fn get_symbol_statistics(
        &self,
    ) -> FxHashMap<Symbol, crate::symbol_discovery::EnhancedSymbolStats> {
        let mut stats = FxHashMap::default();

        for symbol in &self.active_symbols {
            if let Some(symbol_stats) = self.discovery_service.get_market_stats(symbol) {
                stats.insert(symbol.clone(), symbol_stats);
            }
        }

        stats
    }

    fn should_refresh_discovery(&self) -> bool {
        if !self.config.enable_discovery {
            return false;
        }

        match self.last_discovery_update {
            None => true, // Never updated
            Some(last_update) => {
                let elapsed = Instant::now().duration_since(last_update);
                elapsed >= Duration::from_secs(self.config.discovery_refresh_interval)
            }
        }
    }
}

/// Strategy-specific symbol selection for your 10 strategies
pub trait SymbolStrategy {
    /// Get symbols relevant for this strategy
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol>;

    /// Get minimum requirements for this strategy
    fn get_requirements(&self) -> StrategyRequirements;

    /// Get strategy name for logging
    fn get_name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct StrategyRequirements {
    pub min_volume_usd: rust_decimal::Decimal,
    pub max_spread_bps: u32,
    pub required_exchanges: Vec<ExchangeId>,
    pub quote_currencies: Vec<String>,
    pub requires_perpetuals: bool,
    pub requires_funding_data: bool,
    pub min_exchanges: usize,
}

/// CEX-to-CEX arbitrage strategy symbol selection (Strategy #1 & #5)
pub struct CexArbitrageStrategy;

impl SymbolStrategy for CexArbitrageStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // For CEX arbitrage, we want symbols with USDT/USDC quotes
        all_symbols
            .iter()
            .filter(|symbol| matches!(symbol.quote.as_str(), "USDT" | "USDC" | "BUSD"))
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(5_000_000), // $5M daily volume
            max_spread_bps: 30,                                     // 0.3% max spread
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::BingX], // Need at least these
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: false,
            requires_funding_data: false,
            min_exchanges: 2,
        }
    }

    fn get_name(&self) -> &'static str {
        "CEX Arbitrage"
    }
}

/// Spot-Perpetual arbitrage strategy (Strategy #2)
pub struct SpotPerpetualStrategy;

impl SymbolStrategy for SpotPerpetualStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // Need symbols that have both spot and perpetual markets
        all_symbols
            .iter()
            .filter(|symbol| {
                // Major coins that typically have perpetual markets
                matches!(
                    symbol.base.as_str(),
                    "BTC"
                        | "ETH"
                        | "SOL"
                        | "BNB"
                        | "XRP"
                        | "ADA"
                        | "AVAX"
                        | "DOT"
                        | "MATIC"
                        | "LINK"
                ) && matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(10_000_000), // $10M daily volume
            max_spread_bps: 20,                                      // 0.2% max spread
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::OKX], // Need perp markets
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: true,
            requires_funding_data: false,
            min_exchanges: 2,
        }
    }

    fn get_name(&self) -> &'static str {
        "Spot-Perpetual Arbitrage"
    }
}

/// Funding rate arbitrage strategy (Strategy #3 & #4)
pub struct FundingRateStrategy;

impl SymbolStrategy for FundingRateStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // Focus on major coins with active perpetual markets
        all_symbols
            .iter()
            .filter(|symbol| {
                matches!(
                    symbol.base.as_str(),
                    "BTC"
                        | "ETH"
                        | "SOL"
                        | "BNB"
                        | "XRP"
                        | "ADA"
                        | "AVAX"
                        | "DOT"
                        | "MATIC"
                        | "LINK"
                        | "UNI"
                        | "LTC"
                        | "ATOM"
                        | "NEAR"
                ) && matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(20_000_000), // $20M daily volume
            max_spread_bps: 15,                                      // 0.15% max spread
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::OKX], // Need funding data
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: true,
            requires_funding_data: true,
            min_exchanges: 2,
        }
    }

    fn get_name(&self) -> &'static str {
        "Funding Rate Arbitrage"
    }
}

/// Spread capture strategy (Strategy #6)
pub struct SpreadCaptureStrategy;

impl SymbolStrategy for SpreadCaptureStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // Need highly liquid symbols with tight spreads
        all_symbols
            .iter()
            .filter(|symbol| {
                matches!(symbol.base.as_str(), "BTC" | "ETH" | "SOL" | "BNB" | "XRP")
                    && matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(50_000_000), // $50M daily volume
            max_spread_bps: 10,                                      // 0.1% max spread - very tight
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::BingX, ExchangeId::OKX],
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: false,
            requires_funding_data: false,
            min_exchanges: 3, // Need multiple exchanges for spread capture
        }
    }

    fn get_name(&self) -> &'static str {
        "Spread Capture"
    }
}

/// Latency arbitrage strategy (Strategy #7)
pub struct LatencyArbitrageStrategy;

impl SymbolStrategy for LatencyArbitrageStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // Only the most liquid symbols for latency arbitrage
        all_symbols
            .iter()
            .filter(|symbol| {
                matches!(symbol.base.as_str(), "BTC" | "ETH" | "SOL") && symbol.quote == "USDT"
            })
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(100_000_000), // $100M daily volume
            max_spread_bps: 5, // 0.05% max spread - ultra tight
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::BingX, ExchangeId::OKX],
            quote_currencies: vec!["USDT".to_string()],
            requires_perpetuals: false,
            requires_funding_data: false,
            min_exchanges: 3,
        }
    }

    fn get_name(&self) -> &'static str {
        "Latency Arbitrage"
    }
}

/// Stablecoin peg arbitrage strategy (Strategy #8)
pub struct StablecoinPegStrategy;

impl SymbolStrategy for StablecoinPegStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // Stablecoin pairs only
        all_symbols
            .iter()
            .filter(|symbol| {
                // USDT/USDC, BUSD/USDT, DAI/USDC etc.
                (matches!(symbol.base.as_str(), "USDT" | "USDC" | "BUSD" | "DAI" | "TUSD") &&
                 matches!(symbol.quote.as_str(), "USDT" | "USDC" | "BUSD" | "DAI")) ||
                // Also USD pairs if available
                (matches!(symbol.base.as_str(), "USDT" | "USDC" | "BUSD") && symbol.quote == "USD")
            })
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(1_000_000), // $1M daily volume
            max_spread_bps: 100, // 1% max spread - can be wider for stablecoins
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::BingX],
            quote_currencies: vec![
                "USDT".to_string(),
                "USDC".to_string(),
                "BUSD".to_string(),
                "USD".to_string(),
            ],
            requires_perpetuals: false,
            requires_funding_data: false,
            min_exchanges: 2,
        }
    }

    fn get_name(&self) -> &'static str {
        "Stablecoin Peg Arbitrage"
    }
}

/// Convergence arbitrage strategy (Strategy #9)
pub struct ConvergenceArbitrageStrategy;

impl SymbolStrategy for ConvergenceArbitrageStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // Correlated assets - similar to spot-perpetual but broader
        all_symbols
            .iter()
            .filter(|symbol| {
                matches!(
                    symbol.base.as_str(),
                    "BTC"
                        | "ETH"
                        | "SOL"
                        | "BNB"
                        | "XRP"
                        | "ADA"
                        | "AVAX"
                        | "DOT"
                        | "MATIC"
                        | "LINK"
                ) && matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(15_000_000), // $15M daily volume
            max_spread_bps: 25,                                      // 0.25% max spread
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::OKX],
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: true, // Need both spot and perp for correlation analysis
            requires_funding_data: false,
            min_exchanges: 2,
        }
    }

    fn get_name(&self) -> &'static str {
        "Convergence Arbitrage"
    }
}

/// New listing arbitrage strategy (Strategy #10)
pub struct NewListingArbitrageStrategy;

impl SymbolStrategy for NewListingArbitrageStrategy {
    fn get_strategy_symbols(&self, all_symbols: &[Symbol]) -> Vec<Symbol> {
        // All symbols are potential candidates for new listings
        // This strategy will be more dynamic and event-driven
        all_symbols
            .iter()
            .filter(|symbol| matches!(symbol.quote.as_str(), "USDT" | "USDC"))
            .cloned()
            .collect()
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(100_000), // $100K - new listings start small
            max_spread_bps: 500, // 5% max spread - new listings can be very wide
            required_exchanges: vec![ExchangeId::BingX], // MEXC/Gate.io get new listings first
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: false,
            requires_funding_data: false,
            min_exchanges: 1, // Can work with just one exchange initially
        }
    }

    fn get_name(&self) -> &'static str {
        "New Listing Arbitrage"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol_discovery::{EnhancedSymbolStats, MarketInfo, OrderBookDepth};
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    fn create_test_symbol(base: &str, quote: &str) -> Symbol {
        Symbol::new(base, quote)
    }

    fn create_test_config() -> SymbolManagerConfig {
        SymbolManagerConfig {
            core_symbols: vec![Symbol::new("BTC", "USDT"), Symbol::new("ETH", "USDT")],
            enable_discovery: true,
            discovery_refresh_interval: 3600,
            discovery_criteria: SymbolSelectionCriteria::default(),
        }
    }

    #[test]
    fn test_symbol_manager_default_config() {
        let config = SymbolManagerConfig::default();

        assert_eq!(config.core_symbols.len(), 5);
        assert!(config.enable_discovery);
        assert_eq!(config.discovery_refresh_interval, 3600);
    }

    #[test]
    fn test_symbol_manager_new() {
        let config = create_test_config();
        let manager = SymbolManager::new(config);

        let active_symbols = manager.get_active_symbols();
        assert_eq!(active_symbols.len(), 2);
        assert!(active_symbols.contains(&Symbol::new("BTC", "USDT")));
        assert!(active_symbols.contains(&Symbol::new("ETH", "USDT")));
    }

    #[test]
    fn test_is_symbol_active() {
        let config = create_test_config();
        let manager = SymbolManager::new(config);

        assert!(manager.is_symbol_active(&Symbol::new("BTC", "USDT")));
        assert!(manager.is_symbol_active(&Symbol::new("ETH", "USDT")));
        assert!(!manager.is_symbol_active(&Symbol::new("SOL", "USDT")));
    }

    #[test]
    fn test_get_active_symbols() {
        let config = create_test_config();
        let manager = SymbolManager::new(config);

        let symbols = manager.get_active_symbols();
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn test_get_symbol_statistics() {
        let config = create_test_config();
        let manager = SymbolManager::new(config);

        let stats = manager.get_symbol_statistics();
        assert!(stats.is_empty());
    }

    #[tokio::test]
    async fn test_update_market_data_disabled_discovery() {
        let config = SymbolManagerConfig {
            core_symbols: vec![Symbol::new("BTC", "USDT")],
            enable_discovery: false,
            discovery_refresh_interval: 3600,
            discovery_criteria: SymbolSelectionCriteria::default(),
        };
        let mut manager = SymbolManager::new(config);

        let market_info = MarketInfo {
            symbol: Symbol::new("BTC", "USDT"),
            exchange: ExchangeId::ByBit,
            volume_24h_usd: Decimal::from(1000000),
            price_usd: Decimal::from(50000),
            spread_bps: 10,
            is_active: true,
            timestamp: Utc::now(),
            depth_analysis: OrderBookDepth {
                level_1_volume_usd: Decimal::from(100000),
                depth_01_percent_usd: Decimal::from(500000),
                depth_05_percent_usd: Decimal::from(2000000),
                max_order_size_usd: Decimal::from(100000),
            },
        };
        let result = manager.update_market_data(market_info).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_refresh_discovered_symbols_disabled() {
        let config = SymbolManagerConfig {
            core_symbols: vec![Symbol::new("BTC", "USDT")],
            enable_discovery: false,
            discovery_refresh_interval: 3600,
            discovery_criteria: SymbolSelectionCriteria::default(),
        };
        let mut manager = SymbolManager::new(config);

        let result = manager.refresh_discovered_symbols().await;
        assert!(result.is_ok());

        let symbols = manager.get_active_symbols();
        assert_eq!(symbols.len(), 1);
    }

    #[test]
    fn test_cex_arbitrage_strategy_symbols() {
        let strategy = CexArbitrageStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("SOL", "BUSD"),
            Symbol::new("XRP", "BTC"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("ETH", "USDC")));
        assert!(filtered.contains(&Symbol::new("SOL", "BUSD")));
    }

    #[test]
    fn test_cex_arbitrage_strategy_requirements() {
        let strategy = CexArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(5_000_000));
        assert_eq!(requirements.max_spread_bps, 30);
        assert!(!requirements.requires_perpetuals);
        assert!(!requirements.requires_funding_data);
        assert_eq!(requirements.min_exchanges, 2);
    }

    #[test]
    fn test_spot_perpetual_strategy_symbols() {
        let strategy = SpotPerpetualStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("DOGE", "USDT"),
            Symbol::new("ADA", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert!(filtered.len() >= 1);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
    }

    #[test]
    fn test_spot_perpetual_strategy_requirements() {
        let strategy = SpotPerpetualStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(10_000_000));
        assert!(requirements.requires_perpetuals);
        assert_eq!(requirements.min_exchanges, 2);
    }

    #[test]
    fn test_funding_rate_strategy_symbols() {
        let strategy = FundingRateStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("LTC", "USDT"),
            Symbol::new("NEAR", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("LTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("NEAR", "USDT")));
    }

    #[test]
    fn test_funding_rate_strategy_requirements() {
        let strategy = FundingRateStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(20_000_000));
        assert!(requirements.requires_perpetuals);
        assert!(requirements.requires_funding_data);
    }

    #[test]
    fn test_spread_capture_strategy_symbols() {
        let strategy = SpreadCaptureStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("DOGE", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("ETH", "USDC")));
        assert!(!filtered.contains(&Symbol::new("DOGE", "USDT")));
    }

    #[test]
    fn test_spread_capture_strategy_requirements() {
        let strategy = SpreadCaptureStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.max_spread_bps, 10);
        assert_eq!(requirements.min_exchanges, 3);
    }

    #[test]
    fn test_latency_arbitrage_strategy_symbols() {
        let strategy = LatencyArbitrageStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
            Symbol::new("SOL", "USDC"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("ETH", "USDT")));
    }

    #[test]
    fn test_latency_arbitrage_strategy_requirements() {
        let strategy = LatencyArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(100_000_000));
        assert_eq!(requirements.max_spread_bps, 5);
    }

    #[test]
    fn test_stablecoin_peg_strategy_symbols() {
        let strategy = StablecoinPegStrategy;
        let all_symbols = vec![
            Symbol::new("USDT", "USDC"),
            Symbol::new("BUSD", "USDT"),
            Symbol::new("BTC", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&Symbol::new("USDT", "USDC")));
        assert!(filtered.contains(&Symbol::new("BUSD", "USDT")));
    }

    #[test]
    fn test_stablecoin_peg_strategy_requirements() {
        let strategy = StablecoinPegStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(1_000_000));
        assert_eq!(requirements.max_spread_bps, 100);
    }

    #[test]
    fn test_convergence_arbitrage_strategy_symbols() {
        let strategy = ConvergenceArbitrageStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("DOT", "USDT"),
            Symbol::new("XRP", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("DOT", "USDT")));
    }

    #[test]
    fn test_convergence_arbitrage_strategy_requirements() {
        let strategy = ConvergenceArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert!(requirements.requires_perpetuals);
    }

    #[test]
    fn test_new_listing_strategy_symbols() {
        let strategy = NewListingArbitrageStrategy;
        let all_symbols = vec![
            Symbol::new("PEPE", "USDT"),
            Symbol::new("BONK", "USDC"),
            Symbol::new("FLOKI", "BUSD"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&Symbol::new("PEPE", "USDT")));
        assert!(filtered.contains(&Symbol::new("BONK", "USDC")));
        assert!(!filtered.contains(&Symbol::new("FLOKI", "BUSD")));
    }

    #[test]
    fn test_new_listing_strategy_requirements() {
        let strategy = NewListingArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(100_000));
        assert_eq!(requirements.max_spread_bps, 500);
        assert_eq!(requirements.min_exchanges, 1);
    }

    #[test]
    fn test_strategy_requirements_structure() {
        let requirements = StrategyRequirements {
            min_volume_usd: Decimal::from(10_000_000),
            max_spread_bps: 50,
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::OKX],
            quote_currencies: vec!["USDT".to_string()],
            requires_perpetuals: true,
            requires_funding_data: false,
            min_exchanges: 2,
        };

        assert_eq!(requirements.required_exchanges.len(), 2);
        assert!(requirements.requires_perpetuals);
    }

    #[test]
    fn test_symbol_strategy_trait_objects() {
        let strategies: Vec<Box<dyn SymbolStrategy>> = vec![
            Box::new(CexArbitrageStrategy),
            Box::new(SpotPerpetualStrategy),
        ];

        assert_eq!(strategies.len(), 2);

        let all_symbols = vec![Symbol::new("BTC", "USDT")];
        let symbols1 = strategies[0].get_strategy_symbols(&all_symbols);
        let symbols2 = strategies[1].get_strategy_symbols(&all_symbols);

        assert!(!symbols1.is_empty());
        assert!(!symbols2.is_empty());
    }

    #[test]
    fn test_cross_exchange_strategy_symbols() {
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("SOL", "USDT"),
            Symbol::new("DOGE", "BTC"),
        ];

        let strategy = CexArbitrageStrategy;
        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("ETH", "USDC")));
        assert!(filtered.contains(&Symbol::new("SOL", "USDT")));
        assert!(!filtered.contains(&Symbol::new("DOGE", "BTC")));
    }

    #[test]
    fn test_cross_exchange_strategy_requirements_details() {
        let strategy = CexArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(5_000_000));
        assert_eq!(requirements.max_spread_bps, 30);
        assert!(!requirements.requires_perpetuals);
        assert!(!requirements.requires_funding_data);
        assert_eq!(requirements.min_exchanges, 2);
        assert!(requirements.required_exchanges.contains(&ExchangeId::ByBit));
        assert!(requirements.required_exchanges.contains(&ExchangeId::BingX));
    }

    #[test]
    fn test_spot_perpetual_strategy_comprehensive_symbols() {
        let strategy = SpotPerpetualStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("SOL", "USDT"),
            Symbol::new("XRP", "USDT"),
            Symbol::new("ADA", "USDT"),
            Symbol::new("DOGE", "USDT"),
            Symbol::new("LINK", "USDT"),
            Symbol::new("AVAX", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 7);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("ETH", "USDC")));
        assert!(filtered.contains(&Symbol::new("SOL", "USDT")));
        assert!(!filtered.contains(&Symbol::new("DOGE", "USDT")));
    }

    #[test]
    fn test_spot_perpetual_strategy_requirements_comprehensive() {
        let strategy = SpotPerpetualStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(10_000_000));
        assert_eq!(requirements.max_spread_bps, 20);
        assert!(requirements.requires_perpetuals);
        assert!(!requirements.requires_funding_data);
        assert_eq!(requirements.min_exchanges, 2);
        assert!(requirements.quote_currencies.contains(&"USDT".to_string()));
        assert!(requirements.quote_currencies.contains(&"USDC".to_string()));
    }

    #[test]
    fn test_funding_rate_strategy_comprehensive_symbols() {
        let strategy = FundingRateStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("SOL", "USDT"),
            Symbol::new("XRP", "USDT"),
            Symbol::new("UNI", "USDT"),
            Symbol::new("LTC", "USDT"),
            Symbol::new("NEAR", "USDT"),
            Symbol::new("ATOM", "USDT"),
            Symbol::new("DOT", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 9);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("LTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("NEAR", "USDT")));
    }

    #[test]
    fn test_funding_rate_strategy_requirements_comprehensive() {
        let strategy = FundingRateStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(20_000_000));
        assert_eq!(requirements.max_spread_bps, 15);
        assert!(requirements.requires_perpetuals);
        assert!(requirements.requires_funding_data);
        assert_eq!(requirements.min_exchanges, 2);
    }

    #[test]
    fn test_spread_capture_strategy_symbols_edge_cases() {
        let strategy = SpreadCaptureStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("SOL", "BUSD"),
            Symbol::new("XRP", "USDT"),
            Symbol::new("BNB", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 4);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("ETH", "USDC")));
        assert!(filtered.contains(&Symbol::new("XRP", "USDT")));
        assert!(filtered.contains(&Symbol::new("BNB", "USDT")));
        assert!(!filtered.contains(&Symbol::new("SOL", "BUSD")));
    }

    #[test]
    fn test_spread_capture_strategy_requirements_comprehensive() {
        let strategy = SpreadCaptureStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(50_000_000));
        assert_eq!(requirements.max_spread_bps, 10);
        assert_eq!(requirements.min_exchanges, 3);
        assert!(!requirements.requires_perpetuals);
    }

    #[test]
    fn test_latency_arbitrage_strategy_strict_filtering() {
        let strategy = LatencyArbitrageStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
            Symbol::new("SOL", "USDC"),
            Symbol::new("SOL", "USDT"),
            Symbol::new("BNB", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("ETH", "USDT")));
        assert!(filtered.contains(&Symbol::new("SOL", "USDT")));
        assert!(!filtered.contains(&Symbol::new("SOL", "USDC")));
    }

    #[test]
    fn test_latency_arbitrage_strategy_requirements_comprehensive() {
        let strategy = LatencyArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(100_000_000));
        assert_eq!(requirements.max_spread_bps, 5);
        assert_eq!(requirements.min_exchanges, 3);
        assert_eq!(requirements.quote_currencies.len(), 1);
        assert_eq!(requirements.quote_currencies[0], "USDT");
    }

    #[test]
    fn test_stablecoin_peg_strategy_comprehensive_pairs() {
        let strategy = StablecoinPegStrategy;
        let all_symbols = vec![
            Symbol::new("USDT", "USDC"),
            Symbol::new("USDC", "USDT"),
            Symbol::new("BUSD", "USDT"),
            Symbol::new("DAI", "USDC"),
            Symbol::new("TUSD", "USD"),
            Symbol::new("USDT", "USD"),
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 5);
        assert!(filtered.contains(&Symbol::new("USDT", "USDC")));
        assert!(filtered.contains(&Symbol::new("BUSD", "USDT")));
        assert!(filtered.contains(&Symbol::new("USDT", "USD")));
        assert!(!filtered.contains(&Symbol::new("BTC", "USDT")));
    }

    #[test]
    fn test_stablecoin_peg_strategy_requirements_comprehensive() {
        let strategy = StablecoinPegStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(1_000_000));
        assert_eq!(requirements.max_spread_bps, 100);
        assert_eq!(requirements.min_exchanges, 2);
        assert!(!requirements.requires_perpetuals);
    }

    #[test]
    fn test_convergence_arbitrage_strategy_symbol_filtering() {
        let strategy = ConvergenceArbitrageStrategy;
        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDC"),
            Symbol::new("SOL", "USDT"),
            Symbol::new("DOT", "USDT"),
            Symbol::new("XRP", "USDT"),
            Symbol::new("ADA", "USDT"),
            Symbol::new("DOGE", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 6);
        assert!(filtered.contains(&Symbol::new("BTC", "USDT")));
        assert!(filtered.contains(&Symbol::new("DOT", "USDT")));
        assert!(!filtered.contains(&Symbol::new("DOGE", "USDT")));
    }

    #[test]
    fn test_convergence_arbitrage_strategy_requirements_comprehensive() {
        let strategy = ConvergenceArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(15_000_000));
        assert_eq!(requirements.max_spread_bps, 25);
        assert!(requirements.requires_perpetuals);
        assert!(!requirements.requires_funding_data);
    }

    #[test]
    fn test_new_listing_strategy_comprehensive() {
        let strategy = NewListingArbitrageStrategy;
        let all_symbols = vec![
            Symbol::new("PEPE", "USDT"),
            Symbol::new("BONK", "USDC"),
            Symbol::new("FLOKI", "BUSD"),
            Symbol::new("WIF", "USDT"),
            Symbol::new("NEWCOIN", "USDT"),
        ];

        let filtered = strategy.get_strategy_symbols(&all_symbols);

        assert_eq!(filtered.len(), 4);
        assert!(filtered.contains(&Symbol::new("PEPE", "USDT")));
        assert!(filtered.contains(&Symbol::new("BONK", "USDC")));
        assert!(filtered.contains(&Symbol::new("WIF", "USDT")));
        assert!(filtered.contains(&Symbol::new("NEWCOIN", "USDT")));
        assert!(!filtered.contains(&Symbol::new("FLOKI", "BUSD")));
    }

    #[test]
    fn test_new_listing_strategy_requirements_comprehensive() {
        let strategy = NewListingArbitrageStrategy;
        let requirements = strategy.get_requirements();

        assert_eq!(requirements.min_volume_usd, Decimal::from(100_000));
        assert_eq!(requirements.max_spread_bps, 500);
        assert_eq!(requirements.min_exchanges, 1);
        assert!(!requirements.requires_perpetuals);
    }

    #[test]
    fn test_symbol_manager_with_custom_config() {
        let config = SymbolManagerConfig {
            core_symbols: vec![
                Symbol::new("BTC", "USDT"),
                Symbol::new("ETH", "USDT"),
                Symbol::new("SOL", "USDT"),
            ],
            enable_discovery: false,
            discovery_refresh_interval: 7200,
            discovery_criteria: SymbolSelectionCriteria::default(),
        };
        let manager = SymbolManager::new(config);

        let active_symbols = manager.get_active_symbols();
        assert_eq!(active_symbols.len(), 3);
        assert!(manager.is_symbol_active(&Symbol::new("BTC", "USDT")));
    }

    #[test]
    fn test_symbol_manager_discovery_disabled() {
        let config = SymbolManagerConfig {
            core_symbols: vec![Symbol::new("BTC", "USDT")],
            enable_discovery: false,
            discovery_refresh_interval: 3600,
            discovery_criteria: SymbolSelectionCriteria::default(),
        };
        let manager = SymbolManager::new(config);

        assert!(manager.is_symbol_active(&Symbol::new("BTC", "USDT")));
        assert!(!manager.is_symbol_active(&Symbol::new("ETH", "USDT")));
    }

    #[test]
    fn test_all_strategies_trait_object_collection() {
        let strategies: Vec<Box<dyn SymbolStrategy>> = vec![
            Box::new(CexArbitrageStrategy),
            Box::new(SpotPerpetualStrategy),
            Box::new(FundingRateStrategy),
            Box::new(SpreadCaptureStrategy),
            Box::new(LatencyArbitrageStrategy),
            Box::new(StablecoinPegStrategy),
            Box::new(ConvergenceArbitrageStrategy),
            Box::new(NewListingArbitrageStrategy),
        ];

        assert_eq!(strategies.len(), 8);

        let all_symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
            Symbol::new("USDT", "USDC"),
        ];

        for strategy in &strategies {
            let filtered = strategy.get_strategy_symbols(&all_symbols);
            let requirements = strategy.get_requirements();
            let name = strategy.get_name();

            assert!(!name.is_empty());
            assert!(requirements.min_volume_usd > Decimal::ZERO);
            assert!(requirements.max_spread_bps > 0);
        }
    }

    #[test]
    fn test_strategy_requirements_equality() {
        let req1 = StrategyRequirements {
            min_volume_usd: Decimal::from(10_000_000),
            max_spread_bps: 50,
            required_exchanges: vec![ExchangeId::ByBit],
            quote_currencies: vec!["USDT".to_string()],
            requires_perpetuals: true,
            requires_funding_data: false,
            min_exchanges: 1,
        };

        let req2 = StrategyRequirements {
            min_volume_usd: Decimal::from(10_000_000),
            max_spread_bps: 50,
            required_exchanges: vec![ExchangeId::ByBit],
            quote_currencies: vec!["USDT".to_string()],
            requires_perpetuals: true,
            requires_funding_data: false,
            min_exchanges: 1,
        };

        assert_eq!(req1.min_volume_usd, req2.min_volume_usd);
        assert_eq!(req1.max_spread_bps, req2.max_spread_bps);
        assert_eq!(req1.requires_perpetuals, req2.requires_perpetuals);
    }
}
