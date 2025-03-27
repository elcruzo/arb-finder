//! Prelude module for arbfinder-exchange
//!
//! Re-exports commonly used types and traits

pub use crate::traits::{
    ExchangeAdapter,
    RestClient,
    ExchangeConfig,
    SymbolNormalizer,
    WebSocketHandler,
    ConnectionStatus,
    SubscriptionInfo,
    SymbolInfo,
    AccountInfo,
    TradingFees,
    MarketDataStream,
    OrderUpdateStream,
};

pub use crate::manager::ExchangeManager;
pub use crate::normalizer::{DefaultSymbolNormalizer, SymbolFormat};
pub use crate::rate_limiter::RateLimiter;

// Re-export common types from core
pub use arbfinder_core::prelude::*;
