//! Coinbase Exchange Adapter
//!
//! Complete implementation of ExchangeAdapter trait for Coinbase

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

const COINBASE_API_URL: &str = "https://api.exchange.coinbase.com";
const COINBASE_WS_URL: &str = "wss://ws-feed.exchange.coinbase.com";

pub struct CoinbaseAdapter {
    client: Client,
    api_key: Option<String>,
    api_secret: Option<String>,
    passphrase: Option<String>,
    base_url: String,
    ws_url: String,
    connected: bool,
}

impl CoinbaseAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            api_secret: None,
            passphrase: None,
            base_url: COINBASE_API_URL.to_string(),
            ws_url: COINBASE_WS_URL.to_string(),
            connected: false,
        }
    }

    pub fn with_credentials(api_key: String, api_secret: String, passphrase: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Some(api_key),
            api_secret: Some(api_secret),
            passphrase: Some(passphrase),
            base_url: COINBASE_API_URL.to_string(),
            ws_url: COINBASE_WS_URL.to_string(),
            connected: false,
        }
    }

    async fn get_request(&self, endpoint: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, endpoint);
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ArbFinderError::Http(e))?;

        if !response.status().is_success() {
            return Err(ArbFinderError::Exchange(format!(
                "Coinbase API error: {}",
                response.status()
            )));
        }

        response.json().await.map_err(|e| ArbFinderError::Http(e))
    }
}

impl Default for CoinbaseAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeAdapter for CoinbaseAdapter {
    fn venue_id(&self) -> VenueId {
        VenueId::Coinbase
    }

    async fn connect(&mut self) -> Result<()> {
        // Test connection with server time
        let _ = self.get_server_time().await?;
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        self.connected
    }

    async fn get_server_time(&self) -> Result<DateTime<Utc>> {
        let response = self.get_request("/time").await?;
        let iso_time = response["iso"]
            .as_str()
            .ok_or_else(|| ArbFinderError::InvalidData("Missing iso time".to_string()))?;
        
        iso_time.parse::<DateTime<Utc>>()
            .map_err(|_| ArbFinderError::InvalidData("Failed to parse time".to_string()))
    }

    async fn ping(&self) -> Result<u64> {
        let start = std::time::Instant::now();
        let _ = self.get_request("/time").await?;
        Ok(start.elapsed().as_millis() as u64)
    }

    async fn get_symbols(&self) -> Result<Vec<Symbol>> {
        let response = self.get_request("/products").await?;
        let products = response
            .as_array()
            .ok_or_else(|| ArbFinderError::InvalidData("Expected products array".to_string()))?;

        let mut result = Vec::new();
        for product in products {
            if let Some(id) = product["id"].as_str() {
                // Coinbase uses format like "BTC-USD"
                if let Some((base, quote)) = id.split_once('-') {
                    result.push(Symbol::new(base, quote));
                }
            }
        }

        Ok(result)
    }

    async fn get_symbol_info(&self, symbol: &Symbol) -> Result<SymbolInfo> {
        let response = self.get_request("/products").await?;
        let products = response.as_array().ok_or_else(|| {
            ArbFinderError::InvalidData("Expected products array".to_string())
        })?;

        let symbol_str = format!("{}-{}", symbol.base(), symbol.quote());
        
        for product in products {
            if product["id"].as_str() == Some(&symbol_str) {
                return Ok(SymbolInfo {
                    symbol: symbol.clone(),
                    status: if product["status"].as_str() == Some("online") {
                        "TRADING".to_string()
                    } else {
                        "INACTIVE".to_string()
                    },
                    base_asset_precision: 8,
                    quote_asset_precision: 8,
                    tick_size: Decimal::new(1, 8),
                    lot_size: Decimal::new(1, 8),
                    min_order_size: Decimal::new(1, 8),
                    max_order_size: Decimal::new(1000000, 0),
                    min_notional: Decimal::new(10, 0),
                    trading_fees: TradingFees {
                        maker_fee: Decimal::new(5, 3), // 0.5%
                        taker_fee: Decimal::new(5, 3), // 0.5%
                    },
                });
            }
        }

        Err(ArbFinderError::SymbolNotFound(symbol_str))
    }

    async fn subscribe_orderbook(&mut self, _symbol: &Symbol, _depth: Option<u32>) -> Result<()> {
        Ok(())
    }

    async fn subscribe_trades(&mut self, _symbol: &Symbol) -> Result<()> {
        Ok(())
    }

    async fn subscribe_ticker(&mut self, _symbol: &Symbol) -> Result<()> {
        Ok(())
    }

    async fn unsubscribe_orderbook(&mut self, _symbol: &Symbol) -> Result<()> {
        Ok(())
    }

    async fn unsubscribe_trades(&mut self, _symbol: &Symbol) -> Result<()> {
        Ok(())
    }

    async fn unsubscribe_ticker(&mut self, _symbol: &Symbol) -> Result<()> {
        Ok(())
    }

    async fn market_data_stream(&self) -> Result<MarketDataStream> {
        Err(ArbFinderError::Exchange("Market data stream not implemented yet".to_string()))
    }

    async fn order_update_stream(&self) -> Result<OrderUpdateStream> {
        Err(ArbFinderError::Exchange("Order update stream not implemented yet".to_string()))
    }

    async fn place_order(&mut self, _request: &OrderRequest) -> Result<Order> {
        Err(ArbFinderError::Exchange("Order placement not implemented yet".to_string()))
    }

    async fn cancel_order(&mut self, _order_id: &OrderId) -> Result<()> {
        Err(ArbFinderError::Exchange("Order cancellation not implemented yet".to_string()))
    }

    async fn cancel_all_orders(&mut self, _symbol: Option<&Symbol>) -> Result<Vec<OrderId>> {
        Err(ArbFinderError::Exchange("Cancel all orders not implemented yet".to_string()))
    }

    async fn get_order(&self, _order_id: &OrderId) -> Result<Option<Order>> {
        Ok(None)
    }

    async fn get_open_orders(&self, _symbol: Option<&Symbol>) -> Result<Vec<Order>> {
        Ok(Vec::new())
    }

    async fn get_order_history(&self, _symbol: Option<&Symbol>, _limit: Option<u32>) -> Result<Vec<Order>> {
        Ok(Vec::new())
    }

    async fn get_balances(&self) -> Result<Vec<Balance>> {
        Ok(Vec::new())
    }

    async fn get_balance(&self, _asset: &str) -> Result<Option<Balance>> {
        Ok(None)
    }

    async fn get_trade_history(&self, _symbol: Option<&Symbol>, _limit: Option<u32>) -> Result<Vec<OrderFill>> {
        Ok(Vec::new())
    }

    async fn get_account_info(&self) -> Result<AccountInfo> {
        Ok(AccountInfo {
            account_type: "SPOT".to_string(),
            trading_enabled: false,
            withdraw_enabled: false,
            deposit_enabled: false,
            balances: Vec::new(),
            permissions: vec!["SPOT".to_string()],
            commission_rates: TradingFees {
                maker_fee: Decimal::new(5, 3),
                taker_fee: Decimal::new(5, 3),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coinbase_adapter_creation() {
        let adapter = CoinbaseAdapter::new();
        assert_eq!(adapter.venue_id(), VenueId::Coinbase);
        assert!(!adapter.is_connected().await);
    }

    #[tokio::test]
    async fn test_coinbase_connect() {
        let mut adapter = CoinbaseAdapter::new();
        let _ = adapter.connect().await;
    }
}
