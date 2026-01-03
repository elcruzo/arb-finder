use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use rust_decimal::Decimal;
use std::str::FromStr;

const BINANCE_WS_BASE: &str = "wss://stream.binance.com:9443";

#[derive(Debug, Clone, Deserialize)]
struct BinanceDepthUpdate {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "E")]
    event_time: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "U")]
    first_update_id: u64,
    #[serde(rename = "u")]
    final_update_id: u64,
    #[serde(rename = "b")]
    bids: Vec<(String, String)>, // [price, quantity]
    #[serde(rename = "a")]
    asks: Vec<(String, String)>, // [price, quantity]
}

pub struct BinanceOrderbookStream {
    symbol: Symbol,
    orderbook: Arc<RwLock<OrderBook>>,
    update_tx: mpsc::UnboundedSender<OrderBook>,
    last_update_id: u64,
}

impl BinanceOrderbookStream {
    pub fn new(
        symbol: Symbol,
        update_tx: mpsc::UnboundedSender<OrderBook>,
    ) -> Self {
        let orderbook = Arc::new(RwLock::new(OrderBook::new(symbol.clone())));
        
        Self {
            symbol,
            orderbook,
            update_tx,
            last_update_id: 0,
        }
    }

    pub fn get_ws_url(&self) -> String {
        let stream_name = format!("{}{}@depth", 
            self.symbol.base().to_lowercase(),
            self.symbol.quote().to_lowercase()
        );
        format!("{}/ws/{}", BINANCE_WS_BASE, stream_name)
    }

    pub async fn get_orderbook(&self) -> OrderBook {
        self.orderbook.read().await.clone()
    }

    async fn process_depth_update(&mut self, update: BinanceDepthUpdate) -> Result<()> {
        // Check for sequence gaps
        if self.last_update_id > 0 && update.first_update_id != self.last_update_id + 1 {
            warn!(
                "Sequence gap detected: expected {}, got {}",
                self.last_update_id + 1,
                update.first_update_id
            );
        }

        let mut orderbook = self.orderbook.write().await;

        // Update bids
        for (price_str, qty_str) in update.bids {
            if let (Ok(price), Ok(qty)) = (
                Decimal::from_str(&price_str),
                Decimal::from_str(&qty_str)
            ) {
                orderbook.update_bid(price, qty);
            }
        }

        // Update asks
        for (price_str, qty_str) in update.asks {
            if let (Ok(price), Ok(qty)) = (
                Decimal::from_str(&price_str),
                Decimal::from_str(&qty_str)
            ) {
                orderbook.update_ask(price, qty);
            }
        }

        self.last_update_id = update.final_update_id;

        // Send update notification
        let _ = self.update_tx.send(orderbook.clone());

        debug!(
            "Updated {} orderbook: {} bids, {} asks (seq: {})",
            self.symbol.to_pair(),
            orderbook.bids.len(),
            orderbook.asks.len(),
            self.last_update_id
        );

        Ok(())
    }
}

#[async_trait]
impl WebSocketHandler for BinanceOrderbookStream {
    async fn on_message(&mut self, message: &str) -> Result<()> {
        match serde_json::from_str::<BinanceDepthUpdate>(message) {
            Ok(update) => {
                if update.symbol == format!("{}{}", self.symbol.base(), self.symbol.quote()) {
                    self.process_depth_update(update).await?;
                }
            }
            Err(e) => {
                // Not a depth update, might be other message type
                debug!("Failed to parse as depth update: {}", e);
            }
        }
        Ok(())
    }

    async fn on_connect(&mut self) -> Result<()> {
        info!("Binance WebSocket connected for {}", self.symbol.to_pair());
        Ok(())
    }

    async fn on_disconnect(&mut self) -> Result<()> {
        warn!("Binance WebSocket disconnected for {}", self.symbol.to_pair());
        Ok(())
    }

    async fn on_error(&mut self, error: &ArbFinderError) -> Result<()> {
        error!("Binance WebSocket error for {}: {}", self.symbol.to_pair(), error);
        Ok(())
    }

    async fn on_ping(&mut self) -> Result<()> {
        debug!("Received ping for {}", self.symbol.to_pair());
        Ok(())
    }

    async fn on_pong(&mut self) -> Result<()> {
        debug!("Received pong for {}", self.symbol.to_pair());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_url_generation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let stream = BinanceOrderbookStream::new(Symbol::new("BTC", "USDT"), tx);
        let url = stream.get_ws_url();
        assert_eq!(url, "wss://stream.binance.com:9443/ws/btcusdt@depth");
    }

    #[tokio::test]
    async fn test_process_depth_update() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut stream = BinanceOrderbookStream::new(Symbol::new("BTC", "USDT"), tx);

        let update_json = r#"{
            "e": "depthUpdate",
            "E": 1638747741000,
            "s": "BTCUSDT",
            "U": 1,
            "u": 1,
            "b": [
                ["50000.00", "1.5"],
                ["49999.00", "2.0"]
            ],
            "a": [
                ["50001.00", "1.0"],
                ["50002.00", "0.5"]
            ]
        }"#;

        let update: BinanceDepthUpdate = serde_json::from_str(update_json).unwrap();
        stream.process_depth_update(update).await.unwrap();

        // Check we received the update
        let received_book = rx.recv().await.unwrap();
        assert!(received_book.best_bid().is_some());
        assert!(received_book.best_ask().is_some());

        let best_bid = received_book.best_bid().unwrap();
        let best_ask = received_book.best_ask().unwrap();

        assert_eq!(best_bid.price, Decimal::from_str("50000.00").unwrap());
        assert_eq!(best_ask.price, Decimal::from_str("50001.00").unwrap());
    }
}
