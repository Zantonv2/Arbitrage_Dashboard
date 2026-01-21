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
            None => true,
            Some(last_update) => {
                let elapsed = Instant::now().duration_since(last_update);
                elapsed >= Duration::from_secs(self.config.discovery_refresh_interval)
            }
        }
    }
}

// ============================================================================
// Shared Symbol Filtering Helpers
// ============================================================================

/// Common quote currencies used in arbitrage strategies
pub static COMMON_QUOTE_CURRENCIES: &[&str] = &["USDT", "USDC", "BUSD", "USD"];

/// Major base assets with active perpetual markets
pub static MAJOR_CRYPTO_BASE_ASSETS: &[&str] = &[
    "BTC", "ETH", "SOL", "BNB", "XRP", "ADA", "AVAX", "DOT", "MATIC", "LINK", "UNI", "LTC", "ATOM",
    "NEAR",
];

/// Ultra-major base assets with highest liquidity
pub static ULTRA_MAJOR_BASE_ASSETS: &[&str] = &["BTC", "ETH", "SOL"];

/// Common stablecoin assets
pub static STABLECOIN_ASSETS: &[&str] = &["USDT", "USDC", "BUSD", "DAI", "TUSD"];

/// Filter symbols by quote currency from a given list
///
/// # Arguments
/// * `all_symbols` - List of all available symbols
/// * `allowed_quotes` - List of allowed quote currencies
///
/// # Returns
/// Symbols with quotes in the allowed list
pub fn filter_by_quote_currency(all_symbols: &[Symbol], allowed_quotes: &[&str]) -> Vec<Symbol> {
    all_symbols
        .iter()
        .filter(|symbol| allowed_quotes.contains(&symbol.quote.as_str()))
        .cloned()
        .collect()
}

/// Filter symbols by base asset from a given list
///
/// # Arguments
/// * `all_symbols` - List of all available symbols
/// * `allowed_bases` - List of allowed base assets
///
/// # Returns
/// Symbols with bases in the allowed list
pub fn filter_by_base_asset(all_symbols: &[Symbol], allowed_bases: &[&str]) -> Vec<Symbol> {
    all_symbols
        .iter()
        .filter(|symbol| allowed_bases.contains(&symbol.base.as_str()))
        .cloned()
        .collect()
}

/// Filter symbols by both base and quote currency
///
/// # Arguments
/// * `all_symbols` - List of all available symbols
/// * `allowed_bases` - List of allowed base assets
/// * `allowed_quotes` - List of allowed quote currencies
///
/// # Returns
/// Symbols matching both base and quote criteria
pub fn filter_by_base_and_quote(
    all_symbols: &[Symbol],
    allowed_bases: &[&str],
    allowed_quotes: &[&str],
) -> Vec<Symbol> {
    all_symbols
        .iter()
        .filter(|symbol| {
            allowed_bases.contains(&symbol.base.as_str())
                && allowed_quotes.contains(&symbol.quote.as_str())
        })
        .cloned()
        .collect()
}

/// Filter symbols for major crypto pairs (high liquidity)
///
/// # Arguments
/// * `all_symbols` - List of all available symbols
///
/// # Returns
/// Symbols with major crypto bases and common quote currencies
pub fn filter_major_crypto_pairs(all_symbols: &[Symbol]) -> Vec<Symbol> {
    filter_by_base_and_quote(
        all_symbols,
        MAJOR_CRYPTO_BASE_ASSETS,
        COMMON_QUOTE_CURRENCIES,
    )
}

/// Filter symbols for ultra-major crypto pairs (highest liquidity)
///
/// # Arguments
/// * `all_symbols` - List of all available symbols
///
/// # Returns
/// Symbols with ultra-major bases (BTC, ETH, SOL) and USDT quote
pub fn filter_ultra_major_crypto_pairs(all_symbols: &[Symbol]) -> Vec<Symbol> {
    all_symbols
        .iter()
        .filter(|symbol| {
            ULTRA_MAJOR_BASE_ASSETS.contains(&symbol.base.as_str()) && symbol.quote == "USDT"
        })
        .cloned()
        .collect()
}

/// Filter stablecoin pairs for stablecoin arbitrage
///
/// # Arguments
/// * `all_symbols` - List of all available symbols
///
/// # Returns
/// Pairs of stablecoins against each other or against USD
pub fn filter_stablecoin_pairs(all_symbols: &[Symbol]) -> Vec<Symbol> {
    all_symbols
        .iter()
        .filter(|symbol| {
            let base_is_stable = STABLECOIN_ASSETS.contains(&symbol.base.as_str());
            let quote_is_stable_or_usd =
                STABLECOIN_ASSETS.contains(&symbol.quote.as_str()) || symbol.quote == "USD";
            (base_is_stable && quote_is_stable_or_usd)
                || (symbol.base == "USDT" || symbol.base == "USDC" || symbol.base == "BUSD")
                    && symbol.quote == "USD"
        })
        .cloned()
        .collect()
}

/// Filter symbols that have both spot and perpetual market availability
///
/// This is typically the major crypto pairs that are listed on both spot and futures exchanges.
///
/// # Arguments
/// * `all_symbols` - List of all available symbols
///
/// # Returns
/// Symbols likely to have both spot and perpetual markets
pub fn filter_spot_perp_eligible(all_symbols: &[Symbol]) -> Vec<Symbol> {
    filter_by_base_and_quote(all_symbols, MAJOR_CRYPTO_BASE_ASSETS, &["USDT", "USDC"])
}

/// Validate symbol meets minimum requirements for arbitrage
///
/// # Arguments
/// * `symbol` - The symbol to validate
/// * `min_volume_usd` - Minimum daily volume in USD
/// * `max_spread_bps` - Maximum acceptable spread in basis points
/// * `required_exchanges` - List of exchanges that must support this symbol
///
/// # Returns
/// true if the symbol meets all requirements
pub fn validate_symbol_requirements(
    symbol: &Symbol,
    _min_volume_usd: rust_decimal::Decimal,
    _max_spread_bps: u32,
    _required_exchanges: &[ExchangeId],
) -> bool {
    let quote_is_common = COMMON_QUOTE_CURRENCIES.contains(&symbol.quote.as_str());
    let base_is_major = MAJOR_CRYPTO_BASE_ASSETS.contains(&symbol.base.as_str())
        || STABLECOIN_ASSETS.contains(&symbol.base.as_str());
    quote_is_common && base_is_major
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
        filter_by_quote_currency(all_symbols, &["USDT", "USDC", "BUSD"])
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(5_000_000),
            max_spread_bps: 30,
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::BingX],
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
        filter_spot_perp_eligible(all_symbols)
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(10_000_000),
            max_spread_bps: 20,
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::OKX],
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
        filter_spot_perp_eligible(all_symbols)
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(20_000_000),
            max_spread_bps: 15,
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::OKX],
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
        filter_by_base_and_quote(
            all_symbols,
            &["BTC", "ETH", "SOL", "BNB", "XRP"],
            &["USDT", "USDC"],
        )
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(50_000_000),
            max_spread_bps: 10,
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::BingX, ExchangeId::OKX],
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: false,
            requires_funding_data: false,
            min_exchanges: 3,
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
        filter_ultra_major_crypto_pairs(all_symbols)
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(100_000_000),
            max_spread_bps: 5,
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
        filter_stablecoin_pairs(all_symbols)
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(1_000_000),
            max_spread_bps: 100,
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
        filter_spot_perp_eligible(all_symbols)
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(15_000_000),
            max_spread_bps: 25,
            required_exchanges: vec![ExchangeId::ByBit, ExchangeId::OKX],
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: true,
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
        filter_by_quote_currency(all_symbols, &["USDT", "USDC"])
    }

    fn get_requirements(&self) -> StrategyRequirements {
        StrategyRequirements {
            min_volume_usd: rust_decimal::Decimal::from(100_000),
            max_spread_bps: 500,
            required_exchanges: vec![ExchangeId::BingX],
            quote_currencies: vec!["USDT".to_string(), "USDC".to_string()],
            requires_perpetuals: false,
            requires_funding_data: false,
            min_exchanges: 1,
        }
    }

    fn get_name(&self) -> &'static str {
        "New Listing Arbitrage"
    }
}
