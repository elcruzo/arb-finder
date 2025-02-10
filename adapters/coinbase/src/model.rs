use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub base_min_size: Decimal,
    pub base_max_size: Decimal,
    pub quote_increment: Decimal,
    pub min_market_funds: Decimal,
    pub max_market_funds: Decimal,
    pub status: String,
    pub status_message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderBook {
    pub sequence: u64,
    pub bids: Vec<[String; 3]>, // price, size, num_orders
    pub asks: Vec<[String; 3]>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ticker {
    pub trade_id: u64,
    pub sequence: u64,
    pub price: String,
    pub size: String,
    pub bid: String,
    pub ask: String,
    pub volume: String,
    pub time: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trade {
    pub trade_id: u64,
    pub price: String,
    pub size: String,
    pub time: DateTime<Utc>,
    pub side: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub currency: String,
    pub balance: String,
    pub available: String,
    pub hold: String,
    pub profile_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub price: String,
    pub size: String,
    pub product_id: String,
    pub side: String,
    pub stp: Option<String>,
    pub r#type: String,
    pub time_in_force: Option<String>,
    pub post_only: bool,
    pub created_at: DateTime<Utc>,
    pub fill_fees: String,
    pub filled_size: String,
    pub executed_value: String,
    pub status: String,
    pub settled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fill {
    pub trade_id: u64,
    pub product_id: String,
    pub price: String,
    pub size: String,
    pub order_id: String,
    pub created_at: DateTime<Utc>,
    pub liquidity: String,
    pub fee: String,
    pub settled: bool,
    pub side: String,
}