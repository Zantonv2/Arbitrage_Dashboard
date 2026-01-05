// Hyperliquid connector - placeholder for Phase 2
// This will be implemented when adding multi-exchange support

use crate::connector::{ConnectorConfig, ConnectorError, ConnectionEvent, ExchangeConnector};
use arbitrage_core::types::{ConnectionStatus, ExchangeId, Symbol};
use async_trait::async_trait;
use tokio::sync::broadcast;

/// Hyperliquid exchange connector (placeholder)
pub struct HyperliquidConnector {
    config: ConnectorConfig,
    status: ConnectionStatus,
    event_sender: broadcast::Sender<ConnectionEvent>,
    subscribed_symbols: Vec<Symbol>,
}

impl HyperliquidConnector {
    pub fn new(config: ConnectorConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        Self {
            config,
            status: ConnectionStatus::Disconnected,
            event_sender,
            subscribed_symbols: Vec::new(),
        }
    }
}

#[async_trait]
impl ExchangeConnector for HyperliquidConnector {
    fn exchange_id(&self) -> ExchangeId {
        ExchangeId::Hyperliquid
    }

    async fn connect(&mut self) -> Result<(), ConnectorError> {
        // TODO: Implement in Phase 2
        Err(ConnectorError::Generic(anyhow::anyhow!("Hyperliquid connector not implemented yet")))
    }

    async fn disconnect(&mut self) -> Result<(), ConnectorError> {
        Ok(())
    }

    async fn subscribe(&mut self, _symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        Err(ConnectorError::Generic(anyhow::anyhow!("Hyperliquid connector not implemented yet")))
    }

    async fn unsubscribe(&mut self, _symbols: Vec<Symbol>) -> Result<(), ConnectorError> {
        Ok(())
    }

    fn status(&self) -> ConnectionStatus {
        self.status.clone()
    }

    fn event_receiver(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.event_sender.subscribe()
    }

    fn subscribed_symbols(&self) -> Vec<Symbol> {
        self.subscribed_symbols.clone()
    }
}