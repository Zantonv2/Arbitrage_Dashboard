use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditDecision {
    SignalDetected,
    SignalEmitted,
    ExecutionPrepared,
    ExecutionConfirmed,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionRejected,
    OrderPlaced,
    OrderFilled,
    OrderCancelled,
    ErrorOccurred,
    ConfigChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub decision: AuditDecision,
    pub opportunity_id: Option<Uuid>,
    pub symbol: Option<String>,
    pub buy_exchange: Option<String>,
    pub sell_exchange: Option<String>,
    pub expected_profit: Option<String>,
    pub actual_profit: Option<String>,
    pub status: String,
    pub details: String,
}

impl AuditEntry {
    pub fn new(decision: AuditDecision, status: &str, details: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            decision,
            opportunity_id: None,
            symbol: None,
            buy_exchange: None,
            sell_exchange: None,
            expected_profit: None,
            actual_profit: None,
            status: status.to_string(),
            details: details.to_string(),
        }
    }

    pub fn with_opportunity(mut self, opportunity_id: Uuid, symbol: &str) -> Self {
        self.opportunity_id = Some(opportunity_id);
        self.symbol = Some(symbol.to_string());
        self
    }

    pub fn with_exchanges(mut self, buy: &str, sell: &str) -> Self {
        self.buy_exchange = Some(buy.to_string());
        self.sell_exchange = Some(sell.to_string());
        self
    }

    pub fn with_profit(mut self, expected: &str, actual: Option<&str>) -> Self {
        self.expected_profit = Some(expected.to_string());
        self.actual_profit = actual.map(|s| s.to_string());
        self
    }
}

#[derive(Clone)]
pub struct AuditLogger {
    entries: Arc<Mutex<Vec<AuditEntry>>>,
    max_entries: usize,
}

impl AuditLogger {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            max_entries,
        }
    }

    pub async fn log(&self, entry: AuditEntry) {
        let mut entries = self.entries.lock().await;
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(entry);
    }

    pub async fn log_decision(&self, decision: AuditDecision, status: &str, details: &str) {
        let entry = AuditEntry::new(decision, status, details);
        self.log(entry).await;
    }

    pub async fn log_execution(
        &self,
        opportunity_id: Uuid,
        symbol: &str,
        buy_exchange: &str,
        sell_exchange: &str,
        expected_profit: &str,
        actual_profit: Option<&str>,
        status: &str,
    ) {
        let entry = AuditEntry::new(AuditDecision::ExecutionConfirmed, status, "Trade executed")
            .with_opportunity(opportunity_id, symbol)
            .with_exchanges(buy_exchange, sell_exchange)
            .with_profit(expected_profit, actual_profit);
        self.log(entry).await;
    }

    pub async fn log_error(&self, _error_type: &str, context: &str) {
        let entry = AuditEntry::new(AuditDecision::ErrorOccurred, "error", context)
            .with_profit("N/A", None);
        self.log(entry).await;
    }

    pub async fn get_all_entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().await.clone()
    }

    pub async fn get_recent(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.lock().await;
        let start = entries.len().saturating_sub(limit);
        entries[start..].to_vec()
    }

    pub async fn get_entries_by_decision(&self, decision: AuditDecision) -> Vec<AuditEntry> {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.decision == decision)
            .cloned()
            .collect()
    }

    /// Persist audit log to file for immutable storage
    /// This creates a new file with all current entries
    pub async fn persist_to_file(&self, path: &PathBuf) -> std::io::Result<()> {
        let entries = self.entries.lock().await;
        let json = serde_json::to_string_pretty(&*entries)?;
        drop(entries);

        let file = File::create(path).await?;
        let mut writer = BufWriter::new(file);
        writer.write_all(json.as_bytes()).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Append a single entry to the audit log file
    /// This maintains immutability by only appending new entries
    pub async fn append_to_file(&self, path: &PathBuf) -> std::io::Result<()> {
        let entries = self.entries.lock().await;
        
        // Get the last entry for appending
        if let Some(last_entry) = entries.last() {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await?;

            let json = serde_json::to_string(last_entry)?;
            file.write_all(format!("\n{}", json).as_bytes()).await?;
            file.flush().await?;
        }

        Ok(())
    }

    /// Export audit log to CSV format for compliance reporting
    pub async fn export_to_csv(&self, path: &PathBuf) -> std::io::Result<()> {
        let entries = self.entries.lock().await;
        
        let mut csv_content = String::from("id,timestamp,decision,opportunity_id,symbol,buy_exchange,sell_exchange,expected_profit,actual_profit,status,details\n");
        
        for entry in entries.iter() {
            csv_content.push_str(&format!(
                "{},{},{:?},{},{},{},{},{},{},{},{}",
                entry.id,
                entry.timestamp.to_rfc3339(),
                entry.decision,
                entry.opportunity_id.map(|u| u.to_string()).unwrap_or_default(),
                entry.symbol.as_deref().unwrap_or(""),
                entry.buy_exchange.as_deref().unwrap_or(""),
                entry.sell_exchange.as_deref().unwrap_or(""),
                entry.expected_profit.as_deref().unwrap_or(""),
                entry.actual_profit.as_deref().unwrap_or(""),
                entry.status,
                entry.details.replace(",", ";") // Escape commas
            ));
            csv_content.push('\n');
        }

        let file = File::create(path).await?;
        let mut writer = BufWriter::new(file);
        writer.write_all(csv_content.as_bytes()).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Get audit entries within a time range
    pub async fn get_entries_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<AuditEntry> {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .cloned()
            .collect()
    }

    /// Get total entry count
    pub async fn len(&self) -> usize {
        self.entries.lock().await.len()
    }

    /// Check if audit log is empty
    pub async fn is_empty(&self) -> bool {
        self.entries.lock().await.is_empty()
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(10000)
    }
}
