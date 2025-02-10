use std::sync::Arc;
use async_trait::async_trait;
use rust_decimal::Decimal;
use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use arbfinder_orderbook::OrderBook;

pub mod simple;

#[async_trait]
pub trait Strategy: Send + Sync {
    /// The name of the strategy
    fn name(&self) -> String;

    /// Called on each tick of the market data
    async fn on_tick(&mut self, market: &Market, ticker: &Ticker, orderbook: Arc<OrderBook>);

    /// Called when an order is updated
    async fn on_order(&mut self, order: &Order);

    /// Called when a trade is executed
    async fn on_trade(&mut self, trade: &Trade);
}

pub mod prelude {
    pub use super::{Strategy};
    pub use super::simple::*;
}