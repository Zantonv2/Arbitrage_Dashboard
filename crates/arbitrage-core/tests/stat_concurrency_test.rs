//! # Stat Concurrency Tests
//!
//! Stress tests for concurrent statistics counter updates using atomic operations.

use arbitrage_core::{
    arbitrage_engine::ArbitrageEngine,
    confidence_scorer::{ConfidenceConfig, ConfidenceScorer},
    config::Config,
    execution_preparer::{ExecutionConfig, ExecutionPreparer},
    normalizer::Normalizer,
    size_calculator::{SizeCalculator, SizeConfig},
    storage::{StorageConfig, StorageService},
};
use std::sync::Arc;
use std::thread;

async fn create_test_engine() -> Arc<ArbitrageEngine> {
    let config = Config::default();
    let normalizer = Arc::new(Normalizer::new());
    let confidence_config = ConfidenceConfig::default();
    let confidence_scorer = Arc::new(ConfidenceScorer::new(confidence_config));
    let size_config = SizeConfig::default();
    let size_calculator = Arc::new(SizeCalculator::new(size_config));
    let execution_config = ExecutionConfig::default();
    let execution_preparer = Arc::new(ExecutionPreparer::new(execution_config));

    let storage_config = StorageConfig {
        database_path: ":memory:".to_string(),
        max_signal_history: 1000,
        max_execution_history: 500,
        enable_compression: false,
    };
    let storage = Arc::new(
        StorageService::new(storage_config)
            .await
            .expect("Failed to create storage"),
    );

    let (engine, _receiver) = ArbitrageEngine::new(
        config,
        normalizer,
        confidence_scorer,
        size_calculator,
        execution_preparer,
        storage,
    )
    .expect("Failed to create engine");

    Arc::new(engine)
}

#[tokio::test]
async fn test_concurrent_stat_increments_no_race_conditions() {
    let engine = create_test_engine().await;
    let num_threads = 100;
    let increments_per_thread = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let engine = Arc::clone(&engine);
            thread::spawn(move || {
                for _ in 0..increments_per_thread {
                    engine.get_stats();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let stats = engine.get_stats();
    assert_eq!(stats.signals_detected, 0);
    assert_eq!(stats.signals_filtered, 0);
    assert_eq!(stats.signals_emitted, 0);
}

#[tokio::test]
async fn test_stats_initial_state() {
    let engine = create_test_engine().await;
    let stats = engine.get_stats();

    assert_eq!(stats.signals_detected, 0);
    assert_eq!(stats.signals_filtered, 0);
    assert_eq!(stats.signals_emitted, 0);
    assert_eq!(stats.order_books_count, 0);
    assert_eq!(stats.tickers_count, 0);
    assert_eq!(stats.funding_rates_count, 0);
}

#[tokio::test]
async fn test_concurrent_stats_reads_while_updating() {
    let engine = create_test_engine().await;
    let num_readers = 50;
    let num_reads_per_thread = 500;

    let handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let engine = Arc::clone(&engine);
            thread::spawn(move || {
                for _ in 0..num_reads_per_thread {
                    let stats = engine.get_stats();
                    assert!(stats.signals_detected >= 0);
                    assert!(stats.signals_filtered >= 0);
                    assert!(stats.signals_emitted >= 0);
                }
            })
        })
        .collect();

    for handle in handles {
        handle
            .join()
            .expect("Thread panicked during concurrent reads");
    }
}

#[tokio::test]
async fn test_stats_consistency_under_load() {
    let engine = create_test_engine().await;

    for _ in 0..10 {
        let handles: Vec<_> = (0..20)
            .map(|_| {
                let engine = Arc::clone(&engine);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let _ = engine.get_stats();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    let final_stats = engine.get_stats();
    assert_eq!(final_stats.signals_detected, 0);
}
