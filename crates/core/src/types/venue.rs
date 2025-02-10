use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VenueId {
    Binance,
    Coinbase,
    Kraken,
    Bitfinex,
    Huobi,
    OKX,
    Custom(String),
}

impl fmt::Display for VenueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VenueId::Binance => write!(f, "binance"),
            VenueId::Coinbase => write!(f, "coinbase"),
            VenueId::Kraken => write!(f, "kraken"),
            VenueId::Bitfinex => write!(f, "bitfinex"),
            VenueId::Huobi => write!(f, "huobi"),
            VenueId::OKX => write!(f, "okx"),
            VenueId::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl From<&str> for VenueId {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "binance" => VenueId::Binance,
            "coinbase" => VenueId::Coinbase,
            "kraken" => VenueId::Kraken,
            "bitfinex" => VenueId::Bitfinex,
            "huobi" => VenueId::Huobi,
            "okx" => VenueId::OKX,
            name => VenueId::Custom(name.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub total: Decimal,
    pub available: Decimal,
    pub locked: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl Balance {
    pub fn new(asset: String, total: Decimal, available: Decimal, locked: Decimal) -> Self {
        Self {
            asset,
            total,
            available,
            locked,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradingFee {
    pub maker_fee: Decimal,
    pub taker_fee: Decimal,
    pub tier: Option<String>,
}

impl TradingFee {
    pub fn new(maker_fee: Decimal, taker_fee: Decimal) -> Self {
        Self {
            maker_fee,
            taker_fee,
            tier: None,
        }
    }

    pub fn with_tier(maker_fee: Decimal, taker_fee: Decimal, tier: String) -> Self {
        Self {
            maker_fee,
            taker_fee,
            tier: Some(tier),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueInfo {
    pub id: VenueId,
    pub name: String,
    pub base_url: String,
    pub websocket_url: String,
    pub trading_fees: HashMap<String, TradingFee>,
    pub min_order_sizes: HashMap<String, Decimal>,
    pub max_order_sizes: HashMap<String, Decimal>,
    pub tick_sizes: HashMap<String, Decimal>,
    pub supports_margin: bool,
    pub supports_futures: bool,
    pub rate_limits: RateLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimits {
    pub requests_per_second: u32,
    pub requests_per_minute: u32,
    pub orders_per_second: u32,
    pub orders_per_day: u32,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            requests_per_second: 10,
            requests_per_minute: 1200,
            orders_per_second: 10,
            orders_per_day: 200000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueCredentials {
    pub api_key: String,
    pub secret_key: String,
    pub passphrase: Option<String>,
    pub sandbox: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VenueStatus {
    Online,
    Offline,
    Maintenance,
    RateLimited,
    AuthenticationError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueConnection {
    pub venue_id: VenueId,
    pub status: VenueStatus,
    pub last_ping: Option<DateTime<Utc>>,
    pub connected_at: Option<DateTime<Utc>>,
    pub reconnect_count: u32,
    pub error_count: u32,
}

impl VenueConnection {
    pub fn new(venue_id: VenueId) -> Self {
        Self {
            venue_id,
            status: VenueStatus::Offline,
            last_ping: None,
            connected_at: None,
            reconnect_count: 0,
            error_count: 0,
        }
    }

    pub fn mark_connected(&mut self) {
        self.status = VenueStatus::Online;
        self.connected_at = Some(Utc::now());
    }

    pub fn mark_disconnected(&mut self) {
        self.status = VenueStatus::Offline;
        self.connected_at = None;
    }

    pub fn mark_error(&mut self) {
        self.error_count += 1;
    }

    pub fn mark_reconnect(&mut self) {
        self.reconnect_count += 1;
    }

    pub fn update_ping(&mut self) {
        self.last_ping = Some(Utc::now());
    }
}