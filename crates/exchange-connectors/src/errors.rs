use arbitrage_core::{types::ExchangeId, ArbitrageError, Result};
use serde_json::Value;
use std::str::FromStr;

pub trait ExchangeErrorMapper: Send + Sync {
    fn exchange_id(&self) -> ExchangeId;

    fn map_error(&self, error: impl std::error::Error, operation: &str) -> ArbitrageError {
        ArbitrageError::Exchange(format!(
            "[{}] {} operation failed: {}",
            self.exchange_id(),
            operation,
            error
        ))
    }

    fn map_network_error(
        &self,
        operation: &str,
        details: impl std::fmt::Display,
    ) -> ArbitrageError {
        ArbitrageError::Network(format!(
            "[{}] {} operation failed: {}",
            self.exchange_id(),
            operation,
            details
        ))
    }

    fn map_http_error(&self, operation: &str, status_code: u16, body: &str) -> ArbitrageError {
        ArbitrageError::HttpError(format!(
            "[{}] {} request failed with status {}: {}",
            self.exchange_id(),
            operation,
            status_code,
            body
        ))
    }

    fn map_parse_error(&self, path: &[&str], value: &Value) -> ArbitrageError {
        ArbitrageError::ParsingError(format!(
            "[{}] Failed to parse at {}: {}",
            self.exchange_id(),
            path.join("."),
            value
        ))
    }

    fn map_missing_field(&self, field: &str, context: &str) -> ArbitrageError {
        ArbitrageError::ParsingError(format!(
            "[{}] Missing required field '{}' in {}",
            self.exchange_id(),
            field,
            context
        ))
    }

    fn map_invalid_value(&self, field: &str, value: &Value, expected: &str) -> ArbitrageError {
        ArbitrageError::ParsingError(format!(
            "[{}] Invalid value for field '{}': got {}, expected {}",
            self.exchange_id(),
            field,
            value,
            expected
        ))
    }

    fn map_api_error(&self, code: i32, message: &str, operation: &str) -> ArbitrageError {
        ArbitrageError::ExchangeApiError {
            code,
            message: format!("[{}] {}: {}", self.exchange_id(), operation, message),
        }
    }

    fn map_auth_error(&self, operation: &str) -> ArbitrageError {
        ArbitrageError::AuthenticationFailed(format!(
            "[{}] Authentication failed for {} operation",
            self.exchange_id(),
            operation
        ))
    }

    fn map_rate_limit_error(&self, operation: &str) -> ArbitrageError {
        ArbitrageError::RateLimitExceeded(format!(
            "[{}] Rate limit exceeded for {} operation",
            self.exchange_id(),
            operation
        ))
    }

    fn map_timeout_error(&self, operation: &str) -> ArbitrageError {
        ArbitrageError::Timeout(format!(
            "[{}] {} operation timed out",
            self.exchange_id(),
            operation
        ))
    }
}

pub trait ResultExt<T> {
    fn map_err_with_context(self, context: &str) -> Result<T>;
}

impl<T> ResultExt<T> for Result<T> {
    fn map_err_with_context(self, context: &str) -> Result<T> {
        self.map_err(|e| ArbitrageError::Exchange(format!("{}: {}", context, e)))
    }
}

pub struct ErrorContext {
    exchange: ExchangeId,
    operation: String,
    endpoint: String,
}

impl ErrorContext {
    pub fn new(
        exchange: ExchangeId,
        operation: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            exchange,
            operation: operation.into(),
            endpoint: endpoint.into(),
        }
    }

    pub fn map_request_error(&self, error: impl std::error::Error) -> ArbitrageError {
        ArbitrageError::Network(format!(
            "[{}] Request failed for {} at {}: {}",
            self.exchange, self.operation, self.endpoint, error
        ))
    }

    pub fn map_response_error(&self, status: u16, body: &str) -> ArbitrageError {
        ArbitrageError::HttpError(format!(
            "[{}] Response error for {} at {}: status={}, body={}",
            self.exchange, self.operation, self.endpoint, status, body
        ))
    }

    pub fn map_parse_error(&self, path: &[&str], value: &Value) -> ArbitrageError {
        ArbitrageError::ParsingError(format!(
            "[{}] Parse error in {} at {} - path: {}, value: {}",
            self.exchange,
            self.operation,
            self.endpoint,
            path.join("."),
            value
        ))
    }
}

#[macro_export]
macro_rules! ensure_field {
    ($value:expr, $field:expr, $context:expr) => {
        $value.as_ref().ok_or_else(|| {
            arbitrage_core::ArbitrageError::ParsingError(format!(
                "Missing required field '{}' in {}",
                $field, $context
            ))
        })?
    };
}

#[macro_export]
macro_rules! parse_field {
    ($value:expr, $field:expr, $parser:expr) => {
        $parser($value).map_err(|_| {
            arbitrage_core::ArbitrageError::ParsingError(format!(
                "Failed to parse field '{}': {}",
                $field, $value
            ))
        })
    };
}

#[macro_export]
macro_rules! parse_optional_field {
    ($value:expr, $field:expr, $parser:expr) => {
        $value.as_ref().and_then(|v| $parser(v).ok())
    };
}

#[macro_export]
macro_rules! extract_result {
    ($response:expr, $result_path:expr) => {
        $response.get($result_path).ok_or_else(|| {
            arbitrage_core::ArbitrageError::ParsingError(format!(
                "Expected '{}' in response",
                $result_path
            ))
        })?
    };
}

#[macro_export]
macro_rules! extract_array {
    ($response:expr, $array_path:expr) => {
        $response
            .get($array_path)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                arbitrage_core::ArbitrageError::ParsingError(format!(
                    "Expected array at '{}' in response",
                    $array_path
                ))
            })?
    };
}

#[macro_export]
macro_rules! exchange_error {
    ($exchange:expr, $operation:expr, $message:expr) => {
        arbitrage_core::ArbitrageError::Exchange(format!(
            "[{}] {}: {}",
            $exchange, $operation, $message
        ))
    };
}

#[macro_export]
macro_rules! exchange_api_error {
    ($exchange:expr, $operation:expr, $code:expr, $message:expr) => {
        arbitrage_core::ArbitrageError::ExchangeApiError {
            code: $code,
            message: format!("[{}] {}: {}", $exchange, $operation, $message),
        }
    };
}

pub fn parse_decimal_value(value: &Value) -> Result<rust_decimal::Decimal> {
    match value {
        Value::Number(n) => {
            let s = n.to_string();
            rust_decimal::Decimal::from_str(&s).map_err(|_| {
                ArbitrageError::ParsingError(format!("Failed to parse decimal: {}", value))
            })
        }
        Value::String(s) => rust_decimal::Decimal::from_str(s).map_err(|_| {
            ArbitrageError::ParsingError(format!("Failed to parse decimal: {}", value))
        }),
        _ => Err(ArbitrageError::ParsingError(format!(
            "Expected number or string for decimal, got: {}",
            value
        ))),
    }
}

pub fn parse_timestamp_value(value: &Value) -> Result<chrono::DateTime<chrono::Utc>> {
    match value {
        Value::Number(n) => {
            let ms = n.as_u64().ok_or_else(|| {
                ArbitrageError::ParsingError(format!("Failed to parse timestamp: {}", value))
            })?;
            chrono::DateTime::from_timestamp_millis(ms as i64).ok_or_else(|| {
                ArbitrageError::ParsingError(format!("Invalid timestamp value: {}", value))
            })
        }
        Value::String(s) => {
            let ms = s.parse::<i64>().map_err(|_| {
                ArbitrageError::ParsingError(format!("Failed to parse timestamp: {}", value))
            })?;
            chrono::DateTime::from_timestamp_millis(ms).ok_or_else(|| {
                ArbitrageError::ParsingError(format!("Invalid timestamp value: {}", value))
            })
        }
        _ => Err(ArbitrageError::ParsingError(format!(
            "Expected number or string for timestamp, got: {}",
            value
        ))),
    }
}

pub fn extract_string_field(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            ArbitrageError::ParsingError(format!("Missing or invalid string field '{}'", field))
        })
}

pub fn extract_opt_string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

pub fn extract_u64_field(value: &Value, field: &str) -> Result<u64> {
    value.get(field).and_then(|v| v.as_u64()).ok_or_else(|| {
        ArbitrageError::ParsingError(format!("Missing or invalid u64 field '{}'", field))
    })
}

pub fn extract_i64_field(value: &Value, field: &str) -> Result<i64> {
    value.get(field).and_then(|v| v.as_i64()).ok_or_else(|| {
        ArbitrageError::ParsingError(format!("Missing or invalid i64 field '{}'", field))
    })
}

pub fn extract_f64_field(value: &Value, field: &str) -> Result<f64> {
    value.get(field).and_then(|v| v.as_f64()).ok_or_else(|| {
        ArbitrageError::ParsingError(format!("Missing or invalid f64 field '{}'", field))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_decimal_value_number() {
        let value = json!(42.5);
        let result = parse_decimal_value(&value);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            rust_decimal::Decimal::from_str("42.5").unwrap()
        );
    }

    #[test]
    fn test_parse_decimal_value_string() {
        let value = json!("42.5");
        let result = parse_decimal_value(&value);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            rust_decimal::Decimal::from_str("42.5").unwrap()
        );
    }

    #[test]
    fn test_extract_string_field() {
        let value = json!({"name": "test"});
        let result = extract_string_field(&value, "name");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test");
    }

    #[test]
    fn test_extract_string_field_missing() {
        let value = json!({});
        let result = extract_string_field(&value, "name");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_opt_string_field() {
        let value = json!({"name": "test", "optional": null});
        assert_eq!(
            extract_opt_string_field(&value, "name"),
            Some("test".to_string())
        );
        assert_eq!(extract_opt_string_field(&value, "optional"), None);
        assert_eq!(extract_opt_string_field(&value, "missing"), None);
    }
}
