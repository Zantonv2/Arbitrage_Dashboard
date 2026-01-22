#[cfg(test)]
mod gateio_connector_tests {
    use super::*;
    use crate::connections::gateio::GateioConnector;
    use arbitrage_core::types::Symbol;
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::str::FromStr;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    #[test]
    fn test_parse_order_book_valid_data() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5"], ["50001.00", "2.0"]],
            "bids": [["49999.00", "1.0"], ["49998.00", "0.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.symbol, symbol);
        assert_eq!(order_book.asks.len(), 2);
        assert_eq!(order_book.bids.len(), 2);
        assert_eq!(
            order_book.asks[0].price,
            Decimal::from_str("50000.00").unwrap()
        );
        assert_eq!(
            order_book.asks[0].quantity,
            Decimal::from_str("1.5").unwrap()
        );
    }

    #[test]
    fn test_parse_order_book_missing_asks() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bids": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_missing_bids() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_empty_arrays() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [],
            "bids": []
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert!(order_book.asks.is_empty());
        assert!(order_book.bids.is_empty());
    }

    #[test]
    fn test_parse_order_book_malformed_entries() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00"], ["50001.00", "2.0", "extra"]],
            "bids": [["49999.00"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.asks.len(), 1);
        assert_eq!(order_book.bids.len(), 0);
    }

    #[test]
    fn test_parse_ticker_valid_data() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "highest_bid": "49999.00",
            "lowest_ask": "50001.00",
            "base_volume": "1000.00",
            "change_percentage": "2.5"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.last_price, Decimal::from_str("50000.00").unwrap());
        assert_eq!(ticker.bid_price, Decimal::from_str("49999.00").unwrap());
        assert_eq!(ticker.ask_price, Decimal::from_str("50001.00").unwrap());
    }

    #[test]
    fn test_parse_ticker_missing_fields() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_invalid_number_format() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "invalid",
            "highest_bid": "49999.00",
            "lowest_ask": "50001.00",
            "base_volume": "1000.00",
            "change_percentage": "2.5"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_symbol_conversion() {
        let connector = GateioConnector::new();
        let symbol = create_test_symbol();

        let gateio_symbol = connector.symbol_to_gateio(&symbol);
        assert_eq!(gateio_symbol, "BTC_USDT");

        let parsed = connector.symbol_from_gateio(&gateio_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }
}

#[cfg(test)]
mod mexc_connector_tests {
    use super::*;
    use crate::connections::mexc::MEXCConnector;
    use arbitrage_core::types::Symbol;
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::str::FromStr;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    #[test]
    fn test_parse_order_book_valid_data() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5"], ["50001.00", "2.0"]],
            "bids": [["49999.00", "1.0"], ["49998.00", "0.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.symbol, symbol);
        assert_eq!(order_book.asks.len(), 2);
        assert_eq!(order_book.bids.len(), 2);
    }

    #[test]
    fn test_parse_order_book_missing_asks() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bids": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_missing_bids() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_empty_data() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [],
            "bids": []
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert!(order_book.asks.is_empty());
        assert!(order_book.bids.is_empty());
    }

    #[test]
    fn test_parse_order_book_malformed_structure() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00"]],
            "bids": [["49999.00", "1.0", "extra"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.asks.len(), 0);
        assert_eq!(order_book.bids.len(), 1);
    }

    #[test]
    fn test_parse_ticker_valid_data() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "price": "50000.00",
            "bidPrice": "49999.00",
            "askPrice": "50001.00",
            "bidQty": "100.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.last_price, Decimal::from_str("50000.00").unwrap());
        assert_eq!(ticker.bid_price, Decimal::from_str("49999.00").unwrap());
        assert_eq!(ticker.ask_price, Decimal::from_str("50001.00").unwrap());
    }

    #[test]
    fn test_parse_ticker_missing_price() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bidPrice": "49999.00",
            "askPrice": "50001.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.last_price, Decimal::ZERO);
    }

    #[test]
    fn test_parse_ticker_zero_values() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "price": "0",
            "bidPrice": "0",
            "askPrice": "0",
            "bidQty": "0"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
    }

    #[test]
    fn test_symbol_conversion() {
        let connector = MEXCConnector::new();
        let symbol = create_test_symbol();

        let mexc_symbol = connector.symbol_to_mexc(&symbol);
        assert_eq!(mexc_symbol, "BTCUSDT");

        let parsed = connector.symbol_from_mexc(&mexc_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }
}

#[cfg(test)]
mod bybit_connector_tests {
    use super::*;
    use crate::connections::bybit::BybitConnector;
    use arbitrage_core::types::Symbol;
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::str::FromStr;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    #[test]
    fn test_parse_order_book_valid_data() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "a": [["50000.00", "1.5"], ["50001.00", "2.0"]],
            "b": [["49999.00", "1.0"], ["49998.00", "0.5"]]
        });

        let result = connector.parse_order_book(data.as_object().unwrap(), &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.symbol, symbol);
        assert_eq!(order_book.asks.len(), 2);
        assert_eq!(order_book.bids.len(), 2);
    }

    #[test]
    fn test_parse_order_book_missing_asks_or_bids() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "a": [],
            "b": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(data.as_object().unwrap(), &symbol);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_order_book_empty_data() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "a": [],
            "b": []
        });

        let result = connector.parse_order_book(data.as_object().unwrap(), &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert!(order_book.asks.is_empty());
        assert!(order_book.bids.is_empty());
    }

    #[test]
    fn test_parse_order_book_with_valid_entries() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "a": [["50001.00", "2.0"]],
            "b": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(data.as_object().unwrap(), &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.asks.len(), 1);
        assert_eq!(order_book.bids.len(), 1);
    }

    #[test]
    fn test_parse_order_book_empty_arrays() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "a": [],
            "b": []
        });

        let result = connector.parse_order_book(data.as_object().unwrap(), &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert!(order_book.asks.is_empty());
        assert!(order_book.bids.is_empty());
    }

    #[test]
    fn test_parse_order_book_with_entries() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "a": [["50001.00", "2.0"]],
            "b": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(data.as_object().unwrap(), &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.asks.len(), 1);
        assert_eq!(order_book.bids.len(), 1);
    }

    #[test]
    fn test_parse_ticker_valid_data() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "symbol": "BTCUSDT",
            "lastPrice": "50000.00",
            "bid1Price": "49999.00",
            "ask1Price": "50001.00",
            "volume24h": "1000.00",
            "price24hPcnt": "2.5"
        });

        let result = connector.parse_ticker(&data);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.last_price, Decimal::from_str("50000.00").unwrap());
        assert_eq!(ticker.bid_price, Decimal::from_str("49999.00").unwrap());
        assert_eq!(ticker.ask_price, Decimal::from_str("50001.00").unwrap());
    }

    #[test]
    fn test_parse_ticker_missing_symbol() {
        let connector = BybitConnector::new();

        let data = json!({
            "lastPrice": "50000.00",
            "bid1Price": "49999.00",
            "ask1Price": "50001.00"
        });

        let result = connector.parse_ticker(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_invalid_numbers() {
        let connector = BybitConnector::new();

        let data = json!({
            "symbol": "BTCUSDT",
            "lastPrice": "not_a_number",
            "bid1Price": "49999.00",
            "ask1Price": "50001.00",
            "volume24h": "1000.00",
            "price24hPcnt": "2.5"
        });

        let result = connector.parse_ticker(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_negative_values() {
        let connector = BybitConnector::new();

        let data = json!({
            "symbol": "BTCUSDT",
            "lastPrice": "50000.00",
            "bid1Price": "49999.00",
            "ask1Price": "50001.00",
            "volume24h": "-100.00",
            "price24hPcnt": "-5.0"
        });

        let result = connector.parse_ticker(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_symbol_conversion() {
        let connector = BybitConnector::new();
        let symbol = create_test_symbol();

        let bybit_symbol = connector.symbol_to_bybit(&symbol);
        assert_eq!(bybit_symbol, "BTCUSDT");

        let parsed = connector.symbol_from_bybit(&bybit_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }
}

#[cfg(test)]
mod kraken_connector_tests {
    use super::*;
    use crate::connections::kraken::KrakenConnector;
    use arbitrage_core::types::Symbol;
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::str::FromStr;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC", "USD")
    }

    #[test]
    fn test_parse_order_book_valid_data() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5", "1580000000"], ["50001.00", "2.0", "1580000001"]],
            "bids": [["49999.00", "1.0", "1580000000"], ["49998.00", "0.5", "1580000001"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.symbol, symbol);
        assert_eq!(order_book.asks.len(), 2);
        assert_eq!(order_book.bids.len(), 2);
    }

    #[test]
    fn test_parse_order_book_missing_asks() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bids": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_missing_bids() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_empty_data() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [],
            "bids": []
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert!(order_book.asks.is_empty());
        assert!(order_book.bids.is_empty());
    }

    #[test]
    fn test_parse_order_book_malformed_entries() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00"], ["50001.00", "2.0", "extra", "more"]],
            "bids": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.asks.len(), 1);
        assert_eq!(order_book.bids.len(), 1);
    }

    #[test]
    fn test_parse_ticker_valid_data() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "c": ["50000.00"],
            "b": ["49999.00"],
            "a": ["50001.00"],
            "v": ["1000.00", "1500.00"]
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.last_price, Decimal::from_str("50000.00").unwrap());
        assert_eq!(ticker.bid_price, Decimal::from_str("49999.00").unwrap());
        assert_eq!(ticker.ask_price, Decimal::from_str("50001.00").unwrap());
    }

    #[test]
    fn test_parse_ticker_missing_last_price() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "b": ["49999.00"],
            "a": ["50001.00"]
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_missing_bid_price() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "c": ["50000.00"],
            "a": ["50001.00"]
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_missing_ask_price() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "c": ["50000.00"],
            "b": ["49999.00"]
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_empty_arrays() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "c": [],
            "b": [],
            "a": []
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_symbol_conversion() {
        let connector = KrakenConnector::new();
        let symbol = create_test_symbol();

        let kraken_symbol = connector.symbol_to_kraken(&symbol);
        assert_eq!(kraken_symbol, "BTC/USD");

        let parsed = connector.symbol_from_kraken(&kraken_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }
}

#[cfg(test)]
mod okx_connector_tests {
    use super::*;
    use crate::connections::okx::OKXConnector;
    use arbitrage_core::types::Symbol;
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::str::FromStr;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC", "USDT")
    }

    #[test]
    fn test_parse_order_book_valid_data() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5", "0"], ["50001.00", "2.0", "0"]],
            "bids": [["49999.00", "1.0", "0"], ["49998.00", "0.5", "0"]],
            "ts": "1609459200000"
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.symbol, symbol);
        assert_eq!(order_book.asks.len(), 2);
        assert_eq!(order_book.bids.len(), 2);
    }

    #[test]
    fn test_parse_order_book_missing_asks() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bids": [["49999.00", "1.0", "0"]],
            "ts": "1609459200000"
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_missing_bids() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5", "0"]],
            "ts": "1609459200000"
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_empty_data() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [],
            "bids": [],
            "ts": "1609459200000"
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_order_book_malformed_structure() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00"], ["50001.00", "2.0"]],
            "bids": [["49999.00", "1.0", "0", "extra"]],
            "ts": "1609459200000"
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.asks.len(), 1);
        assert_eq!(order_book.bids.len(), 1);
    }

    #[test]
    fn test_parse_ticker_valid_data() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "bidPx": "49999.00",
            "askPx": "50001.00",
            "vol24h": "1000.00",
            "sodUtc0": "49000.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.last_price, Decimal::from_str("50000.00").unwrap());
        assert_eq!(ticker.bid_price, Decimal::from_str("49999.00").unwrap());
        assert_eq!(ticker.ask_price, Decimal::from_str("50001.00").unwrap());
    }

    #[test]
    fn test_parse_ticker_missing_last() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bidPx": "49999.00",
            "askPx": "50001.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_missing_bidpx() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "askPx": "50001.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_missing_askpx() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "bidPx": "49999.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_invalid_format() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "not_a_number",
            "bidPx": "49999.00",
            "askPx": "50001.00",
            "vol24h": "1000.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_symbol_conversion() {
        let connector = OKXConnector::new();
        let symbol = create_test_symbol();

        let okx_symbol = connector.symbol_to_okx(&symbol);
        assert_eq!(okx_symbol, "BTC-USDT");

        let parsed = connector.symbol_from_okx(&okx_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }
}

#[cfg(test)]
mod bitstamp_connector_tests {
    use super::*;
    use crate::connections::bitstamp::BitstampConnector;
    use arbitrage_core::types::Symbol;
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::str::FromStr;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC", "USD")
    }

    #[test]
    fn test_parse_order_book_valid_data() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5"], ["50001.00", "2.0"]],
            "bids": [["49999.00", "1.0"], ["49998.00", "0.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.symbol, symbol);
        assert_eq!(order_book.asks.len(), 2);
        assert_eq!(order_book.bids.len(), 2);
    }

    #[test]
    fn test_parse_order_book_missing_asks() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bids": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_missing_bids() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00", "1.5"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_order_book_empty_data() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [],
            "bids": []
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_order_book_malformed_entries() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "asks": [["50000.00"], ["50001.00", "2.0", "extra"]],
            "bids": [["49999.00", "1.0"]]
        });

        let result = connector.parse_order_book(&data, &symbol);
        assert!(result.is_ok());
        let order_book = result.unwrap();
        assert_eq!(order_book.asks.len(), 1);
        assert_eq!(order_book.bids.len(), 1);
    }

    #[test]
    fn test_parse_ticker_valid_data() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "bid": "49999.00",
            "ask": "50001.00",
            "volume": "1000.00",
            "open": "49000.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.last_price, Decimal::from_str("50000.00").unwrap());
        assert_eq!(ticker.bid_price, Decimal::from_str("49999.00").unwrap());
        assert_eq!(ticker.ask_price, Decimal::from_str("50001.00").unwrap());
    }

    #[test]
    fn test_parse_ticker_missing_last() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "bid": "49999.00",
            "ask": "50001.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_missing_bid() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "ask": "50001.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_missing_ask() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "bid": "49999.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ticker_no_open_field() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "50000.00",
            "bid": "49999.00",
            "ask": "50001.00",
            "volume": "1000.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_ok());
        let ticker = result.unwrap();
        assert_eq!(ticker.price_change_24h, Decimal::ZERO);
    }

    #[test]
    fn test_parse_ticker_invalid_numbers() {
        let connector = BitstampConnector::new();
        let symbol = create_test_symbol();

        let data = json!({
            "last": "invalid",
            "bid": "49999.00",
            "ask": "50001.00",
            "volume": "1000.00"
        });

        let result = connector.parse_ticker(&data, &symbol);
        assert!(result.is_err());
    }

    #[test]
    fn test_symbol_conversion_usd() {
        let connector = BitstampConnector::new();
        let symbol = Symbol::new("btc", "USD");

        let bitstamp_symbol = connector.symbol_to_bitstamp(&symbol);
        assert_eq!(bitstamp_symbol, "btcusd");

        let parsed = connector.symbol_from_bitstamp(&bitstamp_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }

    #[test]
    fn test_symbol_conversion_eur() {
        let connector = BitstampConnector::new();
        let symbol = Symbol::new("btc", "EUR");

        let bitstamp_symbol = connector.symbol_to_bitstamp(&symbol);
        assert_eq!(bitstamp_symbol, "btceur");

        let parsed = connector.symbol_from_bitstamp(&bitstamp_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }

    #[test]
    fn test_symbol_conversion_btc() {
        let connector = BitstampConnector::new();
        let symbol = Symbol::new("eth", "BTC");

        let bitstamp_symbol = connector.symbol_to_bitstamp(&symbol);
        assert_eq!(bitstamp_symbol, "ethbtc");

        let parsed = connector.symbol_from_bitstamp(&bitstamp_symbol);
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap(), symbol);
    }

    #[test]
    fn test_symbol_conversion_unknown() {
        let connector = BitstampConnector::new();
        let symbol = Symbol::new("LTC", "USD");

        let parsed = connector.symbol_from_bitstamp("ltcusd");
        assert!(parsed.is_ok());
    }
}
