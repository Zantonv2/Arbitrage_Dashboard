use crate::{
    symbol_discovery::{SymbolDiscoveryService, SymbolSelectionCriteria, MarketInfo},
    types::{ExchangeId, Symbol},
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
    active_symbols: HashSet<Symbol>,
    last_discovery_update: Option<Instant>,
}

impl SymbolManager {
    pub fn new(config: SymbolManagerConfig) -> Self {
        let (discovery_service, _) = SymbolDiscoveryService::new(config.discovery_criteria.clone());
        let mut active_symbols = HashSet::new();
        
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
        self.discovery_service.update_market_data(market_info).await?;
        
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
        let mut new_active_symbols = HashSet::new();
        for symbol in &self.config.core_symbols {
            new_active_symbols.insert(symbol.clone());
        }
        
        // Add discovered symbols
        for symbol in discovered_symbols {
            new_active_symbols.insert(symbol);
        }
        
        let added_symbols: Vec<_> = new_active_symbols.difference(&self.active_symbols).cloned().collect();
        let removed_symbols: Vec<_> = self.active_symbols.difference(&new_active_symbols).cloned().collect();
        
        self.active_symbols = new_active_symbols;
        self.last_discovery_update = Some(Instant::now());
        
        if !added_symbols.is_empty() {
            tracing::info!("Added {} new symbols for monitoring: {:?}", added_symbols.len(), added_symbols);
        }
        
        if !removed_symbols.is_empty() {
            tracing::info!("Removed {} symbols from monitoring: {:?}", removed_symbols.len(), removed_symbols);
        }
        
        Ok(())
    }

    /// Check if a symbol is currently being monitored
    pub fn is_symbol_active(&self, symbol: &Symbol) -> bool {
        self.active_symbols.contains(symbol)
    }

    /// Get statistics for all active symbols
    pub fn get_symbol_statistics(&self) -> HashMap<Symbol, crate::symbol_discovery::EnhancedSymbolStats> {
        let mut stats = HashMap::new();
        
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
            .filter(|symbol| {
                matches!(symbol.quote.as_str(), "USDT" | "USDC" | "BUSD")
            })
            .cloned()
            .collect()
    }
    
    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(5_000_000), // $5M daily volume
            max_spread_bps: 30, // 0.3% max spread
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
                matches!(symbol.base.as_str(), "BTC" | "ETH" | "SOL" | "BNB" | "XRP" | "ADA" | "AVAX" | "DOT" | "MATIC" | "LINK") &&
                matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }
    
    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(10_000_000), // $10M daily volume
            max_spread_bps: 20, // 0.2% max spread
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
                matches!(symbol.base.as_str(), "BTC" | "ETH" | "SOL" | "BNB" | "XRP" | "ADA" | "AVAX" | "DOT" | "MATIC" | "LINK" | "UNI" | "LTC" | "ATOM" | "NEAR") &&
                matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }
    
    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(20_000_000), // $20M daily volume
            max_spread_bps: 15, // 0.15% max spread
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
                matches!(symbol.base.as_str(), "BTC" | "ETH" | "SOL" | "BNB" | "XRP") &&
                matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }
    
    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(50_000_000), // $50M daily volume
            max_spread_bps: 10, // 0.1% max spread - very tight
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
                matches!(symbol.base.as_str(), "BTC" | "ETH" | "SOL") &&
                symbol.quote == "USDT"
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
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string(), "BUSD".to_string(), "USD".to_string()],
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
                matches!(symbol.base.as_str(), "BTC" | "ETH" | "SOL" | "BNB" | "XRP" | "ADA" | "AVAX" | "DOT" | "MATIC" | "LINK") &&
                matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
            .cloned()
            .collect()
    }
    
    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(15_000_000), // $15M daily volume
            max_spread_bps: 25, // 0.25% max spread
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
            .filter(|symbol| {
                matches!(symbol.quote.as_str(), "USDT" | "USDC")
            })
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