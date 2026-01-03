use std::sync::Arc;
use async_trait::async_trait;
use arbfinder_core::prelude::*;
use arbfinder_orderbook::FastOrderBook;

pub mod simple;
pub mod arbitrage;

#[async_trait]
pub trait Strategy: Send + Sync {
    /// The name of the strategy
    fn name(&self) -> String;

    /// Called on each tick of the market data
    async fn on_tick(&mut self, symbol: &Symbol, ticker: &Ticker, orderbook: Arc<FastOrderBook>);

    /// Called when an order is updated
    async fn on_order(&mut self, order: &Order);

    /// Called when a trade is executed
    async fn on_trade(&mut self, trade: &Trade);
}

pub mod prelude {
    pub use super::{Strategy};
    pub use super::simple::*;
    pub use super::arbitrage::*;
}
