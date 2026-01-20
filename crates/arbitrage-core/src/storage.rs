use crate::{
    types::{ExecutionInstruction, Signal},
    ArbitrageError, Result,
};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::{sqlite::SqlitePool, Row};
use std::str::FromStr;
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

impl SignalStatus {
    fn to_string(&self) -> &'static str {
        match self {
            SignalStatus::Detected => "detected",
            SignalStatus::Filtered => "filtered",
            SignalStatus::Executed => "executed",
            SignalStatus::Expired => "expired",
            SignalStatus::Failed => "failed",
        }
    }

    fn from_string(s: &str) -> Result<Self> {
        match s {
            "detected" => Ok(SignalStatus::Detected),
            "filtered" => Ok(SignalStatus::Filtered),
            "executed" => Ok(SignalStatus::Executed),
            "expired" => Ok(SignalStatus::Expired),
            "failed" => Ok(SignalStatus::Failed),
            _ => Err(ArbitrageError::Storage(format!(
                "Invalid signal status: {}",
                s
            ))),
        }
    }
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

impl ExecutionStatus {
    fn to_string(&self) -> &'static str {
        match self {
            ExecutionStatus::Pending => "pending",
            ExecutionStatus::PartiallyFilled => "partially_filled",
            ExecutionStatus::Completed => "completed",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::Cancelled => "cancelled",
        }
    }

    fn from_string(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(ExecutionStatus::Pending),
            "partially_filled" => Ok(ExecutionStatus::PartiallyFilled),
            "completed" => Ok(ExecutionStatus::Completed),
            "failed" => Ok(ExecutionStatus::Failed),
            "cancelled" => Ok(ExecutionStatus::Cancelled),
            _ => Err(ArbitrageError::Storage(format!(
                "Invalid execution status: {}",
                s
            ))),
        }
    }
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

/// Storage service for persisting signals and executions using SQLite
pub struct StorageService {
    config: StorageConfig,
    pool: SqlitePool,
}

impl StorageService {
    pub async fn new(config: StorageConfig) -> Result<Self> {
        let database_url = format!("sqlite:{}", config.database_path);

        let pool = SqlitePool::connect(&database_url).await.map_err(|e| {
            ArbitrageError::Storage(format!("Failed to connect to database: {}", e))
        })?;

        let service = Self { config, pool };
        service.initialize_database().await?;

        Ok(service)
    }

    /// Initialize database tables
    pub async fn initialize_database(&self) -> Result<()> {
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                ArbitrageError::Storage(format!("Failed to begin transaction: {}", e))
            })?;

        // Create signals table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS signals (
                id TEXT PRIMARY KEY,
                symbol_base TEXT NOT NULL,
                symbol_quote TEXT NOT NULL,
                buy_exchange TEXT NOT NULL,
                sell_exchange TEXT NOT NULL,
                buy_price TEXT NOT NULL,
                sell_price TEXT NOT NULL,
                gross_profit_percent TEXT NOT NULL,
                net_profit_percent TEXT NOT NULL,
                confidence TEXT NOT NULL,
                recommended_size TEXT NOT NULL,
                max_size TEXT NOT NULL,
                expected_slippage TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                metadata TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                confidence_score TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| ArbitrageError::Storage(format!("Failed to create signals table: {}", e)))?;

        // Create executions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS executions (
                signal_id TEXT PRIMARY KEY,
                instruction_data TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                status TEXT NOT NULL,
                actual_profit TEXT,
                execution_time_ms INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            ArbitrageError::Storage(format!("Failed to create executions table: {}", e))
        })?;

        // Create indexes for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_signals_timestamp ON signals(timestamp)")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                ArbitrageError::Storage(format!("Failed to create timestamp index: {}", e))
            })?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_signals_status ON signals(status)")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                ArbitrageError::Storage(format!("Failed to create status index: {}", e))
            })?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_signals_symbol ON signals(symbol_base, symbol_quote)",
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| ArbitrageError::Storage(format!("Failed to create symbol index: {}", e)))?;

        tx.commit()
            .await
            .map_err(|e| ArbitrageError::Storage(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Store a detected signal
    pub async fn store_signal(
        &self,
        signal: Signal,
        confidence_score: Decimal,
        status: SignalStatus,
    ) -> Result<()> {
        let metadata_json = serde_json::to_string(&signal.metadata)
            .map_err(|e| ArbitrageError::Storage(format!("Failed to serialize metadata: {}", e)))?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO signals (
                id, symbol_base, symbol_quote, buy_exchange, sell_exchange,
                buy_price, sell_price, gross_profit_percent, net_profit_percent,
                confidence, recommended_size, max_size, expected_slippage,
                expires_at, metadata, timestamp, confidence_score, status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        )
        .bind(signal.id.to_string())
        .bind(&signal.symbol.base)
        .bind(&signal.symbol.quote)
        .bind(signal.buy_exchange.to_string())
        .bind(signal.sell_exchange.to_string())
        .bind(signal.buy_price.to_string())
        .bind(signal.sell_price.to_string())
        .bind(signal.gross_profit_percent.to_string())
        .bind(signal.net_profit_percent.to_string())
        .bind(signal.confidence.to_string())
        .bind(signal.recommended_size.to_string())
        .bind(signal.max_size.to_string())
        .bind(signal.expected_slippage.to_string())
        .bind(signal.expires_at.to_rfc3339())
        .bind(metadata_json)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(confidence_score.to_string())
        .bind(status.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ArbitrageError::Storage(format!("Failed to store signal: {}", e)))?;

        // Cleanup old signals if needed
        self.cleanup_old_signals().await?;

        Ok(())
    }

    /// Store an execution instruction
    pub async fn store_execution(
        &self,
        instruction: ExecutionInstruction,
        status: ExecutionStatus,
    ) -> Result<()> {
        let instruction_json = serde_json::to_string(&instruction).map_err(|e| {
            ArbitrageError::Storage(format!("Failed to serialize instruction: {}", e))
        })?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO executions (
                signal_id, instruction_data, timestamp, status, actual_profit, execution_time_ms
            ) VALUES (?, ?, ?, ?, ?, ?)
        "#,
        )
        .bind(instruction.signal_id.to_string())
        .bind(instruction_json)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(status.to_string())
        .bind(None::<String>)
        .bind(None::<i64>)
        .execute(&self.pool)
        .await
        .map_err(|e| ArbitrageError::Storage(format!("Failed to store execution: {}", e)))?;

        // Cleanup old executions if needed
        self.cleanup_old_executions().await?;

        Ok(())
    }

    /// Update signal status
    pub async fn update_signal_status(&self, signal_id: Uuid, status: SignalStatus) -> Result<()> {
        sqlx::query("UPDATE signals SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(signal_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| {
                ArbitrageError::Storage(format!("Failed to update signal status: {}", e))
            })?;

        Ok(())
    }

    /// Update execution status and results
    pub async fn update_execution_status(
        &self,
        signal_id: Uuid,
        status: ExecutionStatus,
        actual_profit: Option<Decimal>,
        execution_time_ms: Option<u64>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE executions 
            SET status = ?, actual_profit = ?, execution_time_ms = ?
            WHERE signal_id = ?
        "#,
        )
        .bind(status.to_string())
        .bind(actual_profit.map(|p| p.to_string()))
        .bind(execution_time_ms.map(|t| t as i64))
        .bind(signal_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            ArbitrageError::Storage(format!("Failed to update execution status: {}", e))
        })?;

        Ok(())
    }

    /// Query signals with filters
    pub async fn query_signals(&self, query: &SignalQuery) -> Result<Vec<StoredSignal>> {
        let mut sql = "SELECT * FROM signals WHERE 1=1".to_string();
        let mut params: Vec<String> = Vec::new();

        if let Some(status) = &query.status_filter {
            sql.push_str(" AND status = ?");
            params.push(status.to_string().to_string());
        }

        if let Some(symbol_filter) = &query.symbol_filter {
            sql.push_str(" AND (symbol_base = ? OR symbol_quote = ?)");
            params.push(symbol_filter.clone());
            params.push(symbol_filter.clone());
        }

        if let Some(exchange) = &query.exchange_filter {
            sql.push_str(" AND (buy_exchange = ? OR sell_exchange = ?)");
            let exchange_str = exchange.to_string();
            params.push(exchange_str.clone());
            params.push(exchange_str);
        }

        if let Some(min_confidence) = query.min_confidence {
            sql.push_str(" AND CAST(confidence_score AS REAL) >= ?");
            params.push(min_confidence.to_string());
        }

        if let Some((start, end)) = query.time_range {
            sql.push_str(" AND timestamp BETWEEN ? AND ?");
            params.push(start.to_rfc3339());
            params.push(end.to_rfc3339());
        }

        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = query.limit {
            sql.push_str(" LIMIT ?");
            params.push(limit.to_string());
        }

        let mut query_builder = sqlx::query(&sql);
        for param in params {
            query_builder = query_builder.bind(param);
        }

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ArbitrageError::Storage(format!("Failed to query signals: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            let stored_signal = self.row_to_stored_signal(row)?;
            results.push(stored_signal);
        }

        Ok(results)
    }

    /// Get execution by signal ID
    pub async fn get_execution(&self, signal_id: Uuid) -> Result<Option<StoredExecution>> {
        let row = sqlx::query("SELECT * FROM executions WHERE signal_id = ?")
            .bind(signal_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ArbitrageError::Storage(format!("Failed to get execution: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_stored_execution(row)?))
        } else {
            Ok(None)
        }
    }

    /// Get storage statistics
    pub async fn get_statistics(&self) -> Result<StorageStatistics> {
        let signal_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signals")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ArbitrageError::Storage(format!("Failed to count signals: {}", e)))?;

        let execution_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM executions")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ArbitrageError::Storage(format!("Failed to count executions: {}", e)))?;

        // Get signal status counts
        let status_rows =
            sqlx::query("SELECT status, COUNT(*) as count FROM signals GROUP BY status")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| {
                    ArbitrageError::Storage(format!("Failed to get signal status counts: {}", e))
                })?;

        let mut signal_status_counts = std::collections::HashMap::new();
        for row in status_rows {
            let status_str: String = row.get("status");
            let count: i64 = row.get("count");
            if let Ok(status) = SignalStatus::from_string(&status_str) {
                signal_status_counts.insert(status, count as usize);
            }
        }

        // Get execution status counts
        let exec_status_rows =
            sqlx::query("SELECT status, COUNT(*) as count FROM executions GROUP BY status")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| {
                    ArbitrageError::Storage(format!("Failed to get execution status counts: {}", e))
                })?;

        let mut execution_status_counts = std::collections::HashMap::new();
        for row in exec_status_rows {
            let status_str: String = row.get("status");
            let count: i64 = row.get("count");
            if let Ok(status) = ExecutionStatus::from_string(&status_str) {
                execution_status_counts.insert(status, count as usize);
            }
        }

        Ok(StorageStatistics {
            total_signals: signal_count as usize,
            total_executions: execution_count as usize,
            signal_status_counts,
            execution_status_counts,
            memory_usage_estimate_mb: 0.0, // SQLite handles memory
            avg_query_time_ms: None,
        })
    }

    /// Cleanup old signals
    async fn cleanup_old_signals(&self) -> Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM signals")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ArbitrageError::Storage(format!("Failed to count signals: {}", e)))?;

        if count as usize > self.config.max_signal_history {
            let to_delete = count as usize - self.config.max_signal_history;

            sqlx::query(
                r#"
                DELETE FROM signals 
                WHERE id IN (
                    SELECT id FROM signals 
                    ORDER BY timestamp ASC 
                    LIMIT ?
                )
            "#,
            )
            .bind(to_delete as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                ArbitrageError::Storage(format!("Failed to cleanup old signals: {}", e))
            })?;
        }

        Ok(())
    }

    /// Cleanup old executions
    async fn cleanup_old_executions(&self) -> Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM executions")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ArbitrageError::Storage(format!("Failed to count executions: {}", e)))?;

        if count as usize > self.config.max_execution_history {
            let to_delete = count as usize - self.config.max_execution_history;

            sqlx::query(
                r#"
                DELETE FROM executions 
                WHERE signal_id IN (
                    SELECT signal_id FROM executions 
                    ORDER BY timestamp ASC 
                    LIMIT ?
                )
            "#,
            )
            .bind(to_delete as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                ArbitrageError::Storage(format!("Failed to cleanup old executions: {}", e))
            })?;
        }

        Ok(())
    }

    /// Convert database row to StoredSignal
    fn row_to_stored_signal(&self, row: sqlx::sqlite::SqliteRow) -> Result<StoredSignal> {
        let id_str: String = row.get("id");
        let id = Uuid::parse_str(&id_str)
            .map_err(|e| ArbitrageError::Storage(format!("Invalid UUID: {}", e)))?;

        let symbol = crate::types::Symbol::new(
            row.get::<String, _>("symbol_base"),
            row.get::<String, _>("symbol_quote"),
        );

        let buy_exchange = row
            .get::<String, _>("buy_exchange")
            .parse()
            .map_err(|e| ArbitrageError::Storage(format!("Invalid buy exchange: {}", e)))?;
        let sell_exchange = row
            .get::<String, _>("sell_exchange")
            .parse()
            .map_err(|e| ArbitrageError::Storage(format!("Invalid sell exchange: {}", e)))?;

        let buy_price = Decimal::from_str(&row.get::<String, _>("buy_price"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid buy price: {}", e)))?;
        let sell_price = Decimal::from_str(&row.get::<String, _>("sell_price"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid sell price: {}", e)))?;

        let mut signal = Signal::new(
            symbol,
            buy_exchange,
            sell_exchange,
            buy_price,
            sell_price,
            Utc::now(),
        );
        signal.id = id;

        signal.gross_profit_percent =
            Decimal::from_str(&row.get::<String, _>("gross_profit_percent"))
                .map_err(|e| ArbitrageError::Storage(format!("Invalid gross profit: {}", e)))?;
        signal.net_profit_percent = Decimal::from_str(&row.get::<String, _>("net_profit_percent"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid net profit: {}", e)))?;
        signal.confidence = Decimal::from_str(&row.get::<String, _>("confidence"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid confidence: {}", e)))?;
        signal.recommended_size = Decimal::from_str(&row.get::<String, _>("recommended_size"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid recommended size: {}", e)))?;
        signal.max_size = Decimal::from_str(&row.get::<String, _>("max_size"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid max size: {}", e)))?;
        signal.expected_slippage = Decimal::from_str(&row.get::<String, _>("expected_slippage"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid expected slippage: {}", e)))?;

        let expires_at_str: String = row.get("expires_at");
        signal.expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at_str)
            .map_err(|e| ArbitrageError::Storage(format!("Invalid expires_at: {}", e)))?
            .with_timezone(&chrono::Utc);

        let metadata_str: String = row.get("metadata");
        signal.metadata = serde_json::from_str(&metadata_str)
            .map_err(|e| ArbitrageError::Storage(format!("Invalid metadata: {}", e)))?;

        let timestamp_str: String = row.get("timestamp");
        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
            .map_err(|e| ArbitrageError::Storage(format!("Invalid timestamp: {}", e)))?
            .with_timezone(&chrono::Utc);

        let confidence_score = Decimal::from_str(&row.get::<String, _>("confidence_score"))
            .map_err(|e| ArbitrageError::Storage(format!("Invalid confidence score: {}", e)))?;

        let status_str: String = row.get("status");
        let status = SignalStatus::from_string(&status_str)?;

        Ok(StoredSignal {
            signal,
            timestamp,
            confidence_score,
            status,
        })
    }

    /// Convert database row to StoredExecution
    fn row_to_stored_execution(&self, row: sqlx::sqlite::SqliteRow) -> Result<StoredExecution> {
        let signal_id_str: String = row.get("signal_id");
        let signal_id = Uuid::parse_str(&signal_id_str)
            .map_err(|e| ArbitrageError::Storage(format!("Invalid signal UUID: {}", e)))?;

        let instruction_json: String = row.get("instruction_data");
        let mut instruction: ExecutionInstruction = serde_json::from_str(&instruction_json)
            .map_err(|e| ArbitrageError::Storage(format!("Invalid instruction data: {}", e)))?;
        instruction.signal_id = signal_id;

        let timestamp_str: String = row.get("timestamp");
        let timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp_str)
            .map_err(|e| ArbitrageError::Storage(format!("Invalid timestamp: {}", e)))?
            .with_timezone(&chrono::Utc);

        let status_str: String = row.get("status");
        let status = ExecutionStatus::from_string(&status_str)?;

        let actual_profit =
            if let Some(profit_str) = row.get::<Option<String>, _>("actual_profit") {
                Some(Decimal::from_str(&profit_str).map_err(|e| {
                    ArbitrageError::Storage(format!("Invalid actual profit: {}", e))
                })?)
            } else {
                None
            };

        let execution_time_ms = row
            .get::<Option<i64>, _>("execution_time_ms")
            .map(|t| t as u64);

        Ok(StoredExecution {
            instruction,
            timestamp,
            status,
            actual_profit,
            execution_time_ms,
        })
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
    pub signal_status_counts: std::collections::HashMap<SignalStatus, usize>,
    pub execution_status_counts: std::collections::HashMap<ExecutionStatus, usize>,
    pub memory_usage_estimate_mb: f64,
    pub avg_query_time_ms: Option<f64>,
}
