use arbitrage_core::{types::ExchangeId, ArbitrageError, Result};
use reqwest::{Client, StatusCode};
use rustc_hash::FxHashMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

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
}

impl Default for RestClientConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            rate_limit: RateLimitConfig::default(),
            timeout_seconds: 30,
        }
    }
}

#[derive(Debug)]
pub struct BaseRestClient {
    client: Client,
    base_url: String,
    rate_limiter: Option<RateLimiter>,
}

impl BaseRestClient {
    pub fn new(config: RestClientConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("Failed to build reqwest client");

        let rate_limiter = if config.rate_limit.requests_per_second > 0 {
            Some(RateLimiter::new(config.rate_limit))
        } else {
            None
        };

        Self {
            client,
            base_url: config.base_url,
            rate_limiter,
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

    async fn execute_request<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        operation: &str,
        request: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<T> {
        if let Some(limiter) = &self.rate_limiter {
            limiter.acquire().await?;
        }

        let url = self.build_url(endpoint);
        let request_builder = request();

        let response = request_builder.send().await.map_err(|e| {
            ArbitrageError::Network(format!("{} request to {} failed: {}", operation, url, e))
        })?;

        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(ArbitrageError::RateLimitExceeded(format!(
                "Rate limit exceeded for {}",
                operation
            )));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ArbitrageError::HttpError(format!(
                "{} request to {} failed with status {}: {}",
                operation, url, status, body
            )));
        }

        response.json().await.map_err(ArbitrageError::from)
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
}
