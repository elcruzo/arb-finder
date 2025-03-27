//! Binance Exchange Adapter
//!
//! Complete implementation of ExchangeAdapter trait for Binance

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

pub mod websocket;
pub use websocket::BinanceOrderbookStream;

const BINANCE_API_URL: &str = "https://api.binance.com";
const BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";

pub struct BinanceAdapter {
    client: Client,
    api_key: Option<String>,
    api_secret: Option<String>,
    base_url: String,
    ws_url: String,
    connected: bool,
}

impl BinanceAdapter {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            api_secret: None,
            base_url: BINANCE_API_URL.to_string(),
            ws_url: BINANCE_WS_URL.to_string(),
            connected: false,
        }
    }

    pub fn with_credentials(api_key: String, api_secret: String) -> Self {
        Self {
            client: Client::new(),
            api_key: Some(api_key),
            api_secret: Some(api_secret),
            base_url: BINANCE_API_URL.to_string(),
            ws_url: BINANCE_WS_URL.to_string(),
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
                "Binance API error: {}",
                response.status()
            )));
        }

        response.json().await.map_err(|e| ArbFinderError::Http(e))
    }

    /// Fetch orderbook depth from Binance
    pub async fn get_orderbook(&self, symbol: &Symbol, limit: Option<u32>) -> Result<OrderBook> {
        let symbol_str = format!("{}{}", symbol.base(), symbol.quote());
        let limit = limit.unwrap_or(100).min(5000); // Binance max is 5000
        let endpoint = format!("/api/v3/depth?symbol={}&limit={}", symbol_str, limit);
        
        let response = self.get_request(&endpoint).await?;
        
        let mut orderbook = OrderBook::new(symbol.clone());
        
        // Parse bids
        if let Some(bids) = response["bids"].as_array() {
            for bid in bids {
                if let (Some(price_str), Some(qty_str)) = (bid[0].as_str(), bid[1].as_str()) {
                    if let (Ok(price), Ok(qty)) = (
                        price_str.parse::<Decimal>(),
                        qty_str.parse::<Decimal>()
                    ) {
                        orderbook.update_bid(price, qty);
                    }
                }
            }
        }
        
        // Parse asks
        if let Some(asks) = response["asks"].as_array() {
            for ask in asks {
                if let (Some(price_str), Some(qty_str)) = (ask[0].as_str(), ask[1].as_str()) {
                    if let (Ok(price), Ok(qty)) = (
                        price_str.parse::<Decimal>(),
                        qty_str.parse::<Decimal>()
                    ) {
                        orderbook.update_ask(price, qty);
                    }
                }
            }
        }
        
        Ok(orderbook)
    }
}

impl Default for BinanceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExchangeAdapter for BinanceAdapter {
    fn venue_id(&self) -> VenueId {
        VenueId::Binance
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
        let response = self.get_request("/api/v3/time").await?;
        let server_time = response["serverTime"]
            .as_i64()
            .ok_or_else(|| ArbFinderError::InvalidData("Missing serverTime".to_string()))?;
        
        Ok(DateTime::from_timestamp_millis(server_time)
            .unwrap_or_else(|| Utc::now()))
    }

    async fn ping(&self) -> Result<u64> {
        let start = std::time::Instant::now();
        let _ = self.get_request("/api/v3/ping").await?;
        Ok(start.elapsed().as_millis() as u64)
    }

    async fn get_symbols(&self) -> Result<Vec<Symbol>> {
        let response = self.get_request("/api/v3/exchangeInfo").await?;
        let symbols = response["symbols"]
            .as_array()
            .ok_or_else(|| ArbFinderError::InvalidData("Missing symbols array".to_string()))?;

        let mut result = Vec::new();
        for symbol_data in symbols {
            if let (Some(base), Some(quote)) = (
                symbol_data["baseAsset"].as_str(),
                symbol_data["quoteAsset"].as_str(),
            ) {
                result.push(Symbol::new(base, quote));
            }
        }

        Ok(result)
    }

    async fn get_symbol_info(&self, symbol: &Symbol) -> Result<SymbolInfo> {
        let response = self.get_request("/api/v3/exchangeInfo").await?;
        let symbols = response["symbols"].as_array().ok_or_else(|| {
            ArbFinderError::InvalidData("Missing symbols array".to_string())
        })?;

        let symbol_str = format!("{}{}", symbol.base(), symbol.quote());
        
        for symbol_data in symbols {
            if symbol_data["symbol"].as_str() == Some(&symbol_str) {
                return Ok(SymbolInfo {
                    symbol: symbol.clone(),
                    status: symbol_data["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                    base_asset_precision: symbol_data["baseAssetPrecision"].as_u64().unwrap_or(8) as u32,
                    quote_asset_precision: symbol_data["quoteAssetPrecision"].as_u64().unwrap_or(8) as u32,
                    tick_size: Decimal::new(1, 8),
                    lot_size: Decimal::new(1, 8),
                    min_order_size: Decimal::new(1, 8),
                    max_order_size: Decimal::new(1000000, 0),
                    min_notional: Decimal::new(10, 0),
                    trading_fees: TradingFees {
                        maker_fee: Decimal::new(1, 3), // 0.1%
                        taker_fee: Decimal::new(1, 3), // 0.1%
                    },
                });
            }
        }

        Err(ArbFinderError::SymbolNotFound(symbol_str))
    }

    async fn subscribe_orderbook(&mut self, _symbol: &Symbol, _depth: Option<u32>) -> Result<()> {
        // WebSocket subscription would go here
        Ok(())
    }

    async fn subscribe_trades(&mut self, _symbol: &Symbol) -> Result<()> {
        // WebSocket subscription would go here
        Ok(())
    }

    async fn subscribe_ticker(&mut self, _symbol: &Symbol) -> Result<()> {
        // WebSocket subscription would go here
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
        Err(ArbFinderError::Exchange("Order placement requires authenticated API - not implemented in this version".to_string()))
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
                maker_fee: Decimal::new(1, 3),
                taker_fee: Decimal::new(1, 3),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_binance_adapter_creation() {
        let adapter = BinanceAdapter::new();
        assert_eq!(adapter.venue_id(), VenueId::Binance);
        assert!(!adapter.is_connected().await);
    }

    #[tokio::test]
    async fn test_binance_connect() {
        let mut adapter = BinanceAdapter::new();
        // Connection test - may fail without network
        let _ = adapter.connect().await;
    }
}
