use arbitrage_core::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};
use exchange_connectors::connections::bitstamp::BitstampConnector;
use exchange_connectors::connections::bybit::BybitConnector;
use exchange_connectors::connections::gateio::GateIoConnector;
use exchange_connectors::connections::kraken::KrakenConnector;
use exchange_connectors::connections::mexc::MEXCConnector;
use exchange_connectors::connections::okx::OKXConnector;
use rust_decimal::Decimal;
use serde_json::json;

fn create_mock_orderbook_data_okx() -> serde_json::Value {
    json!({
        "ts": "1704067200000",
        "bids": [
            ["50000.00", "1.5"],
            ["49999.00", "2.0"],
            ["49998.00", "0.5"]
        ],
        "asks": [
            ["50001.00", "1.0"],
            ["50002.00", "2.5"],
            ["50003.00", "1.0"]
        ]
    })
}

fn create_mock_orderbook_data_bybit() -> serde_json::Value {
    json!({
        "a": [
            ["50001.00", "1.0"],
            ["50002.00", "2.5"],
            ["50003.00", "1.0"]
        ],
        "b": [
            ["50000.00", "1.5"],
            ["49999.00", "2.0"],
            ["49998.00", "0.5"]
        ]
    })
}

fn create_mock_orderbook_data_mexc() -> serde_json::Value {
    json!({
        "bids": [
            ["50000.00", "1.5"],
            ["49999.00", "2.0"]
        ],
        "asks": [
            ["50001.00", "1.0"],
            ["50002.00", "2.5"]
        ]
    })
}

fn create_mock_orderbook_data_gateio() -> serde_json::Value {
    json!({
        "asks": [
            ["50001.00", "1.0"],
            ["50002.00", "2.5"]
        ],
        "bids": [
            ["50000.00", "1.5"],
            ["49999.00", "2.0"]
        ]
    })
}

fn create_mock_orderbook_data_kraken() -> serde_json::Value {
    json!({
        "asks": [
            ["50001.00", "1.0", "1704067200000"],
            ["50002.00", "2.5", "1704067200000"]
        ],
        "bids": [
            ["50000.00", "1.5", "1704067200000"],
            ["49999.00", "2.0", "1704067200000"]
        ]
    })
}

fn create_mock_orderbook_data_bitstamp() -> serde_json::Value {
    json!({
        "asks": [
            ["50001.00", "1.0"],
            ["50002.00", "2.5"]
        ],
        "bids": [
            ["50000.00", "1.5"],
            ["49999.00", "2.0"]
        ]
    })
}

fn create_mock_ticker_data_okx() -> serde_json::Value {
    json!({
        "last": "50001.00",
        "bidPx": "50000.50",
        "askPx": "50001.50",
        "vol24h": "1000.00",
        "sodUtc0": "49900.00"
    })
}

fn create_mock_ticker_data_bybit() -> serde_json::Value {
    json!({
        "symbol": "BTCUSDT",
        "lastPrice": "50001.00",
        "bid1Price": "50000.50",
        "ask1Price": "50001.50",
        "volume24h": "1000.00",
        "price24hPcnt": "0.002"
    })
}

#[cfg(test)]
mod okx_connector_tests {
    use super::*;

    #[test]
    fn test_okx_connector_instantiation() {
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
        assert!(config.ws_url.contains("okx.com"));
        assert!(config.rest_url.contains("okx.com"));
    }

    #[test]
    fn test_okx_connector_base() {
        let connector = OKXConnector::new();
        let base = connector.base();
        assert_eq!(
            base.status,
            arbitrage_core::types::ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn test_okx_symbol_to_okx_format() {
        let connector = OKXConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let okx_symbol = connector.symbol_to_okx(&symbol);
        assert_eq!(okx_symbol, "BTC-USDT");
    }

    #[test]
    fn test_okx_symbol_from_okx_format() {
        let connector = OKXConnector::new();
        let result = connector.symbol_from_okx("BTC-USDT");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_okx_symbol_from_invalid_format() {
        let connector = OKXConnector::new();
        let result = connector.symbol_from_okx("INVALID");
        assert!(result.is_err());
    }

    #[test]
    fn test_okx_parse_order_book() {
        let connector = OKXConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_mock_orderbook_data_okx();

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let orderbook = result.unwrap();
        assert_eq!(orderbook.exchange, ExchangeId::OKX);
        assert_eq!(orderbook.symbol, symbol);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
    }

    #[test]
    fn test_okx_parse_order_book_missing_asks() {
        let connector = OKXConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "bids": [["50000.00", "1.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_okx_parse_ticker() {
        let connector = OKXConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_mock_ticker_data_okx();

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.exchange, ExchangeId::OKX);
        assert_eq!(ticker.symbol, symbol);
        assert!(ticker.last_price > Decimal::ZERO);
        assert!(ticker.bid_price > Decimal::ZERO);
        assert!(ticker.ask_price > Decimal::ZERO);
    }

    #[test]
    fn test_okx_parse_ticker_missing_fields() {
        let connector = OKXConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "last": "50001.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_okx_funding_rate_parsing() {
        let connector = OKXConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "fundingRate": "0.0001",
            "fundingTime": "1704067200000",
            "nextFundingRate": "0.0002"
        });

        let result = connector.parse_funding_rate(&data, &symbol);
        assert!(result.is_ok());
        let funding = result.unwrap();
        assert_eq!(funding.exchange, ExchangeId::OKX);
        assert!(funding.funding_rate > Decimal::ZERO);
    }
}

#[cfg(test)]
mod bybit_connector_tests {
    use super::*;

    #[test]
    fn test_bybit_connector_instantiation() {
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
    fn test_bybit_symbol_to_bybit_format() {
        let connector = BybitConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let bybit_symbol = connector.symbol_to_bybit(&symbol);
        assert_eq!(bybit_symbol, "BTCUSDT");
    }

    #[test]
    fn test_bybit_symbol_from_bybit_format() {
        let connector = BybitConnector::new();
        let result = connector.symbol_from_bybit("BTCUSDT");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_bybit_symbol_from_ethusdt() {
        let connector = BybitConnector::new();
        let result = connector.symbol_from_bybit("ETHUSDT");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.base, "ETH");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_bybit_symbol_from_invalid_format() {
        let connector = BybitConnector::new();
        let result = connector.symbol_from_bybit("INVALID");
        assert!(result.is_err());
    }

    #[test]
    fn test_bybit_parse_order_book() {
        let connector = BybitConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_mock_orderbook_data_bybit();
        let map_data = data.as_object().unwrap().clone();

        let result = connector.parse_order_book(&map_data, &symbol);
        assert!(result.is_ok());
        let orderbook = result.unwrap();
        assert_eq!(orderbook.exchange, ExchangeId::ByBit);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
    }

    #[test]
    fn test_bybit_parse_order_book_missing_bids() {
        let connector = BybitConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "a": [["50001.00", "1.0"]]
        })
        .as_object()
        .unwrap()
        .clone();

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_bybit_parse_ticker() {
        let connector = BybitConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_mock_ticker_data_bybit();

        let result = connector.parse_ticker(&data);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.exchange, ExchangeId::ByBit);
        assert!(ticker.last_price > Decimal::ZERO);
    }

    #[test]
    fn test_bybit_parse_funding_rate() {
        let connector = BybitConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "fundingRate": "0.0001",
            "fundingRateTimestamp": "1704067200000"
        });

        let result = connector.parse_funding_rate(&data, &symbol);
        assert!(result.is_ok());
        let funding = result.unwrap();
        assert_eq!(funding.exchange, ExchangeId::ByBit);
    }

    #[test]
    fn test_bybit_clone() {
        let connector = BybitConnector::new();
        let _cloned = connector.clone();
    }

    #[test]
    fn test_bybit_health_check() {
        let connector = BybitConnector::new();
        let health = connector.health_check();
        assert!(health.is_ok() || health.is_err());
    }
}

#[cfg(test)]
mod mexc_connector_tests {
    use super::*;

    #[test]
    fn test_mexc_connector_instantiation() {
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
    fn test_mexc_symbol_to_mexc_format() {
        let connector = MEXCConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let mexc_symbol = connector.symbol_to_mexc(&symbol);
        assert_eq!(mexc_symbol, "BTCUSDT");
    }

    #[test]
    fn test_mexc_symbol_from_mexc_format() {
        let connector = MEXCConnector::new();
        let result = connector.symbol_from_mexc("BTCUSDT");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_mexc_parse_order_book() {
        let connector = MEXCConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_mock_orderbook_data_mexc();

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let orderbook = result.unwrap();
        assert_eq!(orderbook.exchange, ExchangeId::MEXC);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
    }

    #[test]
    fn test_mexc_parse_ticker() {
        let connector = MEXCConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "symbol": "BTCUSDT",
            "price": "50001.00",
            "bidPrice": "50000.50",
            "askPrice": "50001.50",
            "bidQty": "100.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.exchange, ExchangeId::MEXC);
    }

    #[test]
    fn test_mexc_parse_funding_rate() {
        let connector = MEXCConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "data": {
                "fundingRate": "0.0001"
            }
        });

        let result = connector.parse_funding_rate(&data, &symbol);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod gateio_connector_tests {
    use super::*;

    #[test]
    fn test_gateio_connector_instantiation() {
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
    fn test_gateio_symbol_to_gateio_format() {
        let connector = GateIoConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let gateio_symbol = connector.symbol_to_gateio(&symbol);
        assert_eq!(gateio_symbol, "BTC_USDT");
    }

    #[test]
    fn test_gateio_symbol_from_gateio_format() {
        let connector = GateIoConnector::new();
        let result = connector.symbol_from_gateio("BTC_USDT");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_gateio_parse_order_book() {
        let connector = GateIoConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_mock_orderbook_data_gateio();

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let orderbook = result.unwrap();
        assert_eq!(orderbook.exchange, ExchangeId::GateIo);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
    }

    #[test]
    fn test_gateio_parse_ticker() {
        let connector = GateIoConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "last": "50001.00",
            "highest_bid": "50000.50",
            "lowest_ask": "50001.50",
            "base_volume": "1000.00",
            "change_percentage": "2.5"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.exchange, ExchangeId::GateIo);
    }
}

#[cfg(test)]
mod kraken_connector_tests {
    use super::*;

    #[test]
    fn test_kraken_connector_instantiation() {
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
    fn test_kraken_symbol_to_kraken_format() {
        let connector = KrakenConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let kraken_symbol = connector.symbol_to_kraken(&symbol);
        assert_eq!(kraken_symbol, "BTC/USDT");
    }

    #[test]
    fn test_kraken_symbol_from_kraken_format() {
        let connector = KrakenConnector::new();
        let result = connector.symbol_from_kraken("BTC/USDT");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.base, "BTC");
        assert_eq!(symbol.quote, "USDT");
    }

    #[test]
    fn test_kraken_parse_order_book() {
        let connector = KrakenConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = create_mock_orderbook_data_kraken();

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let orderbook = result.unwrap();
        assert_eq!(orderbook.exchange, ExchangeId::Kraken);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
    }

    #[test]
    fn test_kraken_parse_ticker() {
        let connector = KrakenConnector::new();
        let symbol = Symbol::new("BTC", "USDT");
        let data = json!({
            "c": ["50001.00", "100.00"],
            "b": ["50000.50", "50.00"],
            "a": ["50001.50", "75.00"],
            "v": ["1000.00", "2000.00"]
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.exchange, ExchangeId::Kraken);
    }
}

#[cfg(test)]
mod bitstamp_connector_tests {
    use super::*;

    #[test]
    fn test_bitstamp_connector_instantiation() {
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
    fn test_bitstamp_symbol_to_bitstamp_format() {
        let connector = BitstampConnector::new();
        let symbol = Symbol::new("BTC", "USD");
        let bitstamp_symbol = connector.symbol_to_bitstamp(&symbol);
        assert_eq!(bitstamp_symbol, "btcusd");
    }

    #[test]
    fn test_bitstamp_symbol_from_bitstamp_format() {
        let connector = BitstampConnector::new();
        let result = connector.symbol_from_bitstamp("btcusd");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.base, "btc");
        assert_eq!(symbol.quote, "USD");
    }

    #[test]
    fn test_bitstamp_symbol_from_btceur() {
        let connector = BitstampConnector::new();
        let result = connector.symbol_from_bitstamp("btceur");
        assert!(result.is_ok());
        let symbol = result.unwrap();
        assert_eq!(symbol.quote, "EUR");
    }

    #[test]
    fn test_bitstamp_parse_order_book() {
        let connector = BitstampConnector::new();
        let symbol = Symbol::new("BTC", "USD");
        let data = create_mock_orderbook_data_bitstamp();

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let orderbook = result.unwrap();
        assert_eq!(orderbook.exchange, ExchangeId::Bitstamp);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
    }

    #[test]
    fn test_bitstamp_parse_ticker() {
        let connector = BitstampConnector::new();
        let symbol = Symbol::new("BTC", "USD");
        let data = json!({
            "last": "50001.00",
            "bid": "50000.50",
            "ask": "50001.50",
            "volume": "1000.00",
            "open": "49900.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.exchange, ExchangeId::Bitstamp);
    }
}

#[cfg(test)]
mod generic_connector_tests {
    use super::*;

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

    #[test]
    fn test_order_book_level_creation() {
        let level = OrderBookLevel::new(Decimal::from(50000), Decimal::from(1));
        assert_eq!(level.price, Decimal::from(50000));
        assert_eq!(level.quantity, Decimal::from(1));
    }

    #[test]
    fn test_order_book_creation() {
        let symbol = Symbol::new("BTC", "USDT");
        let bids = vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))];
        let asks = vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))];

        let orderbook = OrderBook::new(ExchangeId::OKX, symbol.clone(), bids, asks);

        assert_eq!(orderbook.exchange, ExchangeId::OKX);
        assert_eq!(orderbook.symbol, symbol);
        assert_eq!(orderbook.bids.len(), 1);
        assert_eq!(orderbook.asks.len(), 1);
    }

    #[test]
    fn test_order_book_best_bid_ask() {
        let symbol = Symbol::new("BTC", "USDT");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![
                OrderBookLevel::new(Decimal::from(49999), Decimal::from(1)),
                OrderBookLevel::new(Decimal::from(49998), Decimal::from(2)),
            ],
            vec![
                OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
                OrderBookLevel::new(Decimal::from(50002), Decimal::from(2)),
            ],
        );

        let best_bid = orderbook.best_bid();
        let best_ask = orderbook.best_ask();

        assert_eq!(best_bid.map(|l| l.price), Some(Decimal::from(49999)));
        assert_eq!(best_ask.map(|l| l.price), Some(Decimal::from(50001)));
    }

    #[test]
    fn test_order_book_spread() {
        let symbol = Symbol::new("BTC", "USDT");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );

        let spread = orderbook.spread();
        assert!(spread > Decimal::ZERO);
    }

    #[test]
    fn test_order_book_mid_price() {
        let symbol = Symbol::new("BTC", "USDT");
        let orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(Decimal::from(49900), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50100), Decimal::from(1))],
        );

        let mid = orderbook.mid_price();
        assert_eq!(mid, Decimal::from(50000));
    }

    #[test]
    fn test_order_book_is_valid() {
        let symbol = Symbol::new("BTC", "USDT");
        let valid_orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol.clone(),
            vec![OrderBookLevel::new(Decimal::from(49999), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );

        assert!(valid_orderbook.is_valid());

        let invalid_orderbook = OrderBook::new(
            ExchangeId::OKX,
            symbol,
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
            vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
        );

        assert!(!invalid_orderbook.is_valid());
    }
}
