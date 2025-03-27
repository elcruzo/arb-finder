use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::{RwLock, mpsc};
use rust_decimal::Decimal;
use tracing::{info, warn, error};

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use arbfinder_orderbook::FastOrderBook;
use arbfinder_strategy::prelude::*;

pub mod engine;
pub mod portfolio;
pub mod risk;

pub use engine::ExecutionEngine;
pub use portfolio::Portfolio;
pub use risk::RiskManager;

#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub max_position_size: Decimal,
    pub max_daily_loss: Decimal,
    pub max_orders_per_second: u32,
    pub enable_paper_trading: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_position_size: Decimal::from(1000),
            max_daily_loss: Decimal::from(500),
            max_orders_per_second: 10,
            enable_paper_trading: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    OrderPlaced(Order),
    OrderFilled(Order),
    OrderCanceled(Order),
    TradeExecuted(Trade),
    RiskLimitHit(String),
    StrategySignal {
        strategy: String,
        symbol: Symbol,
        signal: TradingSignal,
    },
}

#[derive(Debug, Clone)]
pub struct TradingSignal {
    pub side: OrderSide,
    pub price: Decimal,
    pub amount: Decimal,
    pub confidence: f64,
    pub reason: String,
}

pub mod prelude {
    pub use super::{ExecutionEngine, Portfolio, RiskManager, ExecutionConfig, ExecutionEvent, TradingSignal};
}