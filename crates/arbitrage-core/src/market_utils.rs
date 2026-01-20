//! Market utility functions for arbitrage strategies

use crate::strategies::MarketBundle;
use crate::types::{exchange_constants, ExchangeId, Symbol};
use rust_decimal::Decimal;

/// Find the best bid (highest price) across multiple exchanges
pub fn find_best_bid(
    market_data: &MarketBundle,
    symbol: &Symbol,
    exchanges: &[ExchangeId],
) -> Option<(ExchangeId, Decimal)> {
    let mut best: Option<(ExchangeId, Decimal)> = None;
    for &exchange in exchanges {
        if let Some(ticker) = market_data.get_ticker(exchange, symbol) {
            if ticker.bid > Decimal::ZERO {
                if let Some((_, current)) = best {
                    if ticker.bid > current {
                        best = Some((exchange, ticker.bid));
                    }
                } else {
                    best = Some((exchange, ticker.bid));
                }
            }
        }
    }
    best
}

/// Find the best ask (lowest price) across multiple exchanges
pub fn find_best_ask(
    market_data: &MarketBundle,
    symbol: &Symbol,
    exchanges: &[ExchangeId],
) -> Option<(ExchangeId, Decimal)> {
    let mut best: Option<(ExchangeId, Decimal)> = None;
    for &exchange in exchanges {
        if let Some(ticker) = market_data.get_ticker(exchange, symbol) {
            if ticker.ask > Decimal::ZERO {
                if let Some((_, current)) = best {
                    if ticker.ask < current {
                        best = Some((exchange, ticker.ask));
                    }
                } else {
                    best = Some((exchange, ticker.ask));
                }
            }
        }
    }
    best
}

/// Get supported spot exchanges for CEX arbitrage
pub fn get_spot_exchanges() -> Vec<ExchangeId> {
    exchange_constants::SPOT_EXCHANGES.to_vec()
}

/// Get supported perpetual exchanges
pub fn get_perpetual_exchanges() -> Vec<ExchangeId> {
    exchange_constants::PERPETUAL_EXCHANGES.to_vec()
}
