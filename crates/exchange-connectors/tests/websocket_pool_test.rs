#[cfg(test)]
mod tests {
    use arbitrage_core::types::Symbol;
    use exchange_connectors::websocket_pool::{
        ConnectionState, ReconnectPolicy, WebSocketEvent, WebSocketMessage, WebSocketPool,
    };

    // === Happy Path Tests ===

    #[test]
    fn test_connection_state_variants() {
        let states = vec![
            ConnectionState::Disconnected,
            ConnectionState::Connecting,
            ConnectionState::Connected,
            ConnectionState::Reconnecting,
            ConnectionState::Error("test error".to_string()),
        ];

        for state in states {
            let debug_str = format!("{:?}", state);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_connection_state_equality() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connecting, ConnectionState::Connecting);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_eq!(ConnectionState::Reconnecting, ConnectionState::Reconnecting);
        assert_ne!(ConnectionState::Connected, ConnectionState::Disconnected);
    }

    #[test]
    fn test_reconnect_policy_default() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.max_attempts, 10);
        assert_eq!(policy.initial_delay_ms, 1000);
        assert_eq!(policy.max_delay_ms, 30000);
        assert_eq!(policy.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_reconnect_policy_new() {
        let policy = ReconnectPolicy::new(5, 500, 10000, 1.5);
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.initial_delay_ms, 500);
        assert_eq!(policy.max_delay_ms, 10000);
        assert_eq!(policy.backoff_multiplier, 1.5);
    }

    #[test]
    fn test_reconnect_policy_next_delay_first_attempt() {
        let policy = ReconnectPolicy::new(10, 1000, 30000, 2.0);
        let delay = policy.next_delay(0);
        // First attempt should use initial delay
        assert_eq!(delay.as_millis(), 1000);
    }

    #[test]
    fn test_reconnect_policy_next_delay_exponential_backoff() {
        let policy = ReconnectPolicy::new(10, 1000, 30000, 2.0);

        let delay0 = policy.next_delay(0);
        let delay1 = policy.next_delay(1);
        let delay2 = policy.next_delay(2);

        // Each attempt should double (with some tolerance)
        assert!(delay1 > delay0);
        assert!(delay2 > delay1);
    }

    #[test]
    fn test_reconnect_policy_next_delay_max_cap() {
        let policy = ReconnectPolicy::new(10, 1000, 5000, 10.0); // Very aggressive backoff

        let delay = policy.next_delay(10);
        // Should be capped at max_delay_ms
        assert!(delay.as_millis() <= 5000);
    }

    #[test]
    fn test_websocket_message_variants() {
        let messages = vec![
            WebSocketMessage::Subscribe(vec![Symbol::new("BTC", "USDT")]),
            WebSocketMessage::Unsubscribe(vec![Symbol::new("ETH", "USDT")]),
            WebSocketMessage::Text("test message".to_string()),
            WebSocketMessage::Ping,
            WebSocketMessage::Disconnect,
        ];

        for message in messages {
            let debug_str = format!("{:?}", message);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_websocket_event_variants() {
        let events = vec![
            WebSocketEvent::Connected,
            WebSocketEvent::Disconnected,
            WebSocketEvent::Error("test error".to_string()),
        ];

        for event in events {
            let debug_str = format!("{:?}", event);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_reconnect_policy_clone() {
        let policy = ReconnectPolicy::default();
        let cloned = policy.clone();
        assert_eq!(cloned.max_attempts, policy.max_attempts);
        assert_eq!(cloned.initial_delay_ms, policy.initial_delay_ms);
    }

    // === Edge Case Tests ===

    #[test]
    fn test_reconnect_policy_zero_attempts() {
        let policy = ReconnectPolicy::new(0, 1000, 30000, 2.0);
        let delay = policy.next_delay(0);
        assert_eq!(delay.as_millis(), 1000);
    }

    #[test]
    fn test_reconnect_policy_zero_initial_delay() {
        let policy = ReconnectPolicy::new(10, 0, 30000, 2.0);
        let delay = policy.next_delay(0);
        assert_eq!(delay.as_millis(), 0);
    }

    #[test]
    fn test_reconnect_policy_zero_backoff() {
        let policy = ReconnectPolicy::new(10, 1000, 30000, 0.0);
        let delay0 = policy.next_delay(0);
        let delay1 = policy.next_delay(1);
        // With 0 backoff, delays should be equal
        assert_eq!(delay0, delay1);
    }

    #[test]
    fn test_reconnect_policy_max_attempts_limit() {
        let policy = ReconnectPolicy::new(3, 1000, 30000, 2.0);

        // After max attempts, should still return a delay (for logging purposes)
        let delay = policy.next_delay(100); // Many attempts
        assert!(delay.as_millis() > 0);
    }

    #[test]
    fn test_connection_state_hash() {
        use std::collections::HashSet;

        let mut set: HashSet<ConnectionState> = HashSet::new();
        set.insert(ConnectionState::Disconnected);
        set.insert(ConnectionState::Connecting);
        set.insert(ConnectionState::Connected);
        set.insert(ConnectionState::Reconnecting);
        set.insert(ConnectionState::Error("error1".to_string()));
        set.insert(ConnectionState::Error("error2".to_string()));

        assert_eq!(set.len(), 6); // Different error messages = different states
    }

    #[test]
    fn test_websocket_message_with_empty_symbols() {
        let message = WebSocketMessage::Subscribe(vec![]);
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("Subscribe"));
        assert!(debug_str.contains("0"));
    }

    #[test]
    fn test_websocket_message_with_multiple_symbols() {
        let symbols = vec![
            Symbol::new("BTC", "USDT"),
            Symbol::new("ETH", "USDT"),
            Symbol::new("SOL", "USDT"),
        ];
        let message = WebSocketMessage::Subscribe(symbols);
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("Subscribe"));
    }

    // === Boundary Condition Tests ===

    #[test]
    fn test_reconnect_policy_large_backoff_multiplier() {
        let policy = ReconnectPolicy::new(5, 1, 1000, 100.0);
        let delay = policy.next_delay(1);
        // Should still be capped
        assert!(delay.as_millis() <= 1000);
    }

    #[test]
    fn test_reconnect_policy_small_initial_delay() {
        let policy = ReconnectPolicy::new(10, 1, 1000, 2.0);
        let delay = policy.next_delay(0);
        assert_eq!(delay.as_millis(), 1);
    }

    #[test]
    fn test_connection_state_display_trait() {
        let states = vec![
            ConnectionState::Disconnected,
            ConnectionState::Connecting,
            ConnectionState::Connected,
            ConnectionState::Reconnecting,
        ];

        for state in states {
            let display_str = format!("{}", state);
            assert!(!display_str.is_empty());
        }
    }

    // === Type Conversion Tests ===

    #[test]
    fn test_websocket_event_clone() {
        let event = WebSocketEvent::Connected;
        let cloned = event.clone();
        assert_eq!(format!("{:?}", cloned), format!("{:?}", event));
    }

    #[test]
    fn test_websocket_message_clone() {
        let message = WebSocketMessage::Text("test".to_string());
        let cloned = message.clone();
        assert_eq!(format!("{:?}", cloned), format!("{:?}", message));
    }

    #[test]
    fn test_connection_state_clone() {
        let state = ConnectionState::Error("test".to_string());
        let cloned = state.clone();
        match cloned {
            ConnectionState::Error(msg) => assert_eq!(msg, "test"),
            _ => panic!("Expected Error state"),
        }
    }

    #[test]
    fn test_reconnect_policy_debug_format() {
        let policy = ReconnectPolicy::default();
        let debug_str = format!("{:?}", policy);
        assert!(debug_str.contains("max_attempts"));
        assert!(debug_str.contains("initial_delay_ms"));
        assert!(debug_str.contains("backoff_multiplier"));
    }

    #[test]
    fn test_connection_state_debug_format() {
        let state = ConnectionState::Connected;
        let debug_str = format!("{:?}", state);
        assert_eq!(debug_str, "Connected");

        let error_state = ConnectionState::Error("connection failed".to_string());
        let error_debug = format!("{:?}", error_state);
        assert!(error_debug.contains("Error"));
    }

    #[test]
    fn test_websocket_message_debug_format() {
        let message = WebSocketMessage::Ping;
        let debug_str = format!("{:?}", message);
        assert_eq!(debug_str, "Ping");

        let text_message = WebSocketMessage::Text("hello".to_string());
        let text_debug = format!("{:?}", text_message);
        assert!(text_debug.contains("Text"));
    }

    #[test]
    fn test_websocket_event_debug_format() {
        let event = WebSocketEvent::Disconnected;
        let debug_str = format!("{:?}", event);
        assert_eq!(debug_str, "Disconnected");

        let error_event = WebSocketEvent::Error("timeout".to_string());
        let error_debug = format!("{:?}", error_event);
        assert!(error_debug.contains("Error"));
    }

    // === Additional Tests ===

    #[test]
    fn test_reconnect_policy_equality() {
        let policy1 = ReconnectPolicy::new(5, 1000, 30000, 2.0);
        let policy2 = ReconnectPolicy::new(5, 1000, 30000, 2.0);
        let policy3 = ReconnectPolicy::new(10, 1000, 30000, 2.0);

        assert_eq!(policy1, policy2);
        assert_ne!(policy1, policy3);
    }

    #[test]
    fn test_connection_state_partial_eq() {
        assert!(ConnectionState::Connected == ConnectionState::Connected);
        assert!(ConnectionState::Connected != ConnectionState::Disconnected);

        let error1 = ConnectionState::Error("error1".to_string());
        let error2 = ConnectionState::Error("error2".to_string());
        let error1_clone = ConnectionState::Error("error1".to_string());

        assert!(error1 == error1_clone);
        assert!(error1 != error2);
    }

    #[test]
    fn test_symbol_in_websocket_message() {
        let symbol = Symbol::new("BTC", "USDT");
        let message = WebSocketMessage::Subscribe(vec![symbol.clone()]);

        match message {
            WebSocketMessage::Subscribe(symbols) => {
                assert_eq!(symbols.len(), 1);
                assert_eq!(symbols[0].base, "BTC");
                assert_eq!(symbols[0].quote, "USDT");
            }
            _ => panic!("Expected Subscribe message"),
        }
    }

    #[test]
    fn test_reconnect_policy_default_values() {
        let policy = ReconnectPolicy::default();
        assert!(policy.max_attempts > 0);
        assert!(policy.initial_delay_ms > 0);
        assert!(policy.max_delay_ms > policy.initial_delay_ms);
        assert!(policy.backoff_multiplier > 1.0);
    }

    #[test]
    fn test_connection_state_from_str() {
        // ConnectionState doesn't implement FromStr, but we can test Display
        let connected = ConnectionState::Connected;
        let connected_str = format!("{}", connected);
        assert!(!connected_str.is_empty());
    }

    #[test]
    fn test_websocket_message_send_and_receive_types() {
        // Test that messages can be used in match expressions
        let messages: Vec<WebSocketMessage> = vec![
            WebSocketMessage::Ping,
            WebSocketMessage::Disconnect,
            WebSocketMessage::Text("test".to_string()),
        ];

        for message in messages {
            match message {
                WebSocketMessage::Ping => {}
                WebSocketMessage::Disconnect => {}
                WebSocketMessage::Text(s) => assert_eq!(s, "test"),
                WebSocketMessage::Subscribe(symbols) => assert!(symbols.is_empty()),
                WebSocketMessage::Unsubscribe(symbols) => assert!(symbols.is_empty()),
            }
        }
    }

    #[test]
    fn test_websocket_event_types() {
        let events: Vec<WebSocketEvent> = vec![
            WebSocketEvent::Connected,
            WebSocketEvent::Disconnected,
            WebSocketEvent::Error("test".to_string()),
        ];

        for event in events {
            match event {
                WebSocketEvent::Connected => {}
                WebSocketEvent::Disconnected => {}
                WebSocketEvent::Error(msg) => assert_eq!(msg, "test"),
                WebSocketEvent::Message(_) => {}
            }
        }
    }

    #[test]
    fn test_connection_state_hash_trait() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn calculate_hash<T: Hash>(t: &T) -> u64 {
            let mut s = DefaultHasher::new();
            t.hash(&mut s);
            s.finish()
        }

        let state1 = ConnectionState::Connected;
        let state2 = ConnectionState::Connected;
        let state3 = ConnectionState::Disconnected;

        assert_eq!(calculate_hash(&state1), calculate_hash(&state2));
        assert_ne!(calculate_hash(&state1), calculate_hash(&state3));
    }

    #[test]
    fn test_reconnect_policy_serialize() {
        let policy = ReconnectPolicy::default();
        let json = serde_json::to_string(&policy);
        assert!(json.is_ok());
        let parsed: ReconnectPolicy = serde_json::from_str(&json.unwrap()).unwrap();
        assert_eq!(parsed.max_attempts, policy.max_attempts);
    }

    #[test]
    fn test_websocket_message_into_text() {
        let message = WebSocketMessage::Text("hello world".to_string());
        if let WebSocketMessage::Text(text) = message {
            assert_eq!(text, "hello world");
        } else {
            panic!("Expected Text message");
        }
    }
}
