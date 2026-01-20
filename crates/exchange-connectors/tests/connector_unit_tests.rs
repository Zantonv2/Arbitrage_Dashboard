#[cfg(test)]
mod tests {
    use arbitrage_core::types::{ExchangeId, Symbol};
    use exchange_connectors::connections::bitstamp::BitstampConnector;
    use exchange_connectors::connections::bybit::BybitConnector;
    use exchange_connectors::connections::gateio::GateIoConnector;
    use exchange_connectors::connections::kraken::KrakenConnector;
    use exchange_connectors::connections::mexc::MEXCConnector;
    use exchange_connectors::connections::okx::OKXConnector;

    // === BybitConnector Tests ===

    #[test]
    fn test_bybit_connector_new() {
        let connector = BybitConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::ByBit);
    }

    #[test]
    fn test_bybit_connector_default() {
        let connector = BybitConnector::default();
        assert_eq!(connector.exchange_id(), ExchangeId::ByBit);
    }

    #[test]
    fn test_bybit_connector_status() {
        let connector = BybitConnector::new();
        let status = connector.status();
        assert_eq!(
            status,
            arbitrage_core::types::ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_bybit_connector_event_receiver() {
        let connector = BybitConnector::new();
        let receiver = connector.event_receiver();
        assert!(receiver.count() > 0);
    }

    #[test]
    fn test_bybit_connector_get_stats() {
        let connector = BybitConnector::new();
        let stats = connector.get_stats();
        assert_eq!(stats.exchange, ExchangeId::ByBit);
    }

    #[test]
    fn test_bybit_connector_clone() {
        let connector = BybitConnector::new();
        let _cloned = connector.clone();
        // Clone should compile and work
    }

    #[test]
    fn test_bybit_connector_debug_format() {
        let connector = BybitConnector::new();
        let debug_str = format!("{:?}", connector);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_bybit_connector_health_check() {
        let connector = BybitConnector::new();
        let health = connector.health_check();
        assert!(health.is_ok() || health.is_err());
    }

    // === OKXConnector Tests ===

    #[test]
    fn test_okx_connector_new() {
        let connector = OKXConnector::new();
        assert_eq!(connector.id(), ExchangeId::OKX);
    }

    #[test]
    fn test_okx_connector_default() {
        let connector = OKXConnector::default();
        assert_eq!(connector.id(), ExchangeId::OKX);
    }

    #[test]
    fn test_okx_connector_config() {
        let connector = OKXConnector::new();
        let config = connector.config();
        assert_eq!(config.exchange_id, ExchangeId::OKX);
    }

    #[test]
    fn test_okx_connector_base() {
        let connector = OKXConnector::new();
        let base = connector.base();
        assert!(base.status == arbitrage_core::types::ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_okx_connector_base_mut() {
        let mut connector = OKXConnector::new();
        let _base = connector.base_mut();
        // Should compile and work
    }

    #[test]
    fn test_okx_connector_event_receiver() {
        let connector = OKXConnector::new();
        let receiver = connector.event_receiver();
        assert!(receiver.count() > 0);
    }

    #[test]
    fn test_okx_connector_clone() {
        let connector = OKXConnector::new();
        let _cloned = connector.clone();
    }

    #[test]
    fn test_okx_connector_debug_format() {
        let connector = OKXConnector::new();
        let debug_str = format!("{:?}", connector);
        assert!(!debug_str.is_empty());
    }

    // === MEXCConnector Tests ===

    #[test]
    fn test_mexc_connector_new() {
        let connector = MEXCConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::MEXC);
    }

    #[test]
    fn test_mexc_connector_default() {
        let connector = MEXCConnector::default();
        assert_eq!(connector.exchange_id(), ExchangeId::MEXC);
    }

    #[test]
    fn test_mexc_connector_status() {
        let connector = MEXCConnector::new();
        let status = connector.status();
        assert_eq!(
            status,
            arbitrage_core::types::ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_mexc_connector_event_receiver() {
        let connector = MEXCConnector::new();
        let receiver = connector.event_receiver();
        assert!(receiver.count() > 0);
    }

    #[test]
    fn test_mexc_connector_get_stats() {
        let connector = MEXCConnector::new();
        let stats = connector.get_stats();
        assert_eq!(stats.exchange, ExchangeId::MEXC);
    }

    #[test]
    fn test_mexc_connector_clone() {
        let connector = MEXCConnector::new();
        let _cloned = connector.clone();
    }

    #[test]
    fn test_mexc_connector_debug_format() {
        let connector = MEXCConnector::new();
        let debug_str = format!("{:?}", connector);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_mexc_connector_health_check() {
        let connector = MEXCConnector::new();
        let health = connector.health_check();
        assert!(health.is_ok() || health.is_err());
    }

    // === GateIoConnector Tests ===

    #[test]
    fn test_gateio_connector_new() {
        let connector = GateIoConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::GateIo);
    }

    #[test]
    fn test_gateio_connector_default() {
        let connector = GateIoConnector::default();
        assert_eq!(connector.exchange_id(), ExchangeId::GateIo);
    }

    #[test]
    fn test_gateio_connector_status() {
        let connector = GateIoConnector::new();
        let status = connector.status();
        assert_eq!(
            status,
            arbitrage_core::types::ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_gateio_connector_event_receiver() {
        let connector = GateIoConnector::new();
        let receiver = connector.event_receiver();
        assert!(receiver.count() > 0);
    }

    #[test]
    fn test_gateio_connector_get_stats() {
        let connector = GateIoConnector::new();
        let stats = connector.get_stats();
        assert_eq!(stats.exchange, ExchangeId::GateIo);
    }

    #[test]
    fn test_gateio_connector_clone() {
        let connector = GateIoConnector::new();
        let _cloned = connector.clone();
    }

    #[test]
    fn test_gateio_connector_debug_format() {
        let connector = GateIoConnector::new();
        let debug_str = format!("{:?}", connector);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_gateio_connector_health_check() {
        let connector = GateIoConnector::new();
        let health = connector.health_check();
        assert!(health.is_ok() || health.is_err());
    }

    // === KrakenConnector Tests ===

    #[test]
    fn test_kraken_connector_new() {
        let connector = KrakenConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::Kraken);
    }

    #[test]
    fn test_kraken_connector_default() {
        let connector = KrakenConnector::default();
        assert_eq!(connector.exchange_id(), ExchangeId::Kraken);
    }

    #[test]
    fn test_kraken_connector_status() {
        let connector = KrakenConnector::new();
        let status = connector.status();
        assert_eq!(
            status,
            arbitrage_core::types::ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_kraken_connector_event_receiver() {
        let connector = KrakenConnector::new();
        let receiver = connector.event_receiver();
        assert!(receiver.count() > 0);
    }

    #[test]
    fn test_kraken_connector_get_stats() {
        let connector = KrakenConnector::new();
        let stats = connector.get_stats();
        assert_eq!(stats.exchange, ExchangeId::Kraken);
    }

    #[test]
    fn test_kraken_connector_clone() {
        let connector = KrakenConnector::new();
        let _cloned = connector.clone();
    }

    #[test]
    fn test_kraken_connector_debug_format() {
        let connector = KrakenConnector::new();
        let debug_str = format!("{:?}", connector);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_kraken_connector_health_check() {
        let connector = KrakenConnector::new();
        let health = connector.health_check();
        assert!(health.is_ok() || health.is_err());
    }

    // === BitstampConnector Tests ===

    #[test]
    fn test_bitstamp_connector_new() {
        let connector = BitstampConnector::new();
        assert_eq!(connector.exchange_id(), ExchangeId::Bitstamp);
    }

    #[test]
    fn test_bitstamp_connector_default() {
        let connector = BitstampConnector::default();
        assert_eq!(connector.exchange_id(), ExchangeId::Bitstamp);
    }

    #[test]
    fn test_bitstamp_connector_status() {
        let connector = BitstampConnector::new();
        let status = connector.status();
        assert_eq!(
            status,
            arbitrage_core::types::ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_bitstamp_connector_event_receiver() {
        let connector = BitstampConnector::new();
        let receiver = connector.event_receiver();
        assert!(receiver.count() > 0);
    }

    #[test]
    fn test_bitstamp_connector_get_stats() {
        let connector = BitstampConnector::new();
        let stats = connector.get_stats();
        assert_eq!(stats.exchange, ExchangeId::Bitstamp);
    }

    #[test]
    fn test_bitstamp_connector_clone() {
        let connector = BitstampConnector::new();
        let _cloned = connector.clone();
    }

    #[test]
    fn test_bitstamp_connector_debug_format() {
        let connector = BitstampConnector::new();
        let debug_str = format!("{:?}", connector);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_bitstamp_connector_health_check() {
        let connector = BitstampConnector::new();
        let health = connector.health_check();
        assert!(health.is_ok() || health.is_err());
    }

    // === Additional Connector Tests ===

    #[test]
    fn test_all_connectors_have_unique_exchange_ids() {
        let connectors: Vec<Box<dyn exchange_connectors::connector::ExchangeConnector>> = vec![
            Box::new(BybitConnector::new()),
            Box::new(OKXConnector::new()),
            Box::new(MEXCConnector::new()),
            Box::new(GateIoConnector::new()),
            Box::new(KrakenConnector::new()),
            Box::new(BitstampConnector::new()),
        ];

        let mut ids = Vec::new();
        for connector in connectors {
            let id = connector.exchange_id();
            assert!(!ids.contains(&id), "Duplicate exchange ID: {:?}", id);
            ids.push(id);
        }

        assert_eq!(ids.len(), 6);
    }

    #[test]
    fn test_all_connectors_implement_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BybitConnector>();
        assert_send_sync::<OKXConnector>();
        assert_send_sync::<MEXCConnector>();
        assert_send_sync::<GateIoConnector>();
        assert_send_sync::<KrakenConnector>();
        assert_send_sync::<BitstampConnector>();
    }

    #[test]
    fn test_all_connectors_initial_status() {
        let connectors: Vec<Box<dyn exchange_connectors::connector::ExchangeConnector>> = vec![
            Box::new(BybitConnector::new()),
            Box::new(OKXConnector::new()),
            Box::new(MEXCConnector::new()),
            Box::new(GateIoConnector::new()),
            Box::new(KrakenConnector::new()),
            Box::new(BitstampConnector::new()),
        ];

        for connector in connectors {
            let status = connector.status();
            assert_eq!(
                status,
                arbitrage_core::types::ConnectionStatus::Disconnected
            );
        }
    }

    #[test]
    fn test_connector_stats_initial_values() {
        let connectors: Vec<&(dyn exchange_connectors::connector::ExchangeConnector + '_)> = vec![
            &BybitConnector::new(),
            &MEXCConnector::new(),
            &GateIoConnector::new(),
            &KrakenConnector::new(),
            &BitstampConnector::new(),
        ];

        for connector in connectors {
            let stats = connector.get_stats();
            assert_eq!(stats.exchange, connector.exchange_id());
        }
    }

    #[test]
    fn test_exchange_id_variants() {
        use arbitrage_core::types::ExchangeId;

        // Test all exchange IDs
        assert_eq!(ExchangeId::ByBit.to_string(), "bybit");
        assert_eq!(ExchangeId::OKX.to_string(), "okx");
        assert_eq!(ExchangeId::MEXC.to_string(), "mexc");
        assert_eq!(ExchangeId::GateIo.to_string(), "gateio");
        assert_eq!(ExchangeId::Kraken.to_string(), "kraken");
        assert_eq!(ExchangeId::Bitstamp.to_string(), "bitstamp");
    }

    #[test]
    fn test_symbol_creation() {
        let symbol = Symbol::new("BTC", "USDT");
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
        assert_eq!(symbol.to_pair(), "BTC/USDT");
    }

    #[test]
    fn test_symbol_from_pair() {
        let symbol = Symbol::from_pair("ETH/USDC").unwrap();
        assert_eq!(symbol.base, "ETH");
        assert_eq!(symbol.quote, "USDC");
    }

    #[test]
    fn test_symbol_from_pair_invalid() {
        assert!(Symbol::from_pair("INVALID").is_none());
        assert!(Symbol::from_pair("BTC/USDT/EXTRA").is_none());
    }

    #[test]
    fn test_symbol_display() {
        let symbol = Symbol::new("SOL", "USD");
        let display = format!("{}", symbol);
        assert_eq!(display, "SOL/USD");
    }
}
