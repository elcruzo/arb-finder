pub use arbfinder_core::prelude::*;
pub use arbfinder_exchange::prelude::*;
pub use arbfinder_orderbook::prelude::*;
pub use arbfinder_strategy::prelude::*;
pub use arbfinder_execution::prelude::*;
pub use arbfinder_monitoring::prelude::*;

// Re-export exchange adapters
pub use arbfinder_binance::BinanceClient;
pub use arbfinder_coinbase::CoinbaseClient;
pub use arbfinder_kraken::KrakenClient;

// Re-export main application
pub mod app {
    pub use crate::main::{ArbFinderApp, AppConfig, ExchangeConfigs, ExchangeCredentials};
}