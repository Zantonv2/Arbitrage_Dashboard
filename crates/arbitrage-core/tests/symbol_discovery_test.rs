use arbitrage_core::{
    symbol_discovery::{
        SymbolDiscoveryService, SymbolSelectionCriteria, MarketInfo, OrderBookDepth,
    },
    types::{ExchangeId, Symbol},
};
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashSet;

#[tokio::test]
async fn test_enhanced_symbol_discovery() {
    let criteria = SymbolSelectionCriteria::default();
    let (mut discovery, _events) = SymbolDiscoveryService::new(criteria);
    
    let btc_usdt = Symbol::new("BTC", "USDT");
    
    let market_info = MarketInfo {
        symbol: btc_usdt.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(50_000_000),
        price_usd: Decimal::from(50000),
        spread_bps: 10,
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(25_000),
            depth_01_percent_usd: Decimal::from(100_000),
            depth_05_percent_usd: Decimal::from(500_000),
            max_order_size_usd: Decimal::from(50_000),
        },
    };
    
    discovery.update_market_data(market_info.clone()).await.unwrap();
    
    // Should not qualify yet (need 2+ exchanges)
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(!symbols.contains(&btc_usdt));
    
    // Add second exchange with slight price difference for arbitrage potential
    let market_info2 = MarketInfo {
        exchange: ExchangeId::BingX,
        price_usd: Decimal::from(50050), // 0.1% price difference
        ..market_info.clone()
    };
    
    discovery.update_market_data(market_info2).await.unwrap();
    
    // Now should qualify
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(symbols.contains(&btc_usdt));
    
    // Check enhanced stats
    let stats = discovery.get_market_stats(&btc_usdt).unwrap();
    assert_eq!(stats.exchange_count, 2);
    assert!(stats.liquidity_metrics.total_level1_usd > Decimal::ZERO);
    assert!(stats.score.is_some());
    
    let score = stats.score.unwrap();
    assert!(score.arbitrage_potential > Decimal::ZERO); // Should detect price difference
    assert!(score.liquidity_score > Decimal::ZERO);
    assert!(score.total_score > Decimal::ZERO);
}

#[tokio::test]
async fn test_symbol_qualification_criteria() {
    let mut criteria = SymbolSelectionCriteria::default();
    criteria.min_volume_usd = Decimal::from(1_000_000); // $1M minimum
    criteria.max_spread_bps = 50; // 0.5% max spread
    criteria.min_level1_liquidity_usd = Decimal::from(10_000); // $10K liquidity
    
    let (mut discovery, _events) = SymbolDiscoveryService::new(criteria);
    
    let test_symbol = Symbol::new("ETH", "USDT");
    
    // Test case 1: Low volume (should not qualify)
    let low_volume_info = MarketInfo {
        symbol: test_symbol.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(500_000), // Below $1M threshold
        price_usd: Decimal::from(3000),
        spread_bps: 20,
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(15_000),
            depth_01_percent_usd: Decimal::from(75_000),
            depth_05_percent_usd: Decimal::from(300_000),
            max_order_size_usd: Decimal::from(30_000),
        },
    };
    
    discovery.update_market_data(low_volume_info.clone()).await.unwrap();
    
    // Add second exchange
    let low_volume_info2 = MarketInfo {
        exchange: ExchangeId::BingX,
        ..low_volume_info.clone()
    };
    
    discovery.update_market_data(low_volume_info2).await.unwrap();
    
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(!symbols.contains(&test_symbol)); // Should not qualify due to low volume
    
    // Test case 2: High spread (should not qualify)
    let high_spread_info = MarketInfo {
        symbol: test_symbol.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(5_000_000), // Good volume
        price_usd: Decimal::from(3000),
        spread_bps: 100, // 1% spread - too high
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(15_000),
            depth_01_percent_usd: Decimal::from(75_000),
            depth_05_percent_usd: Decimal::from(300_000),
            max_order_size_usd: Decimal::from(30_000),
        },
    };
    
    discovery.update_market_data(high_spread_info.clone()).await.unwrap();
    
    let high_spread_info2 = MarketInfo {
        exchange: ExchangeId::BingX,
        ..high_spread_info.clone()
    };
    
    discovery.update_market_data(high_spread_info2).await.unwrap();
    
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(!symbols.contains(&test_symbol)); // Should not qualify due to high spread
    
    // Test case 3: Good criteria (should qualify)
    let good_info = MarketInfo {
        symbol: test_symbol.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(5_000_000), // Good volume
        price_usd: Decimal::from(3000),
        spread_bps: 25, // 0.25% spread - acceptable
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(15_000), // Good liquidity
            depth_01_percent_usd: Decimal::from(75_000),
            depth_05_percent_usd: Decimal::from(300_000),
            max_order_size_usd: Decimal::from(30_000),
        },
    };
    
    discovery.update_market_data(good_info.clone()).await.unwrap();
    
    let good_info2 = MarketInfo {
        exchange: ExchangeId::BingX,
        price_usd: Decimal::from(3005), // Small price difference
        ..good_info.clone()
    };
    
    discovery.update_market_data(good_info2).await.unwrap();
    
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(symbols.contains(&test_symbol)); // Should qualify now
}

#[tokio::test]
async fn test_quote_currency_filtering() {
    let mut criteria = SymbolSelectionCriteria::default();
    let mut allowed_quotes = HashSet::new();
    allowed_quotes.insert("USDT".to_string());
    allowed_quotes.insert("USDC".to_string());
    criteria.allowed_quotes = allowed_quotes;
    
    let (mut discovery, _events) = SymbolDiscoveryService::new(criteria);
    
    // Test USDT pair (should be allowed)
    let usdt_symbol = Symbol::new("BTC", "USDT");
    let usdt_info = create_good_market_info(usdt_symbol.clone(), ExchangeId::ByBit);
    discovery.update_market_data(usdt_info).await.unwrap();
    
    let usdt_info2 = create_good_market_info(usdt_symbol.clone(), ExchangeId::BingX);
    discovery.update_market_data(usdt_info2).await.unwrap();
    
    // Test BTC pair (should be filtered out)
    let btc_symbol = Symbol::new("ETH", "BTC");
    let btc_info = create_good_market_info(btc_symbol.clone(), ExchangeId::ByBit);
    discovery.update_market_data(btc_info).await.unwrap();
    
    let btc_info2 = create_good_market_info(btc_symbol.clone(), ExchangeId::BingX);
    discovery.update_market_data(btc_info2).await.unwrap();
    
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(symbols.contains(&usdt_symbol));
    assert!(!symbols.contains(&btc_symbol)); // Should be filtered out
}

#[tokio::test]
async fn test_data_freshness_validation() {
    let mut criteria = SymbolSelectionCriteria::default();
    criteria.max_data_age_seconds = 300; // 5 minutes max age
    
    let (mut discovery, _events) = SymbolDiscoveryService::new(criteria);
    
    let test_symbol = Symbol::new("SOL", "USDT");
    
    // Test stale data (should be rejected)
    let stale_info = MarketInfo {
        symbol: test_symbol.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(5_000_000),
        price_usd: Decimal::from(100),
        spread_bps: 20,
        is_active: true,
        timestamp: Utc::now() - chrono::Duration::minutes(10), // 10 minutes old
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(15_000),
            depth_01_percent_usd: Decimal::from(75_000),
            depth_05_percent_usd: Decimal::from(300_000),
            max_order_size_usd: Decimal::from(30_000),
        },
    };
    
    let result = discovery.update_market_data(stale_info).await;
    assert!(result.is_err()); // Should reject stale data
    
    // Test fresh data (should be accepted)
    let fresh_info = MarketInfo {
        symbol: test_symbol.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(5_000_000),
        price_usd: Decimal::from(100),
        spread_bps: 20,
        is_active: true,
        timestamp: Utc::now(), // Fresh timestamp
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(15_000),
            depth_01_percent_usd: Decimal::from(75_000),
            depth_05_percent_usd: Decimal::from(300_000),
            max_order_size_usd: Decimal::from(30_000),
        },
    };
    
    let result = discovery.update_market_data(fresh_info).await;
    assert!(result.is_ok()); // Should accept fresh data
}

#[tokio::test]
async fn test_symbol_scoring_system() {
    let criteria = SymbolSelectionCriteria::default();
    let (mut discovery, _events) = SymbolDiscoveryService::new(criteria);
    
    let symbol1 = Symbol::new("BTC", "USDT");
    let symbol2 = Symbol::new("ETH", "USDT");
    
    // BTC with high arbitrage potential
    let btc_info1 = MarketInfo {
        symbol: symbol1.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(100_000_000), // High volume
        price_usd: Decimal::from(50000),
        spread_bps: 5, // Tight spread
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(100_000), // High liquidity
            depth_01_percent_usd: Decimal::from(500_000),
            depth_05_percent_usd: Decimal::from(2_000_000),
            max_order_size_usd: Decimal::from(200_000),
        },
    };
    
    let btc_info2 = MarketInfo {
        exchange: ExchangeId::BingX,
        price_usd: Decimal::from(50100), // 0.2% price difference
        ..btc_info1.clone()
    };
    
    // ETH with lower arbitrage potential
    let eth_info1 = MarketInfo {
        symbol: symbol2.clone(),
        exchange: ExchangeId::ByBit,
        volume_24h_usd: Decimal::from(50_000_000), // Lower volume
        price_usd: Decimal::from(3000),
        spread_bps: 15, // Wider spread
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(50_000), // Lower liquidity
            depth_01_percent_usd: Decimal::from(250_000),
            depth_05_percent_usd: Decimal::from(1_000_000),
            max_order_size_usd: Decimal::from(100_000),
        },
    };
    
    let eth_info2 = MarketInfo {
        exchange: ExchangeId::BingX,
        price_usd: Decimal::from(3010), // 0.33% price difference
        ..eth_info1.clone()
    };
    
    // Update market data
    discovery.update_market_data(btc_info1).await.unwrap();
    discovery.update_market_data(btc_info2).await.unwrap();
    discovery.update_market_data(eth_info1).await.unwrap();
    discovery.update_market_data(eth_info2).await.unwrap();
    
    // Get symbols (should be ordered by score)
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    
    // BTC should rank higher due to better metrics
    let btc_stats = discovery.get_market_stats(&symbol1).unwrap();
    let eth_stats = discovery.get_market_stats(&symbol2).unwrap();
    
    let btc_score = btc_stats.score.unwrap().total_score;
    let eth_score = eth_stats.score.unwrap().total_score;
    
    assert!(btc_score > eth_score); // BTC should have higher score
    
    // BTC should appear first in the list
    assert_eq!(symbols[0], symbol1);
}

// Helper function to create good market info for testing
fn create_good_market_info(symbol: Symbol, exchange: ExchangeId) -> MarketInfo {
    MarketInfo {
        symbol,
        exchange,
        volume_24h_usd: Decimal::from(5_000_000),
        price_usd: Decimal::from(1000),
        spread_bps: 25,
        is_active: true,
        timestamp: Utc::now(),
        depth_analysis: OrderBookDepth {
            level_1_volume_usd: Decimal::from(15_000),
            depth_01_percent_usd: Decimal::from(75_000),
            depth_05_percent_usd: Decimal::from(300_000),
            max_order_size_usd: Decimal::from(30_000),
        },
    }
}

#[tokio::test]
async fn test_criteria_update() {
    let initial_criteria = SymbolSelectionCriteria::default();
    let (mut discovery, _events) = SymbolDiscoveryService::new(initial_criteria);
    
    // Add some market data
    let symbol = Symbol::new("AVAX", "USDT");
    let info1 = create_good_market_info(symbol.clone(), ExchangeId::ByBit);
    let info2 = create_good_market_info(symbol.clone(), ExchangeId::BingX);
    
    discovery.update_market_data(info1).await.unwrap();
    discovery.update_market_data(info2).await.unwrap();
    
    // Should qualify with default criteria
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(symbols.contains(&symbol));
    
    // Update criteria to be more restrictive
    let mut new_criteria = SymbolSelectionCriteria::default();
    new_criteria.min_volume_usd = Decimal::from(10_000_000); // Higher volume requirement
    
    discovery.update_criteria(new_criteria).await.unwrap();
    
    // Should no longer qualify
    let symbols = discovery.get_arbitrage_symbols().unwrap();
    assert!(!symbols.contains(&symbol));
}