#![cfg(test)]

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;

#[test]
fn test_decimal_division_by_zero_handling() {
    let numerator = Decimal::from(100);
    let denominator = Decimal::ZERO;

    let result = numerator.checked_div(denominator);
    assert!(result.is_none(), "Division by zero should return None");
}

#[test]
fn test_decimal_division_normal() {
    let numerator = Decimal::from(100);
    let denominator = Decimal::from(4);

    let result = numerator.checked_div(denominator);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), Decimal::from(25));
}

#[test]
fn test_decimal_multiplication_overflow() {
    let large = Decimal::from_str("99999999999999999999").unwrap();
    let another_large = Decimal::from_str("99999999999999999999").unwrap();

    let result = large.checked_mul(another_large);
    assert!(result.is_none(), "Overflow should return None");
}

#[test]
fn test_decimal_multiplication_normal() {
    let a = Decimal::from(100);
    let b = Decimal::from(25);

    let result = a.checked_mul(b);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), Decimal::from(2500));
}

#[test]
fn test_decimal_to_i32_conversion_normal() {
    let decimal = Decimal::from(42);
    let result = decimal.to_i32();
    assert_eq!(result, Some(42));
}

#[test]
fn test_decimal_to_i32_conversion_negative() {
    let decimal = Decimal::from(-42);
    let result = decimal.to_i32();
    assert_eq!(result, Some(-42));
}

#[test]
fn test_decimal_to_i32_conversion_too_large() {
    let decimal = Decimal::from_str("99999999999999999999").unwrap();
    let result = decimal.to_i32();
    assert!(
        result.is_none(),
        "Conversion of too large number should return None"
    );
}

#[test]
fn test_decimal_to_i32_conversion_fractional() {
    let decimal = Decimal::from_str("42.5").unwrap();
    let result = decimal.to_i32();
    assert_eq!(result, Some(42), "Fractional conversion truncates");
}

#[test]
fn test_decimal_to_f64_normal() {
    let decimal = Decimal::from(42);
    let result = decimal.to_f64();
    assert_eq!(result, Some(42.0));
}

#[test]
fn test_decimal_to_f64_precision_loss() {
    let decimal = Decimal::from_str("0.123456789012345678901234567890").unwrap();
    let result = decimal.to_f64();
    assert!(result.is_some());
    let expected = 0.12345678901234568;
    assert!((result.unwrap() - expected).abs() < 0.0000000000001);
}

#[test]
fn test_profit_calculation_basis_points() {
    let buy_price = Decimal::from(10000);
    let sell_price = Decimal::from(10050);

    let profit_ratio = (sell_price - buy_price).checked_div(buy_price).unwrap();
    assert_eq!(profit_ratio, Decimal::from_str("0.005").unwrap());

    let profit_bps = profit_ratio.checked_mul(Decimal::from(10000)).unwrap();
    assert_eq!(profit_bps, Decimal::from(50));

    let profit_bps_i32 = profit_bps.to_i32().unwrap();
    assert_eq!(profit_bps_i32, 50);
}

#[test]
fn test_profit_calculation_with_fees() {
    let buy_price = Decimal::from(10000);
    let sell_price = Decimal::from(10050);
    let buy_fee_rate = Decimal::from_str("0.001").unwrap();
    let sell_fee_rate = Decimal::from_str("0.001").unwrap();

    let gross_profit = (sell_price - buy_price)
        .checked_div(buy_price)
        .unwrap()
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    let total_fee = buy_fee_rate
        .checked_add(sell_fee_rate)
        .unwrap()
        .checked_mul(Decimal::from(100))
        .unwrap()
        .to_i32()
        .unwrap();

    let net_profit = gross_profit - total_fee;
    assert_eq!(gross_profit, 50);
    assert_eq!(total_fee, 0);
    assert_eq!(net_profit, 50);
}

#[test]
fn test_fee_calculation_in_bps() {
    let buy_fee = Decimal::from_str("0.001").unwrap();
    let sell_fee = Decimal::from_str("0.001").unwrap();

    let total_fee_bps = buy_fee
        .checked_add(sell_fee)
        .unwrap()
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    assert_eq!(total_fee_bps, 20);
}

#[test]
fn test_min_quantity_calculation() {
    let min_notional = Decimal::from(100);
    let price = Decimal::from(25000);

    let min_quantity = min_notional.checked_div(price).unwrap();

    assert_eq!(min_quantity, Decimal::from_str("0.004").unwrap());
}

#[test]
fn test_min_quantity_calculation_division_by_zero() {
    let min_notional = Decimal::from(100);
    let price = Decimal::ZERO;

    let min_quantity = min_notional.checked_div(price);
    assert!(min_quantity.is_none());
}

#[test]
fn test_liquidity_check_below_minimum() {
    let available_liquidity = Decimal::from_str("0.001").unwrap();
    let min_notional = Decimal::from(100);
    let price = Decimal::from(25000);

    let min_quantity = min_notional.checked_div(price).unwrap_or(Decimal::ZERO);

    assert!(
        available_liquidity < min_quantity,
        "Liquidity below minimum"
    );
}

#[test]
fn test_hedge_ratio_calculation() {
    let position_size = Decimal::from(1);
    let hedge_ratio = Decimal::from_str("1.0").unwrap();

    let hedge_size = position_size.checked_mul(hedge_ratio).unwrap();

    assert_eq!(hedge_size, Decimal::from(1));
}

#[test]
fn test_funding_rate_to_bps() {
    let funding_rate = Decimal::from_str("0.0001").unwrap();

    let funding_bps = funding_rate
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    assert_eq!(funding_bps, 1);
}

#[test]
fn test_funding_rate_large_to_bps() {
    let funding_rate = Decimal::from_str("0.01").unwrap();

    let funding_bps = funding_rate
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    assert_eq!(funding_bps, 100);
}

#[test]
fn test_spot_perp_basis_risk() {
    let perp_price = Decimal::from(50000);
    let spot_price = Decimal::from(49900);

    let basis = (perp_price - spot_price).checked_div(spot_price).unwrap();

    let basis_bps = basis
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();
    assert_eq!(basis_bps, 20);
}

#[test]
fn test_basis_risk_zero_spot_price() {
    let perp_price = Decimal::from(50000);
    let spot_price = Decimal::ZERO;

    let basis = (perp_price - spot_price).checked_div(spot_price);
    assert!(basis.is_none());
}

#[test]
fn test_spread_bps_calculation() {
    let bid = Decimal::from(50000);
    let ask = Decimal::from(50050);

    let spread = ask.checked_sub(bid).unwrap();
    let spread_bps = spread
        .checked_div(bid)
        .unwrap()
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    assert_eq!(spread_bps, 10);
}

#[test]
fn test_notional_value_calc() {
    let price = Decimal::from(50000);
    let quantity = Decimal::from_str("0.1").unwrap();

    let notional = price.checked_mul(quantity).unwrap();

    assert_eq!(notional, Decimal::from(5000));
}

#[test]
fn test_notional_value_overflow() {
    let price = Decimal::from_str("99999999999999999999").unwrap();
    let quantity = Decimal::from_str("99999999999999999999").unwrap();

    let notional = price.checked_mul(quantity);
    assert!(notional.is_none());
}

#[test]
fn test_config_decimal_parsing() {
    let value_str = "0.001";
    let decimal = Decimal::from_str_exact(value_str).unwrap();
    assert_eq!(decimal, Decimal::from_str("0.001").unwrap());
}

#[test]
fn test_config_decimal_parsing_invalid() {
    let value_str = "invalid";
    let result = Decimal::from_str_exact(value_str);
    assert!(result.is_err());
}

#[test]
fn test_ratio_distribution_calc() {
    let available = Decimal::from(25);
    let total = Decimal::from(100);

    let ratio = available.checked_div(total).unwrap();
    assert_eq!(ratio, Decimal::from_str("0.25").unwrap());
}

#[test]
fn test_price_deviation_bps() {
    let price = Decimal::from_str("1.005").unwrap();
    let target = Decimal::ONE;

    let deviation = (price - target).checked_div(target).unwrap();
    let deviation_bps = deviation
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    assert_eq!(deviation_bps, 50);
}

#[test]
fn test_negative_deviation_bps() {
    let price = Decimal::from_str("0.995").unwrap();
    let target = Decimal::ONE;

    let deviation = (price - target).checked_div(target).unwrap();
    let deviation_bps = deviation
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    assert_eq!(deviation_bps, -50);
}

#[test]
fn test_net_spread_with_fees() {
    let buy_price = Decimal::from(10000);
    let sell_price = Decimal::from(10050);
    let buy_fee_rate = Decimal::from_str("0.001").unwrap();
    let sell_fee_rate = Decimal::from_str("0.001").unwrap();

    let effective_buy = buy_price * (Decimal::ONE + buy_fee_rate);
    let effective_sell = sell_price * (Decimal::ONE - sell_fee_rate);

    assert!(effective_sell > effective_buy);

    let net_spread = (effective_sell - effective_buy)
        .checked_div(effective_buy)
        .unwrap()
        .checked_mul(Decimal::from(10000))
        .unwrap()
        .to_i32()
        .unwrap();

    assert_eq!(net_spread, 29);
}

#[test]
fn test_zero_price_error() {
    let buy_price = Decimal::ZERO;
    let sell_price = Decimal::from(100);

    let result = sell_price
        .checked_sub(buy_price)
        .and_then(|diff| diff.checked_div(buy_price));

    assert!(result.is_none());
}

#[test]
fn test_negative_price_handling() {
    let buy_price = Decimal::from(-100);
    let sell_price = Decimal::from(100);

    let result = sell_price
        .checked_sub(buy_price)
        .and_then(|diff| diff.checked_div(buy_price));

    assert!(result.is_some());
}

#[test]
fn test_large_decimal_conversion_fails() {
    let huge_decimal = Decimal::from_str("99999999999999999999").unwrap();
    let result = huge_decimal.to_i32();
    assert!(result.is_none());
}
