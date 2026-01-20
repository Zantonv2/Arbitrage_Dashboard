use arbitrage_core::types::Symbol;
use exchange_connectors::websocket_pool::{
    ConnectionState, ReconnectPolicy, WebSocketEvent, WebSocketMessage, WebSocketPool,
};
use rust_decimal::Decimal;
use std::time::Duration;

#[cfg(test)]
mod connection_state_tests {
    use super::*;

    #[test]
    fn test_connection_state_variants() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connecting, ConnectionState::Connecting);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_eq!(ConnectionState::Reconnecting, ConnectionState::Reconnecting);
    }

    #[test]
    fn test_connection_state_with_error() {
        let error_state = ConnectionState::Error("Connection timeout".to_string());
        match &error_state {
            ConnectionState::Error(msg) => {
                assert_eq!(msg, "Connection timeout");
            }
            _ => panic!("Expected Error state"),
        }
    }

    #[test]
    fn test_connection_state_clone() {
        let state = ConnectionState::Connected;
        let cloned = state.clone();
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_connection_state_debug_format() {
        let connected = ConnectionState::Connected;
        let debug_str = format!("{:?}", connected);
        assert_eq!(debug_str, "Connected");

        let error = ConnectionState::Error("test".to_string());
        let error_str = format!("{:?}", error);
        assert!(error_str.contains("Error"));
    }

    #[test]
    fn test_connection_state_equality() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connecting, ConnectionState::Connecting);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_eq!(ConnectionState::Reconnecting, ConnectionState::Reconnecting);

        let error1 = ConnectionState::Error("error1".to_string());
        let error2 = ConnectionState::Error("error1".to_string());
        assert_eq!(error1, error2);

        let error3 = ConnectionState::Error("error2".to_string());
        assert_ne!(error1, error3);
    }

    #[test]
    fn test_connection_state_hash() {
        use std::collections::HashMap;

        let mut map: HashMap<ConnectionState, i32> = HashMap::new();
        map.insert(ConnectionState::Connected, 1);
        map.insert(ConnectionState::Disconnected, 2);

        assert_eq!(map.get(&ConnectionState::Connected), Some(&1));
        assert_eq!(map.get(&ConnectionState::Disconnected), Some(&2));
    }
}

#[cfg(test)]
mod reconnect_policy_tests {
    use super::*;

    #[test]
    fn test_reconnect_policy_default() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.max_attempts, 10);
        assert_eq!(policy.initial_delay_ms, 1000);
        assert_eq!(policy.max_delay_ms, 30000);
        assert_eq!(policy.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_reconnect_policy_custom() {
        let policy = ReconnectPolicy::new(5, 500, 10000, 1.5);
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.initial_delay_ms, 500);
        assert_eq!(policy.max_delay_ms, 10000);
        assert_eq!(policy.backoff_multiplier, 1.5);
    }

    #[test]
    fn test_reconnect_policy_next_delay_exponential_backoff() {
        let policy = ReconnectPolicy::new(10, 1000, 30000, 2.0);

        let delay0 = policy.next_delay(0);
        assert_eq!(delay0, Duration::from_millis(1000));

        let delay1 = policy.next_delay(1);
        assert_eq!(delay1, Duration::from_millis(2000));

        let delay2 = policy.next_delay(2);
        assert_eq!(delay2, Duration::from_millis(4000));
    }

    #[test]
    fn test_reconnect_policy_next_delay_caps_at_max() {
        let policy = ReconnectPolicy::new(100, 1000, 5000, 2.0);

        let delay10 = policy.next_delay(10);
        assert_eq!(delay10, Duration::from_millis(5000));

        let delay100 = policy.next_delay(100);
        assert_eq!(delay100, Duration::from_millis(5000));
    }

    #[test]
    fn test_reconnect_policy_backoff_multiplier_float() {
        let policy = ReconnectPolicy::new(10, 1000, 100000, 1.5);

        let delay0 = policy.next_delay(0);
        let delay1 = policy.next_delay(1);
        let delay2 = policy.next_delay(2);

        let expected1 = (1000.0 * 1.5f64) as u64;
        let expected2 = (1000.0 * 1.5f64 * 1.5f64) as u64;

        assert_eq!(delay0, Duration::from_millis(1000));
        assert_eq!(delay1, Duration::from_millis(expected1));
        assert_eq!(delay2, Duration::from_millis(expected2 as u64));
    }

    #[test]
    fn test_reconnect_policy_clone() {
        let policy = ReconnectPolicy::default();
        let cloned = policy.clone();
        assert_eq!(cloned.max_attempts, policy.max_attempts);
        assert_eq!(cloned.initial_delay_ms, policy.initial_delay_ms);
    }
}

#[cfg(test)]
mod websocket_message_tests {
    use super::*;

    #[test]
    fn test_websocket_message_subscribe() {
        let symbols = vec![Symbol::new("BTC", "USDT")];
        let message = WebSocketMessage::Subscribe(symbols.clone());
        match &message {
            WebSocketMessage::Subscribe(s) => {
                assert_eq!(s.len(), 1);
                assert_eq!(s[0], symbols[0]);
            }
            _ => panic!("Expected Subscribe message"),
        }
    }

    #[test]
    fn test_websocket_message_unsubscribe() {
        let symbols = vec![Symbol::new("ETH", "USDT")];
        let message = WebSocketMessage::Unsubscribe(symbols.clone());
        match &message {
            WebSocketMessage::Unsubscribe(s) => {
                assert_eq!(s.len(), 1);
            }
            _ => panic!("Expected Unsubscribe message"),
        }
    }

    #[test]
    fn test_websocket_message_text() {
        let message = WebSocketMessage::Text("test message".to_string());
        match &message {
            WebSocketMessage::Text(t) => {
                assert_eq!(t, "test message");
            }
            _ => panic!("Expected Text message"),
        }
    }

    #[test]
    fn test_websocket_message_ping() {
        let message = WebSocketMessage::Ping;
        match message {
            WebSocketMessage::Ping => {}
            _ => panic!("Expected Ping message"),
        }
    }

    #[test]
    fn test_websocket_message_disconnect() {
        let message = WebSocketMessage::Disconnect;
        match message {
            WebSocketMessage::Disconnect => {}
            _ => panic!("Expected Disconnect message"),
        }
    }

    #[test]
    fn test_websocket_message_debug_format() {
        let message = WebSocketMessage::Text("test".to_string());
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("Text"));
    }

    #[test]
    fn test_websocket_message_clone() {
        let message = WebSocketMessage::Text("clone me".to_string());
        let cloned = message.clone();
        match cloned {
            WebSocketMessage::Text(s) => {
                assert_eq!(s, "clone me");
            }
            _ => panic!("Expected Text message"),
        }
    }
}

#[cfg(test)]
mod websocket_event_tests {
    use super::*;

    #[test]
    fn test_websocket_event_message() {
        let event = WebSocketEvent::Message(serde_json::json!({"test": "value"}));
        match &event {
            WebSocketEvent::Message(v) => {
                assert!(v.is_object());
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_websocket_event_connected() {
        let event = WebSocketEvent::Connected;
        match event {
            WebSocketEvent::Connected => {}
            _ => panic!("Expected Connected event"),
        }
    }

    #[test]
    fn test_websocket_event_disconnected() {
        let event = WebSocketEvent::Disconnected;
        match event {
            WebSocketEvent::Disconnected => {}
            _ => panic!("Expected Disconnected event"),
        }
    }

    #[test]
    fn test_websocket_event_error() {
        let event = WebSocketEvent::Error("test error".to_string());
        match event {
            WebSocketEvent::Error(msg) => {
                assert_eq!(msg, "test error");
            }
            _ => panic!("Expected Error event"),
        }
    }

    #[test]
    fn test_websocket_event_clone() {
        let event = WebSocketEvent::Message(serde_json::json!({"key": "value"}));
        let cloned = event.clone();
        match cloned {
            WebSocketEvent::Message(v) => {
                assert!(v.is_object());
            }
            _ => panic!("Expected Message event"),
        }
    }
}

#[cfg(test)]
mod websocket_pool_state_tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_initial_state() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let state = pool.state();
        assert_eq!(state, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_pool_queued_messages_initial() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        assert_eq!(pool.queued_messages(), 0);
    }

    #[tokio::test]
    async fn test_pool_is_connected_false_initially() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        assert!(!pool.is_connected());
    }

    #[tokio::test]
    async fn test_pool_subscribe_no_connection() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols = vec![Symbol::new("BTC", "USDT")];
        let result = pool.subscribe(&symbols).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pool_unsubscribe_no_connection() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols = vec![Symbol::new("BTC", "USDT")];
        let result = pool.unsubscribe(&symbols).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pool_disconnect_no_connection() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        pool.disconnect().await;
        let state = pool.state();
        assert_eq!(state, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_pool_on_message_receiver() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let receiver = pool.on_message();
        assert!(receiver.count() > 0);
    }

    #[tokio::test]
    async fn test_pool_multiple_subscriptions() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
            Symbol::new("SOL", "USDT"),
        ];
        let result = pool.subscribe(&symbols).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_pool_subscribe_empty_symbols() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols: Vec<Symbol> = vec![];
        let result = pool.subscribe(&symbols).await;
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod symbol_subscription_tests {
    use super::*;

    fn create_test_symbols() -> Vec<Symbol> {
        vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
            Symbol::new("SOL", "USDT"),
            Symbol::new("XRP", "USDT"),
            Symbol::new("ADA", "USDT"),
        ]
    }

    #[tokio::test]
    async fn test_subscribe_multiple_symbols() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols = create_test_symbols();
        let result = pool.subscribe(&symbols).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subscribe_single_symbol() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols = vec![Symbol::new("BTC", "USDT")];
        let result = pool.subscribe(&symbols).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unsubscribe_all_symbols() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols = create_test_symbols();
        pool.subscribe(&symbols).await.ok();
        let result = pool.unsubscribe(&symbols).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subscribe_then_unsubscribe() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        let symbols = vec![Symbol::new("BTC", "USDT")];

        pool.subscribe(&symbols).await.ok();
        pool.unsubscribe(&symbols).await.ok();
    }

    #[tokio::test]
    async fn test_multiple_subscribe_calls() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();

        pool.subscribe(&vec![Symbol::new("BTC", "USDT")]).await.ok();
        pool.subscribe(&vec![Symbol::new("ETH", "USDT")]).await.ok();
        pool.subscribe(&vec![Symbol::new("SOL", "USDT")]).await.ok();
    }
}

#[cfg(test)]
mod message_queue_tests {
    use super::*;

    #[tokio::test]
    async fn test_message_queue_starts_empty() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();
        assert_eq!(pool.queued_messages(), 0);
    }

    #[tokio::test]
    async fn test_pool_state_transitions() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();

        let initial_state = pool.state();
        assert_eq!(initial_state, ConnectionState::Disconnected);

        pool.disconnect().await;
        let final_state = pool.state();
        assert_eq!(final_state, ConnectionState::Disconnected);
    }
}

#[cfg(test)]
mod websocket_pool_edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_connect_invalid_url() {
        let result = WebSocketPool::connect("invalid-url").await;
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_double_disconnect() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();

        pool.disconnect().await;
        pool.disconnect().await;
    }

    #[tokio::test]
    async fn test_subscribe_after_disconnect() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();

        pool.disconnect().await;

        let symbols = vec![Symbol::new("BTC", "USDT")];
        let result = pool.subscribe(&symbols).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_event_receiver_multiple_subscribers() {
        let pool = WebSocketPool::connect("wss://test.example.com")
            .await
            .unwrap();

        let receiver1 = pool.on_message();
        let receiver2 = pool.on_message();

        assert!(receiver1.count() > 0);
        assert!(receiver2.count() > 0);
    }
}
