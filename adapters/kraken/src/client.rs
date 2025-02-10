use std::time::{SystemTime, UNIX_EPOCH};
use async_trait::async_trait;
use reqwest::{Client as HttpClient, RequestBuilder};
use hmac::{Hmac, Mac};
use sha2::{Sha256, Sha512};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::error::{KrakenError, Result};
use crate::model::*;
use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;

const API_URL: &str = "https://api.kraken.com";
const API_VERSION: &str = "0";
const WS_URL: &str = "wss://ws.kraken.com";

pub struct KrakenClient {
    http_client: HttpClient,
    api_key: Option<String>,
    api_secret: Option<String>,
    base_url: String,
    ws_url: String,
}

impl KrakenClient {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
            api_key: None,
            api_secret: None,
            base_url: API_URL.to_string(),
            ws_url: WS_URL.to_string(),
        }
    }

    pub fn with_auth(api_key: String, api_secret: String) -> Self {
        let mut client = Self::new();
        client.api_key = Some(api_key);
        client.api_secret = Some(api_secret);
        client
    }

    fn get_nonce() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string()
    }

    fn sign_message(&self, path: &str, nonce: &str, data: &[u8]) -> Result<String> {
        if let Some(secret) = &self.api_secret {
            let secret = BASE64.decode(secret)
                .map_err(|e| KrakenError::AuthenticationError(e.to_string()))?;

            let mut mac = Hmac::<Sha512>::new_from_slice(&secret)
                .map_err(|e| KrakenError::AuthenticationError(e.to_string()))?;

            let mut message = path.as_bytes().to_vec();
            let mut sha256 = Sha256::new();
            sha256.update(nonce.as_bytes());
            sha256.update(data);
            message.extend_from_slice(&sha256.finalize());

            mac.update(&message);
            Ok(BASE64.encode(mac.finalize().into_bytes()))
        } else {
            Err(KrakenError::AuthenticationError("API secret not configured".to_string()))
        }
    }

    async fn public_get<T>(&self, endpoint: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{}public/{}", self.base_url, API_VERSION, endpoint);
        let response = self.http_client.get(&url)
            .send()
            .await?
            .json::<KrakenResponse<T>>()
            .await?;

        if !response.error.is_empty() {
            return Err(KrakenError::from_api_error(response.error));
        }

        response.result.ok_or_else(|| KrakenError::InvalidResponse("No result in response".to_string()))
    }

    pub async fn get_asset_pairs(&self) -> Result<HashMap<String, AssetPair>> {
        self.public_get("AssetPairs").await
    }

    pub async fn get_ticker(&self, pair: &str) -> Result<HashMap<String, Ticker>> {
        self.public_get(&format!("Ticker?pair={}", pair)).await
    }

    pub async fn get_orderbook(&self, pair: &str, count: Option<u32>) -> Result<HashMap<String, OrderBook>> {
        let mut endpoint = format!("Depth?pair={}", pair);
        if let Some(count) = count {
            endpoint.push_str(&format!("&count={}", count));
        }
        self.public_get(&endpoint).await
    }
}

#[async_trait]
impl Exchange for KrakenClient {
    async fn get_markets(&self) -> ExchangeResult<Vec<Market>> {
        let pairs = self.get_asset_pairs().await
            .map_err(|e| ExchangeError::Other(e.to_string()))?;
        
        let markets = pairs.into_iter()
            .map(|(_, pair)| Market {
                exchange: "kraken".to_string(),
                symbol: pair.altname,
                base: pair.base,
                quote: pair.quote,
            })
            .collect();
            
        Ok(markets)
    }

    async fn get_ticker(&self, market: &str) -> ExchangeResult<Ticker> {
        let response = self.get_ticker(market).await
            .map_err(|e| ExchangeError::Other(e.to_string()))?;
            
        let ticker = response.values().next()
            .ok_or_else(|| ExchangeError::Other("No ticker data found".to_string()))?;
        
        Ok(arbfinder_core::types::Ticker {
            exchange: "kraken".to_string(),
            symbol: market.to_string(),
            bid: ticker.b[0].parse().unwrap_or_default(),
            ask: ticker.a[0].parse().unwrap_or_default(),
            last: Some(ticker.c[0].parse().unwrap_or_default()),
        })
    }

    async fn get_orderbook(&self, market: &str) -> ExchangeResult<OrderBookSnapshot> {
        let response = self.get_orderbook(market, Some(100)).await
            .map_err(|e| ExchangeError::Other(e.to_string()))?;
            
        let orderbook = response.values().next()
            .ok_or_else(|| ExchangeError::Other("No orderbook data found".to_string()))?;
        
        Ok(OrderBookSnapshot {
            exchange: "kraken".to_string(),
            symbol: market.to_string(),
            sequence: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            bids: orderbook.bids.iter()
                .map(|level| OrderBookLevel {
                    price: level[0].parse().unwrap_or_default(),
                    amount: level[1].parse().unwrap_or_default(),
                })
                .collect(),
            asks: orderbook.asks.iter()
                .map(|level| OrderBookLevel {
                    price: level[0].parse().unwrap_or_default(),
                    amount: level[1].parse().unwrap_or_default(),
                })
                .collect(),
        })
    }
}