use arbfinder_core::{ArbFinderError, Result};
use async_trait::async_trait;
use reqwest::{Client, Method, Response};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, warn};
use url::Url;

use crate::rate_limiter::RateLimiter;
use crate::traits::{ExchangeConfig, RestClient};

#[derive(Debug)]
pub struct RestClientImpl {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    secret_key: Option<String>,
    passphrase: Option<String>,
    rate_limiter: RateLimiter,
    request_timeout: Duration,
}

impl RestClientImpl {
    pub fn new<C: ExchangeConfig>(config: &C) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms()))
            .build()
            .map_err(|e| ArbFinderError::Http(e))?;

        let rate_limiter = RateLimiter::new(
            config.rate_limit_requests_per_second(),
            Duration::from_secs(1),
        );

        Ok(Self {
            client,
            base_url: config.base_url().to_string(),
            api_key: config.api_key().map(|s| s.to_string()),
            secret_key: config.secret_key().map(|s| s.to_string()),
            passphrase: config.passphrase().map(|s| s.to_string()),
            rate_limiter,
            request_timeout: Duration::from_millis(config.request_timeout_ms()),
        })
    }

    pub async fn request(
        &self,
        method: Method,
        endpoint: &str,
        params: Option<&HashMap<String, String>>,
        body: Option<&Value>,
        signed: bool,
    ) -> Result<Value> {
        // Apply rate limiting
        self.rate_limiter.acquire().await;

        let url = self.build_url(endpoint, params)?;
        debug!("Making {} request to: {}", method, url);

        let mut request = self.client.request(method.clone(), &url);

        // Add authentication headers if signed
        if signed {
            let timestamp = chrono::Utc::now().timestamp_millis() as u64;
            let auth_headers = self.build_auth_headers(&method, endpoint, params, body, timestamp)?;
            
            for (key, value) in auth_headers {
                request = request.header(key, value);
            }
        }

        // Add body if present
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ArbFinderError::Http(e))?;

        self.handle_response(response).await
    }

    fn build_url(&self, endpoint: &str, params: Option<&HashMap<String, String>>) -> Result<String> {
        let mut url = Url::parse(&format!("{}{}", self.base_url, endpoint))
            .map_err(|e| ArbFinderError::InvalidData(format!("Invalid URL: {}", e)))?;

        if let Some(params) = params {
            for (key, value) in params {
                url.query_pairs_mut().append_pair(key, value);
            }
        }

        Ok(url.to_string())
    }

    fn build_auth_headers(
        &self,
        method: &Method,
        endpoint: &str,
        params: Option<&HashMap<String, String>>,
        body: Option<&Value>,
        timestamp: u64,
    ) -> Result<HashMap<String, String>> {
        let mut headers = HashMap::new();

        if let Some(api_key) = &self.api_key {
            headers.insert("X-API-KEY".to_string(), api_key.clone());
        }

        if let Some(passphrase) = &self.passphrase {
            headers.insert("X-PASSPHRASE".to_string(), passphrase.clone());
        }

        headers.insert("X-TIMESTAMP".to_string(), timestamp.to_string());

        // Build query string for signature
        let query_string = if let Some(params) = params {
            let mut pairs: Vec<String> = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            pairs.sort();
            pairs.join("&")
        } else {
            String::new()
        };

        // Build body string for signature
        let body_string = if let Some(body) = body {
            serde_json::to_string(body)
                .map_err(|e| ArbFinderError::Json(e))?
        } else {
            String::new()
        };

        let signature = self.sign_request(
            method.as_str(),
            endpoint,
            &format!("{}{}", query_string, body_string),
            timestamp,
        )?;

        headers.insert("X-SIGNATURE".to_string(), signature);

        Ok(headers)
    }

    async fn handle_response(&self, response: Response) -> Result<Value> {
        let status = response.status();
        let headers = response.headers().clone();

        debug!("Response status: {}", status);

        // Handle rate limiting
        if status == 429 {
            warn!("Rate limit exceeded, waiting before retry");
            
            // Extract retry-after header if present
            let retry_after = headers
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1);

            sleep(Duration::from_secs(retry_after)).await;
            return Err(ArbFinderError::RateLimit("Rate limit exceeded".to_string()));
        }

        // Check for other HTTP errors
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("HTTP error {}: {}", status, error_text);
            return Err(ArbFinderError::Exchange(format!("HTTP error {}: {}", status, error_text)));
        }

        // Parse JSON response
        let text = response.text().await.map_err(|e| ArbFinderError::Http(e))?;
        debug!("Response body: {}", text);

        serde_json::from_str(&text).map_err(|e| ArbFinderError::Json(e))
    }

    pub async fn get_with_retry(
        &self,
        endpoint: &str,
        params: Option<&HashMap<String, String>>,
        max_retries: u32,
    ) -> Result<Value> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < max_retries {
            match self.get(endpoint, params).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempts += 1;
                    last_error = Some(e);
                    
                    if attempts < max_retries {
                        let delay = Duration::from_millis(1000 * attempts as u64);
                        warn!("Request failed, retrying in {:?}", delay);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ArbFinderError::Internal("Max retries exceeded".to_string())
        }))
    }

    pub async fn post_with_retry(
        &self,
        endpoint: &str,
        body: Option<&Value>,
        max_retries: u32,
    ) -> Result<Value> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < max_retries {
            match self.post(endpoint, body).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempts += 1;
                    last_error = Some(e);
                    
                    if attempts < max_retries {
                        let delay = Duration::from_millis(1000 * attempts as u64);
                        warn!("Request failed, retrying in {:?}", delay);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ArbFinderError::Internal("Max retries exceeded".to_string())
        }))
    }
}

#[async_trait]
impl RestClient for RestClientImpl {
    async fn get(&self, endpoint: &str, params: Option<&HashMap<String, String>>) -> Result<Value> {
        self.request(Method::GET, endpoint, params, None, true).await
    }

    async fn post(&self, endpoint: &str, body: Option<&Value>) -> Result<Value> {
        self.request(Method::POST, endpoint, None, body, true).await
    }

    async fn put(&self, endpoint: &str, body: Option<&Value>) -> Result<Value> {
        self.request(Method::PUT, endpoint, None, body, true).await
    }

    async fn delete(&self, endpoint: &str, params: Option<&HashMap<String, String>>) -> Result<Value> {
        self.request(Method::DELETE, endpoint, params, None, true).await
    }

    fn sign_request(&self, method: &str, endpoint: &str, params: &str, timestamp: u64) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let secret_key = self.secret_key.as_ref()
            .ok_or_else(|| ArbFinderError::Authentication("Secret key not configured".to_string()))?;

        // Create the string to sign
        let string_to_sign = format!("{}{}{}{}", timestamp, method.to_uppercase(), endpoint, params);

        // Create HMAC
        let mut mac = Hmac::<Sha256>::new_from_slice(secret_key.as_bytes())
            .map_err(|e| ArbFinderError::Authentication(format!("Invalid secret key: {}", e)))?;
        
        mac.update(string_to_sign.as_bytes());
        let signature = mac.finalize().into_bytes();

        // Return hex-encoded signature
        Ok(hex::encode(signature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::DefaultExchangeConfig;

    #[tokio::test]
    async fn test_build_url() {
        let config = DefaultExchangeConfig {
            base_url: "https://api.example.com".to_string(),
            ..Default::default()
        };
        
        let client = RestClientImpl::new(&config).unwrap();
        
        let url = client.build_url("/test", None).unwrap();
        assert_eq!(url, "https://api.example.com/test");
        
        let mut params = HashMap::new();
        params.insert("key1".to_string(), "value1".to_string());
        params.insert("key2".to_string(), "value2".to_string());
        
        let url_with_params = client.build_url("/test", Some(&params)).unwrap();
        assert!(url_with_params.contains("key1=value1"));
        assert!(url_with_params.contains("key2=value2"));
    }

    #[test]
    fn test_sign_request() {
        let config = DefaultExchangeConfig {
            secret_key: Some("test_secret".to_string()),
            ..Default::default()
        };
        
        let client = RestClientImpl::new(&config).unwrap();
        
        let signature = client.sign_request("GET", "/test", "", 1234567890).unwrap();
        assert!(!signature.is_empty());
        assert_eq!(signature.len(), 64); // SHA256 hex string length
    }
}