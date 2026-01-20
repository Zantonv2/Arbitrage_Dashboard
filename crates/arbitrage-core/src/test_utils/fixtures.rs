use crate::strategies::{ConfidenceFactors, FeeSchedule, FundingRate, TradeLeg};
use crate::types::{ExchangeId, OrderBook, OrderBookLevel, Symbol};
use rust_decimal::Decimal;

pub fn btc_usdt_symbol() -> Symbol {
    Symbol::new("BTC", "USDT")
}

pub fn eth_usdt_symbol() -> Symbol {
    Symbol::new("ETH", "USDT")
}

pub fn sol_usdt_symbol() -> Symbol {
    Symbol::new("SOL", "USDT")
}

pub fn eth_usdc_symbol() -> Symbol {
    Symbol::new("ETH", "USDC")
}

pub fn usdt_usdc_symbol() -> Symbol {
    Symbol::new("USDT", "USDC")
}

pub fn okx_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::OKX,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(2))],
        vec![OrderBookLevel::new(Decimal::from(50005), Decimal::from(1))],
    )
}

pub fn bybit_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50010), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50015), Decimal::from(2))],
    )
}

pub fn mexc_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::MEXC,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(49995), Decimal::from(3))],
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(2))],
    )
}

pub fn gateio_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::GateIo,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50002), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50008), Decimal::from(1))],
    )
}

pub fn valid_btc_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![
            OrderBookLevel::new(Decimal::from(50000), Decimal::from(1)),
            OrderBookLevel::new(Decimal::from(49999), Decimal::from(2)),
        ],
        vec![
            OrderBookLevel::new(Decimal::from(50001), Decimal::from(1)),
            OrderBookLevel::new(Decimal::from(50002), Decimal::from(2)),
        ],
    )
}

pub fn invalid_btc_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50002), Decimal::from(1))],
        vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(1))],
    )
}

pub fn low_liquidity_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(50000), Decimal::from(0_0001))],
        vec![OrderBookLevel::new(Decimal::from(50001), Decimal::from(0_0001))],
    )
}

pub fn wide_spread_order_book() -> OrderBook {
    OrderBook::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        vec![OrderBookLevel::new(Decimal::from(49500), Decimal::from(5))],
        vec![OrderBookLevel::new(Decimal::from(50500), Decimal::from(5))],
    )
}

pub fn funding_rate_okx() -> FundingRate {
    FundingRate::new(
        ExchangeId::OKX,
        btc_usdt_symbol(),
        Decimal::from_str_exact("0.0100").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn funding_rate_bybit() -> FundingRate {
    FundingRate::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        Decimal::from_str_exact("-0.0050").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn high_funding_rate() -> FundingRate {
    FundingRate::new(
        ExchangeId::OKX,
        btc_usdt_symbol(),
        Decimal::from_str_exact("0.0500").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn low_funding_rate() -> FundingRate {
    FundingRate::new(
        ExchangeId::ByBit,
        btc_usdt_symbol(),
        Decimal::from_str_exact("-0.0500").unwrap(),
        chrono::Utc::now() + chrono::Duration::hours(8),
    )
}

pub fn fee_schedule_okx() -> FeeSchedule {
    FeeSchedule::new(ExchangeId::OKX, Decimal::from_str_exact("-0.02").unwrap(), Decimal::from_str_exact("0.05").unwrap())
}

pub fn fee_schedule_bybit() -> FeeSchedule {
    FeeSchedule::new(ExchangeId::ByBit, Decimal::from_str_exact("-0.01").unwrap(), Decimal::from_str_exact("0.06").unwrap())
}

pub fn high_confidence_factors() -> ConfidenceFactors {
    ConfidenceFactors {
        depth_score: Decimal::from(90),
        volatility_score: Decimal::from(20),
        reliability_score: Decimal::from(95),
        spread_stability: Decimal::from(85),
        freshness_score: Decimal::from(100),
    }
}

pub fn low_confidence_factors() -> ConfidenceFactors {
    ConfidenceFactors {
        depth_score: Decimal::from(20),
        volatility_score: Decimal::from(80),
        reliability_score: Decimal::from(30),
        spread_stability: Decimal::from(25),
        freshness_score: Decimal::from(40),
    }
}

pub fn buy_trade_leg(exchange: ExchangeId, price: Decimal, quantity: Decimal) -> TradeLeg {
    TradeLeg::new(exchange, btc_usdt_symbol(), crate::types::Side::Buy, price, quantity)
}

pub fn sell_trade_leg(exchange: ExchangeId, price: Decimal, quantity: Decimal) -> TradeLeg {
    TradeLeg::new(exchange, btc_usdt_symbol(), crate::types::Side::Sell, price, quantity)
}
