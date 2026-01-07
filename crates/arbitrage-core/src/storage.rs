use crate::{
    types::{ExecutionInstruction, Signal},
    ArbitrageError, Result,
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub database_path: String,
    pub max_signal_history: usize,
    pub max_execution_history: usize,
    pub enable_compression: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_path: "arbitrage.db".to_string(),
            max_signal_history: 10000,
            max_execution_history: 5000,
            enable_compression: false,
        }
    }
}

/// Stored signal record
#[derive(Debug, Clone)]
pub struct StoredSignal {
    pub signal: Signal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub confidence_score: Decimal,
    pub status: SignalStatus,
}

/// Stored execution record
#[derive(Debug, Clone)]
pub struct StoredExecution {
    pub instruction: ExecutionInstruction,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: ExecutionStatus,
    pub actual_profit: Option<Decimal>,
    pub execution_time_ms: Option<u64>,
}

/// Signal processing status
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SignalStatus {
    Detected,
    Filtered,
    Executed,
    Expired,
    Failed,
}

/// Execution status
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExecutionStatus {
    Pending,
    PartiallyFilled,
    Completed,
    Failed,
    Cancelled,
}

/// Query parameters for retrieving signals
#[derive(Debug, Clone)]
pub struct SignalQuery {
    pub limit: Option<usize>,
    pub status_filter: Option<SignalStatus>,
    pub symbol_filter: Option<String>,
    pub exchange_filter: Option<crate::types::ExchangeId>,
    pub min_confidence: Option<Decimal>,
    pub time_range: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
}

impl Default for SignalQuery {
    fn default() -> Self {
        Self {
            limit: Some(100),
            status_filter: None,
            symbol_filter: None,
            exchange_filter: None,
            min_confidence: None,
            time_range: None,
        }
    }
}

/// Storage service for persisting signals and executions
pub struct StorageService {
    config: StorageConfig,
    // In-memory storage for Phase 1 (SQLite implementation would go here in production)
    signals: Arc<Mutex<HashMap<Uuid, StoredSignal>>>,
    executions: Arc<Mutex<HashMap<Uuid, StoredExecution>>>,
    signal_counter: Arc<Mutex<u64>>,
}

impl StorageService {
    pub fn new(config: StorageConfig) -> Result<Self> {
        // In Phase 1, we'll use in-memory storage
        // In production, this would initialize SQLite connection
        Ok(Self {
            config,
            signals: Arc::new(Mutex::new(HashMap::new())),
            executions: Arc::new(Mutex::new(HashMap::new())),
            signal_counter: Arc::new(Mutex::new(0)),
        })
    }

    /// Store a detected signal with deduplication check
    pub fn store_signal(
        &self,
        signal: Signal,
        confidence_score: Decimal,
        status: SignalStatus,
    ) -> Result<()> {
        let stored_signal = StoredSignal {
            signal: signal.clone(),
            timestamp: chrono::Utc::now(),
            confidence_score,
            status: status.clone(),
        };

        let mut signals = self.signals.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire signals lock".to_string()))?;
        
        // Check for existing signal and log if overwriting
        if let Some(existing) = signals.get(&signal.id) {
            tracing::warn!(
                signal_id = %signal.id,
                old_status = ?existing.status,
                new_status = ?status,
                "Overwriting existing signal"
            );
        }
        
        signals.insert(signal.id, stored_signal);

        // Cleanup old signals if we exceed max history
        if signals.len() > self.config.max_signal_history {
            self.cleanup_old_signals(&mut signals)?;
        }

        tracing::debug!(
            signal_id = %signal.id,
            confidence = %confidence_score,
            status = ?status,
            total_signals = signals.len(),
            "Signal stored successfully"
        );

        Ok(())
    }

    /// Store an execution instruction
    pub fn store_execution(
        &self,
        instruction: ExecutionInstruction,
        status: ExecutionStatus,
    ) -> Result<()> {
        let stored_execution = StoredExecution {
            instruction: instruction.clone(),
            timestamp: chrono::Utc::now(),
            status,
            actual_profit: None,
            execution_time_ms: None,
        };

        let mut executions = self.executions.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire executions lock".to_string()))?;
        
        executions.insert(instruction.signal_id, stored_execution);

        // Cleanup old executions if we exceed max history
        if executions.len() > self.config.max_execution_history {
            self.cleanup_old_executions(&mut executions)?;
        }

        Ok(())
    }

    /// Update signal status
    pub fn update_signal_status(&self, signal_id: Uuid, status: SignalStatus) -> Result<()> {
        let mut signals = self.signals.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire signals lock".to_string()))?;
        
        if let Some(stored_signal) = signals.get_mut(&signal_id) {
            stored_signal.status = status;
        }

        Ok(())
    }

    /// Update execution status and results
    pub fn update_execution_status(
        &self,
        signal_id: Uuid,
        status: ExecutionStatus,
        actual_profit: Option<Decimal>,
        execution_time_ms: Option<u64>,
    ) -> Result<()> {
        let mut executions = self.executions.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire executions lock".to_string()))?;
        
        if let Some(stored_execution) = executions.get_mut(&signal_id) {
            stored_execution.status = status;
            stored_execution.actual_profit = actual_profit;
            stored_execution.execution_time_ms = execution_time_ms;
        }

        Ok(())
    }

    /// Query signals with filters
    pub fn query_signals(&self, query: &SignalQuery) -> Result<Vec<StoredSignal>> {
        let signals = self.signals.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire signals lock".to_string()))?;
        
        let mut results: Vec<StoredSignal> = signals.values()
            .filter(|stored_signal| {
                // Apply filters
                if let Some(status) = &query.status_filter {
                    if stored_signal.status != *status {
                        return false;
                    }
                }

                if let Some(symbol_filter) = &query.symbol_filter {
                    // Normalize symbol comparison (case-insensitive)
                    let filter_upper = symbol_filter.to_uppercase();
                    let signal_base = stored_signal.signal.symbol.base.to_uppercase();
                    let signal_quote = stored_signal.signal.symbol.quote.to_uppercase();
                    
                    if signal_base != filter_upper && signal_quote != filter_upper {
                        return false;
                    }
                }

                if let Some(exchange) = &query.exchange_filter {
                    if stored_signal.signal.buy_exchange != *exchange && stored_signal.signal.sell_exchange != *exchange {
                        return false;
                    }
                }

                if let Some(min_confidence) = query.min_confidence {
                    if stored_signal.confidence_score < min_confidence {
                        return false;
                    }
                }

                if let Some((start, end)) = query.time_range {
                    if stored_signal.timestamp < start || stored_signal.timestamp > end {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort by timestamp (newest first)
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// Get execution by signal ID
    pub fn get_execution(&self, signal_id: Uuid) -> Result<Option<StoredExecution>> {
        let executions = self.executions.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire executions lock".to_string()))?;
        
        Ok(executions.get(&signal_id).cloned())
    }

    /// Get storage statistics
    pub fn get_statistics(&self) -> Result<StorageStatistics> {
        let signals = self.signals.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire signals lock".to_string()))?;
        let executions = self.executions.lock()
            .map_err(|_| ArbitrageError::Storage("Failed to acquire executions lock".to_string()))?;

        let mut status_counts = HashMap::new();
        for stored_signal in signals.values() {
            *status_counts.entry(stored_signal.status.clone()).or_insert(0) += 1;
        }

        let mut execution_status_counts = HashMap::new();
        for stored_execution in executions.values() {
            *execution_status_counts.entry(stored_execution.status.clone()).or_insert(0) += 1;
        }

        // Rough memory usage estimate (for monitoring)
        let estimated_size_per_signal = 1024; // ~1KB per signal (rough estimate)
        let estimated_size_per_execution = 2048; // ~2KB per execution (rough estimate)
        let memory_usage_estimate_mb = ((signals.len() * estimated_size_per_signal) + 
                                       (executions.len() * estimated_size_per_execution)) as f64 / 1_048_576.0;

        Ok(StorageStatistics {
            total_signals: signals.len(),
            total_executions: executions.len(),
            signal_status_counts: status_counts,
            execution_status_counts,
            memory_usage_estimate_mb,
            avg_query_time_ms: None, // Could be implemented with query timing
        })
    }

    /// Cleanup old signals (keep most recent ones) - optimized for large datasets
    fn cleanup_old_signals(&self, signals: &mut HashMap<Uuid, StoredSignal>) -> Result<()> {
        if signals.len() <= self.config.max_signal_history {
            return Ok(());
        }

        // For large datasets, use BTreeMap for efficient sorting by timestamp
        let mut timestamp_to_id: std::collections::BTreeMap<chrono::DateTime<chrono::Utc>, Uuid> = 
            std::collections::BTreeMap::new();
        
        // Build timestamp index
        for (id, stored_signal) in signals.iter() {
            timestamp_to_id.insert(stored_signal.timestamp, *id);
        }
        
        // Remove oldest signals (keep only max_signal_history most recent)
        let to_remove = signals.len() - self.config.max_signal_history;
        let mut removed_count = 0;
        
        for (_, signal_id) in timestamp_to_id.iter() {
            if removed_count >= to_remove {
                break;
            }
            signals.remove(signal_id);
            removed_count += 1;
        }
        
        tracing::info!(
            removed_signals = removed_count,
            remaining_signals = signals.len(),
            "Cleaned up old signals"
        );

        Ok(())
    }

    /// Cleanup old executions (keep most recent ones) - optimized for large datasets
    fn cleanup_old_executions(&self, executions: &mut HashMap<Uuid, StoredExecution>) -> Result<()> {
        if executions.len() <= self.config.max_execution_history {
            return Ok(());
        }

        // For large datasets, use BTreeMap for efficient sorting by timestamp
        let mut timestamp_to_id: std::collections::BTreeMap<chrono::DateTime<chrono::Utc>, Uuid> = 
            std::collections::BTreeMap::new();
        
        // Build timestamp index
        for (id, stored_execution) in executions.iter() {
            timestamp_to_id.insert(stored_execution.timestamp, *id);
        }
        
        // Remove oldest executions (keep only max_execution_history most recent)
        let to_remove = executions.len() - self.config.max_execution_history;
        let mut removed_count = 0;
        
        for (_, execution_id) in timestamp_to_id.iter() {
            if removed_count >= to_remove {
                break;
            }
            executions.remove(execution_id);
            removed_count += 1;
        }
        
        tracing::info!(
            removed_executions = removed_count,
            remaining_executions = executions.len(),
            "Cleaned up old executions"
        );

        Ok(())
    }

    /// Initialize database (placeholder for SQLite setup)
    pub fn initialize_database(&self) -> Result<()> {
        // In Phase 1, this is a no-op
        // In production, this would create SQLite tables
        Ok(())
    }

    /// Backup data to file
    pub fn backup_to_file(&self, _path: &Path) -> Result<()> {
        // Placeholder for backup functionality
        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &StorageConfig {
        &self.config
    }
}

/// Storage statistics with performance metrics
#[derive(Debug, Clone)]
pub struct StorageStatistics {
    pub total_signals: usize,
    pub total_executions: usize,
    pub signal_status_counts: HashMap<SignalStatus, usize>,
    pub execution_status_counts: HashMap<ExecutionStatus, usize>,
    pub memory_usage_estimate_mb: f64, // Rough estimate for monitoring
    pub avg_query_time_ms: Option<f64>, // Could be tracked for performance monitoring
}

impl Default for StorageService {
    fn default() -> Self {
        Self::new(StorageConfig::default()).expect("Failed to create default storage service")
    }
}