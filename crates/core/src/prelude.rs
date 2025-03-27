//! Prelude module - re-exports commonly used types for convenience

pub use crate::error::{ArbFinderError, Result};
pub use crate::types::{
    arbitrage::*,
    market::*,
    order::*,
    venue::*,
};

// Re-export commonly used external types
pub use rust_decimal::Decimal;
pub use chrono::{DateTime, Utc};
pub use uuid::Uuid;
