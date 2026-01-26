use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub id: String,
    pub message_type: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct ConnectedClient {
    pub id: String,
    pub ip_address: String,
    pub connected_at: u64,
    pub last_ping: u64,
}

#[derive(Debug)]
pub struct WebSocketManager {
    clients: Arc<RwLock<HashMap<String, ConnectedClient>>>,
    message_sender: broadcast::Sender<WebSocketMessage>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        let (message_sender, _) = broadcast::channel(1000);
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            message_sender,
        }
    }

    pub async fn add_client(&self, ip_address: String) -> String {
        let client_id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let client = ConnectedClient {
            id: client_id.clone(),
            ip_address,
            connected_at: now,
            last_ping: now,
        };

        self.clients.write().await.insert(client_id.clone(), client);
        client_id
    }

    pub async fn remove_client(&self, client_id: &str) -> bool {
        self.clients.write().await.remove(client_id).is_some()
    }

    pub async fn get_client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    pub async fn get_client(&self, client_id: &str) -> Option<ConnectedClient> {
        self.clients.read().await.get(client_id).cloned()
    }

    pub async fn update_ping(&self, client_id: &str) -> bool {
        if let Some(client) = self.clients.write().await.get_mut(client_id) {
            client.last_ping = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            true
        } else {
            false
        }
    }

    pub async fn broadcast_message(&self, message_type: String, data: serde_json::Value) -> Result<usize, String> {
        let message = WebSocketMessage {
            id: Uuid::new_v4().to_string(),
            message_type,
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        match self.message_sender.send(message.clone()) {
            Ok(receiver_count) => Ok(receiver_count),
            Err(e) => Err(format!("Failed to broadcast message: {}", e)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.message_sender.subscribe()
    }

    pub async fn cleanup_stale_connections(&self, timeout_seconds: u64) -> usize {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut clients = self.clients.write().await;
        let initial_count = clients.len();
        
        clients.retain(|_, client| now - client.last_ping <= timeout_seconds);
        
        initial_count - clients.len()
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_websocket_manager_new() {
        let manager = WebSocketManager::new();
        assert_eq!(manager.get_client_count().await, 0);
    }

    #[tokio::test]
    async fn test_add_client() {
        let manager = WebSocketManager::new();
        let client_id = manager.add_client("127.0.0.1".to_string()).await;
        
        assert!(!client_id.is_empty());
        assert_eq!(manager.get_client_count().await, 1);
        
        let client = manager.get_client(&client_id).await.unwrap();
        assert_eq!(client.ip_address, "127.0.0.1");
        assert_eq!(client.id, client_id);
    }

    #[tokio::test]
    async fn test_remove_client() {
        let manager = WebSocketManager::new();
        let client_id = manager.add_client("192.168.1.1".to_string()).await;
        
        assert_eq!(manager.get_client_count().await, 1);
        
        let removed = manager.remove_client(&client_id).await;
        assert!(removed);
        assert_eq!(manager.get_client_count().await, 0);
        
        // Test removing non-existent client
        let removed_again = manager.remove_client(&client_id).await;
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn test_get_client() {
        let manager = WebSocketManager::new();
        let client_id = manager.add_client("10.0.0.1".to_string()).await;
        
        let client = manager.get_client(&client_id).await;
        assert!(client.is_some());
        
        let non_existent = manager.get_client("non_existent").await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_update_ping() {
        let manager = WebSocketManager::new();
        let client_id = manager.add_client("127.0.0.1".to_string()).await;
        
        let original_client = manager.get_client(&client_id).await.unwrap();
        let original_ping = original_client.last_ping;
        
        // Wait a bit to ensure timestamp difference
        sleep(Duration::from_millis(10)).await;
        
        let updated = manager.update_ping(&client_id).await;
        assert!(updated);
        
        let updated_client = manager.get_client(&client_id).await.unwrap();
        assert!(updated_client.last_ping > original_ping);
        
        // Test updating non-existent client
        let not_updated = manager.update_ping("non_existent").await;
        assert!(!not_updated);
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let manager = WebSocketManager::new();
        let mut receiver = manager.subscribe();
        
        let test_data = serde_json::json!({"message": "test"});
        let result = manager.broadcast_message("test_type".to_string(), test_data.clone()).await;
        
        assert!(result.is_ok());
        
        // Check if message was received
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.message_type, "test_type");
        assert_eq!(received.data, test_data);
        assert!(!received.id.is_empty());
        assert!(received.timestamp > 0);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let manager = WebSocketManager::new();
        let mut receiver1 = manager.subscribe();
        let mut receiver2 = manager.subscribe();
        
        let test_data = serde_json::json!({"broadcast": "test"});
        let result = manager.broadcast_message("broadcast_test".to_string(), test_data).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2); // Two subscribers
        
        // Both should receive the message
        let msg1 = receiver1.recv().await.unwrap();
        let msg2 = receiver2.recv().await.unwrap();
        
        assert_eq!(msg1.id, msg2.id);
        assert_eq!(msg1.message_type, "broadcast_test");
        assert_eq!(msg2.message_type, "broadcast_test");
    }

    #[tokio::test]
    async fn test_cleanup_stale_connections() {
        let manager = WebSocketManager::new();
        
        // Add clients
        let client1 = manager.add_client("127.0.0.1".to_string()).await;
        let client2 = manager.add_client("127.0.0.2".to_string()).await;
        
        assert_eq!(manager.get_client_count().await, 2);
        
        // Update ping for client2
        manager.update_ping(&client2).await;
        
        // Wait a bit to make client1 stale
        sleep(Duration::from_millis(10)).await;
        
        // Cleanup with very short timeout
        let cleaned = manager.cleanup_stale_connections(0).await;
        assert_eq!(cleaned, 1);
        assert_eq!(manager.get_client_count().await, 1);
        
        // client2 should still exist
        assert!(manager.get_client(&client2).await.is_some());
        assert!(manager.get_client(&client1).await.is_none());
    }

    #[tokio::test]
    async fn test_websocket_message_serialization() {
        let message = WebSocketMessage {
            id: "test123".to_string(),
            message_type: "test_type".to_string(),
            data: serde_json::json!({"key": "value"}),
            timestamp: 1234567890,
        };
        
        let json = serde_json::to_string(&message).unwrap();
        let deserialized: WebSocketMessage = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.id, message.id);
        assert_eq!(deserialized.message_type, message.message_type);
        assert_eq!(deserialized.data, message.data);
        assert_eq!(deserialized.timestamp, message.timestamp);
    }

    #[test]
    fn test_connected_client_creation() {
        let client = ConnectedClient {
            id: "client123".to_string(),
            ip_address: "192.168.1.1".to_string(),
            connected_at: 1234567890,
            last_ping: 1234567890,
        };
        
        assert_eq!(client.id, "client123");
        assert_eq!(client.ip_address, "192.168.1.1");
        assert_eq!(client.connected_at, 1234567890);
        assert_eq!(client.last_ping, 1234567890);
    }

    #[tokio::test]
    async fn test_websocket_manager_default() {
        let manager = WebSocketManager::default();
        assert_eq!(manager.get_client_count().await, 0);
    }
}
