use arbitrage_core::{types::ExchangeId, ArbitrageError, Result};
use rand::Rng;
use reqwest::{Client, StatusCode};
use rustc_hash::FxHashMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};

use crate::rate_limiter::{RateLimitConfig, RateLimiter, UnifiedRateLimitManager};

#[async_trait::async_trait]
pub trait ExchangeRestClient: Send + Sync {
    fn base_url(&self) -> &str;
    async fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T>;
    async fn post<T: DeserializeOwned>(&self, endpoint: &str, body: &Value) -> Result<T>;
    fn rate_limiter(&self) -> Option<&RateLimiter>;
}

#[derive(Debug, Clone)]
pub struct RestClientConfig {
    pub base_url: String,
    pub rate_limit: RateLimitConfig,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub circuit_breaker_threshold: u32,
}

impl Default for RestClientConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            rate_limit: RateLimitConfig::default(),
            timeout_seconds: 30,
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 1000,
            circuit_breaker_threshold: 5,
        }
    }
}

#[derive(Debug)]
pub struct BaseRestClient {
    client: Client,
    base_url: String,
    rate_limiter: Option<RateLimiter>,
    failed_attempts: Mutex<u32>,
    config: RestClientConfig,
}

impl BaseRestClient {
    pub fn new(config: RestClientConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("Failed to build reqwest client");

        let rate_limit = config.rate_limit.clone();
        let rate_limiter = if rate_limit.requests_per_second > 0 {
            Some(RateLimiter::new(rate_limit))
        } else {
            None
        };

        Self {
            client,
            base_url: config.base_url.clone(),
            rate_limiter,
            failed_attempts: Mutex::new(0),
            config,
        }
    }

    pub fn with_rate_limiter(mut self, rate_limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    fn build_url(&self, endpoint: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let endpoint = endpoint.trim_start_matches('/');
        format!("{}/{}", base, endpoint)
    }

    fn is_retriable_error(error: &ArbitrageError) -> bool {
        match error {
            ArbitrageError::Network(_) => true,
            ArbitrageError::Timeout(_) => true,
            ArbitrageError::HttpError(msg) => {
                let status = msg.find("status=").and_then(|i| {
                    let substr = &msg[i + 7..].split_whitespace().next()?;
                    substr.parse::<u16>().ok()
                });
                match status {
                    Some(status) => {
                        status == 500 || status == 502 || status == 503 || status == 504
                    }
                    None => false,
                }
            }
            _ => false,
        }
    }

    async fn record_success(&self) {
        let mut failed = self.failed_attempts.lock().await;
        if *failed > 0 {
            *failed = failed.saturating_sub(1);
        }
    }

    async fn record_failure(&self) {
        let mut failed = self.failed_attempts.lock().await;
        *failed += 1;
    }

    async fn is_circuit_open(&self) -> bool {
        let failed = self.failed_attempts.lock().await;
        *failed >= self.config.circuit_breaker_threshold
    }

    async fn execute_request<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        operation: &str,
        request: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<T> {
        if self.is_circuit_open().await {
            return Err(ArbitrageError::CircuitBreakerOpen(format!(
                "Circuit breaker open for {} - too many consecutive failures",
                operation
            )));
        }

        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await?;
        }

        let url = self.build_url(endpoint);
        let timeout_duration = Duration::from_secs(self.config.timeout_seconds);

        let mut attempt = 0;
        let max_retries = self.config.max_retries;
        let initial_backoff_ms = self.config.initial_backoff_ms;
        let max_backoff_ms = self.config.max_backoff_ms;

        loop {
            attempt += 1;
            let request_builder = request();

            match timeout(timeout_duration, request_builder.send()).await {
                Ok(Ok(response)) => {
                    self.record_success().await;

                    if response.status() == StatusCode::TOO_MANY_REQUESTS {
                        return Err(ArbitrageError::RateLimitExceeded(format!(
                            "Rate limit exceeded for {}",
                            operation
                        )));
                    }

                    if !response.status().is_success() {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        let error = ArbitrageError::HttpError(format!(
                            "{} request to {} failed with status {}: {}",
                            operation, url, status, body
                        ));

                        if attempt > max_retries || !Self::is_retriable_error(&error) {
                            self.record_failure().await;
                            return Err(error);
                        }
                    } else {
                        return response.json().await.map_err(ArbitrageError::from);
                    }
                }
                Ok(Err(e)) => {
                    let error = ArbitrageError::Network(format!(
                        "{} request to {} failed: {}",
                        operation, url, e
                    ));

                    if attempt > max_retries || !Self::is_retriable_error(&error) {
                        self.record_failure().await;
                        return Err(error);
                    }
                }
                Err(_) => {
                    let error = ArbitrageError::Timeout(format!(
                        "{} request to {} timed out after {:?}",
                        operation, url, timeout_duration
                    ));

                    if attempt > max_retries {
                        self.record_failure().await;
                        return Err(error);
                    }
                }
            }

            if attempt <= max_retries {
                let backoff = std::cmp::min(
                    initial_backoff_ms * (2_u64.pow(attempt.saturating_sub(1) as u32)),
                    max_backoff_ms,
                );
                let jitter: u64 = rand::thread_rng().gen_range(0..=100);
                let total_backoff = backoff + jitter;
                sleep(Duration::from_millis(total_backoff)).await;
            }
        }
    }
}

#[async_trait::async_trait]
impl ExchangeRestClient for BaseRestClient {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        self.execute_request(endpoint, "GET", || {
            self.client.get(self.build_url(endpoint))
        })
        .await
    }

    async fn post<T: DeserializeOwned>(&self, endpoint: &str, body: &Value) -> Result<T> {
        self.execute_request(endpoint, "POST", || {
            self.client.post(self.build_url(endpoint)).json(body)
        })
        .await
    }

    fn rate_limiter(&self) -> Option<&RateLimiter> {
        self.rate_limiter.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct RestClientManager {
    clients: FxHashMap<ExchangeId, Arc<BaseRestClient>>,
    manager: UnifiedRateLimitManager,
}

impl RestClientManager {
    pub fn new() -> Self {
        Self {
            clients: FxHashMap::default(),
            manager: UnifiedRateLimitManager::new(),
        }
    }

    pub fn register_exchange(
        &mut self,
        exchange_id: ExchangeId,
        config: RestClientConfig,
    ) -> Arc<BaseRestClient> {
        let client = Arc::new(BaseRestClient::new(config));
        self.clients.insert(exchange_id, client.clone());
        client
    }

    pub fn get_client(&self, exchange_id: ExchangeId) -> Option<&Arc<BaseRestClient>> {
        self.clients.get(&exchange_id)
    }

    pub fn add_limiter(&mut self, key: String, config: RateLimitConfig) {
        self.manager.add_limiter(key, config);
    }

    pub async fn acquire(&self, key: &str) -> Result<()> {
        self.manager.acquire(key).await
    }

    pub fn try_acquire(&self, key: &str) -> Result<()> {
        self.manager.try_acquire(key)
    }
}

pub fn create_rest_client_config(
    _exchange_id: ExchangeId,
    base_url: String,
    requests_per_second: u32,
    burst_capacity: u32,
) -> RestClientConfig {
    RestClientConfig {
        base_url,
        rate_limit: RateLimitConfig {
            requests_per_second,
            burst_capacity,
            window_seconds: 60,
        },
        timeout_seconds: 30,
        max_retries: 3,
        initial_backoff_ms: 100,
        max_backoff_ms: 1000,
        circuit_breaker_threshold: 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_limiter::RateLimitConfig;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_base_rest_client_get() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": "test_value"
            })))
            .mount(&mock_server)
            .await;

        let config = RestClientConfig {
            base_url: mock_server.uri(),
            rate_limit: RateLimitConfig {
                requests_per_second: 10,
                burst_capacity: 20,
                window_seconds: 60,
            },
            timeout_seconds: 30,
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 1000,
            circuit_breaker_threshold: 5,
        };

        let client = BaseRestClient::new(config);
        let result: serde_json::Value = client.get("/api/test").await.unwrap();

        assert_eq!(result["data"], "test_value");
    }

    #[tokio::test]
    async fn test_base_rest_client_post() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": "ok"
            })))
            .mount(&mock_server)
            .await;

        let config = RestClientConfig {
            base_url: mock_server.uri(),
            rate_limit: RateLimitConfig::default(),
            timeout_seconds: 30,
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 1000,
            circuit_breaker_threshold: 5,
        };

        let client = BaseRestClient::new(config);
        let body = serde_json::json!({"key": "value"});
        let result: serde_json::Value = client.post("/api/test", &body).await.unwrap();

        assert_eq!(result["result"], "ok");
    }

    #[tokio::test]
    async fn test_rest_client_manager() {
        let mut manager = RestClientManager::new();

        let config = create_rest_client_config(
            ExchangeId::ByBit,
            "https://api.bybit.com".to_string(),
            60,
            120,
        );

        let client = manager.register_exchange(ExchangeId::ByBit, config);
        assert!(manager.get_client(ExchangeId::ByBit).is_some());
        assert!(manager.get_client(ExchangeId::OKX).is_none());
    }

    #[tokio::test]
    async fn test_rest_client_timeout() {
        let mock_server = MockServer::start().await;

        let mut mock = Mock::given(method("GET"))
            .and(path("/api/test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": "test_value"
            })).set_delay(Duration::from_secs(10)));

        mock.mount(&mock_server).await;

        let config = RestClientConfig {
            base_url: mock_server.uri(),
            rate_limit: RateLimitConfig::default(),
            timeout_seconds: 1,
            max_retries: 1,
            initial_backoff_ms: 100,
            max_backoff_ms: 500,
            circuit_breaker_threshold: 5,
        };

        let client = BaseRestClient::new(config);
        let result: Result<serde_json::Value> = client.get("/api/test").await;
        assert!(result.is_err());
        match result {
            Err(ArbitrageError::Timeout(_)) => {}
            Err(ArbitrageError::Network(_)) => {}
            other => panic!("Expected Timeout or Network error, got: {:?}", other),
        }
    }
}
