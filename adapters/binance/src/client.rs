use std::sync::Arc;
use async_trait::async_trait;
use reqwest::{Client as HttpClient, RequestBuilder};
use tokio::sync::RwLock;
use url::Url;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex::encode as hex_encode;

use crate::error::{BinanceError, Result};
use crate::model::*;
use arbfinder_exchange::prelude::*;

const API_URL: &str = "https://api.binance.com";
const WS_URL: &str = "wss://stream.binance.com:9443/ws";

pub struct BinanceClient {
    http_client: HttpClient,
    api_key: Option<String>,
    api_secret: Option<String>,
    base_url: String,
    ws_url: String,
}

impl BinanceClient {
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

    async fn get(&self, endpoint: &str) -> Result<RequestBuilder> {
        let url = format!("{}{}", self.base_url, endpoint);
        Ok(self.http_client.get(&url))
    }

    fn sign_request(&self, payload: &str) -> Result<String> {
        if let Some(secret) = &self.api_secret {
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
                .map_err(|e| BinanceError::AuthenticationError(e.to_string()))?;
            mac.update(payload.as_bytes());
            Ok(hex_encode(mac.finalize().into_bytes()))
        } else {
            Err(BinanceError::AuthenticationError("API secret not configured".to_string()))
        }
    }

    pub async fn get_server_time(&self) -> Result<i64> {
        let response = self.get("/api/v3/time").await?
            .send()
            .await?
            .json::<ServerTime>()
            .await?;
        Ok(response.server_time)
    }

    pub async fn get_exchange_info(&self) -> Result<ExchangeInformation> {
        let response = self.get("/api/v3/exchangeInfo").await?
            .send()
            .await?
            .json::<ExchangeInformation>()
            .await?;
        Ok(response)
    }

    pub async fn get_orderbook(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let mut url = format!("/api/v3/depth?symbol={}", symbol);
        if let Some(limit) = limit {
            url.push_str(&format!("&limit={}", limit));
        }
        
        let response = self.get(&url).await?
            .send()
            .await?
            .json::<OrderBook>()
            .await?;
        Ok(response)
    }
}

#[async_trait]
impl Exchange for BinanceClient {
    async fn get_markets(&self) -> ExchangeResult<Vec<Market>> {
        let info = self.get_exchange_info().await?;
        
        let markets = info.symbols
            .into_iter()
            .map(|symbol| Market {
                exchange: "binance".to_string(),
                symbol: symbol.symbol,
                base: symbol.base_asset,
                quote: symbol.quote_asset,
            })
            .collect();
            
        Ok(markets)
    }

    async fn get_ticker(&self, market: &str) -> ExchangeResult<Ticker> {
        let orderbook = self.get_orderbook(market, Some(5)).await?;
        
        Ok(Ticker {
            exchange: "binance".to_string(),
            symbol: market.to_string(),
            bid: orderbook.bids.first().map(|p| p[0]).unwrap_or_default(),
            ask: orderbook.asks.first().map(|p| p[0]).unwrap_or_default(),
            last: None, // Binance doesn't provide last price in orderbook
        })
    }

    async fn get_orderbook(&self, market: &str) -> ExchangeResult<OrderBookSnapshot> {
        let orderbook = self.get_orderbook(market, Some(1000)).await?;
        
        Ok(OrderBookSnapshot {
            exchange: "binance".to_string(),
            symbol: market.to_string(),
            sequence: orderbook.last_update_id as i64,
            bids: orderbook.bids.into_iter()
                .map(|level| OrderBookLevel {
                    price: level[0],
                    amount: level[1],
                })
                .collect(),
            asks: orderbook.asks.into_iter()
                .map(|level| OrderBookLevel {
                    price: level[0],
                    amount: level[1],
                })
                .collect(),
        })
    }
}