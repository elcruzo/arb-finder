use std::time::{SystemTime, UNIX_EPOCH};
use async_trait::async_trait;
use reqwest::{Client as HttpClient, RequestBuilder};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::error::{CoinbaseError, Result};
use crate::model::*;
use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;

const API_URL: &str = "https://api.pro.coinbase.com";
const WS_URL: &str = "wss://ws-feed.pro.coinbase.com";

pub struct CoinbaseClient {
    http_client: HttpClient,
    api_key: Option<String>,
    api_secret: Option<String>,
    passphrase: Option<String>,
    base_url: String,
    ws_url: String,
}

impl CoinbaseClient {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
            api_key: None,
            api_secret: None,
            passphrase: None,
            base_url: API_URL.to_string(),
            ws_url: WS_URL.to_string(),
        }
    }

    pub fn with_auth(api_key: String, api_secret: String, passphrase: String) -> Self {
        let mut client = Self::new();
        client.api_key = Some(api_key);
        client.api_secret = Some(api_secret);
        client.passphrase = Some(passphrase);
        client
    }

    fn sign_request(&self, timestamp: &str, method: &str, path: &str, body: &str) -> Result<String> {
        if let Some(secret) = &self.api_secret {
            let message = format!("{}{}{}{}", timestamp, method, path, body);
            let key = BASE64.decode(secret)
                .map_err(|e| CoinbaseError::AuthenticationError(e.to_string()))?;
            
            let mut mac = Hmac::<Sha256>::new_from_slice(&key)
                .map_err(|e| CoinbaseError::AuthenticationError(e.to_string()))?;
            mac.update(message.as_bytes());
            
            Ok(BASE64.encode(mac.finalize().into_bytes()))
        } else {
            Err(CoinbaseError::AuthenticationError("API secret not configured".to_string()))
        }
    }

    async fn get(&self, endpoint: &str) -> Result<RequestBuilder> {
        let url = format!("{}{}", self.base_url, endpoint);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        
        let mut builder = self.http_client.get(&url);
        
        if let (Some(key), Some(passphrase)) = (&self.api_key, &self.passphrase) {
            let signature = self.sign_request(&timestamp, "GET", endpoint, "")?;
            builder = builder
                .header("CB-ACCESS-KEY", key)
                .header("CB-ACCESS-SIGN", signature)
                .header("CB-ACCESS-TIMESTAMP", &timestamp)
                .header("CB-ACCESS-PASSPHRASE", passphrase);
        }
        
        Ok(builder)
    }

    pub async fn get_products(&self) -> Result<Vec<Product>> {
        let response = self.get("/products").await?
            .send()
            .await?
            .json()
            .await?;
        Ok(response)
    }

    pub async fn get_product_orderbook(&self, product_id: &str, level: u8) -> Result<OrderBook> {
        let endpoint = format!("/products/{}/book?level={}", product_id, level);
        let response = self.get(&endpoint).await?
            .send()
            .await?
            .json()
            .await?;
        Ok(response)
    }

    pub async fn get_product_ticker(&self, product_id: &str) -> Result<Ticker> {
        let endpoint = format!("/products/{}/ticker", product_id);
        let response = self.get(&endpoint).await?
            .send()
            .await?
            .json()
            .await?;
        Ok(response)
    }
}

#[async_trait]
impl Exchange for CoinbaseClient {
    async fn get_markets(&self) -> ExchangeResult<Vec<Market>> {
        let products = self.get_products().await
            .map_err(|e| ExchangeError::Other(e.to_string()))?;
        
        let markets = products.into_iter()
            .map(|product| Market {
                exchange: "coinbase".to_string(),
                symbol: product.id,
                base: product.base_currency,
                quote: product.quote_currency,
            })
            .collect();
            
        Ok(markets)
    }

    async fn get_ticker(&self, market: &str) -> ExchangeResult<Ticker> {
        let ticker = self.get_product_ticker(market).await
            .map_err(|e| ExchangeError::Other(e.to_string()))?;
        
        Ok(arbfinder_core::types::Ticker {
            exchange: "coinbase".to_string(),
            symbol: market.to_string(),
            bid: ticker.bid.parse().unwrap_or_default(),
            ask: ticker.ask.parse().unwrap_or_default(),
            last: Some(ticker.price.parse().unwrap_or_default()),
        })
    }

    async fn get_orderbook(&self, market: &str) -> ExchangeResult<OrderBookSnapshot> {
        let orderbook = self.get_product_orderbook(market, 2).await
            .map_err(|e| ExchangeError::Other(e.to_string()))?;
        
        Ok(OrderBookSnapshot {
            exchange: "coinbase".to_string(),
            symbol: market.to_string(),
            sequence: orderbook.sequence as i64,
            bids: orderbook.bids.into_iter()
                .map(|level| OrderBookLevel {
                    price: level[0].parse().unwrap_or_default(),
                    amount: level[1].parse().unwrap_or_default(),
                })
                .collect(),
            asks: orderbook.asks.into_iter()
                .map(|level| OrderBookLevel {
                    price: level[0].parse().unwrap_or_default(),
                    amount: level[1].parse().unwrap_or_default(),
                })
                .collect(),
        })
    }
}