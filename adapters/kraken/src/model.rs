use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;

#[derive(Debug, Serialize, Deserialize)]
pub struct KrakenResponse<T> {
    pub error: Vec<String>,
    pub result: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetPair {
    pub altname: String,
    pub wsname: Option<String>,
    pub aclass_base: String,
    pub base: String,
    pub aclass_quote: String,
    pub quote: String,
    pub lot: String,
    pub pair_decimals: i32,
    pub lot_decimals: i32,
    pub lot_multiplier: i32,
    pub leverage_buy: Vec<i32>,
    pub leverage_sell: Vec<i32>,
    pub fees: Vec<Vec<Decimal>>,
    pub fees_maker: Option<Vec<Vec<Decimal>>>,
    pub fee_volume_currency: String,
    pub margin_call: i32,
    pub margin_stop: i32,
    pub ordermin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ticker {
    pub a: Vec<String>, // ask [price, whole lot volume, lot volume]
    pub b: Vec<String>, // bid [price, whole lot volume, lot volume]
    pub c: Vec<String>, // last trade closed [price, lot volume]
    pub v: Vec<String>, // volume [today, last 24 hours]
    pub p: Vec<String>, // volume weighted average price [today, last 24 hours]
    pub t: Vec<i64>,   // number of trades [today, last 24 hours]
    pub l: Vec<String>, // low [today, last 24 hours]
    pub h: Vec<String>, // high [today, last 24 hours]
    pub o: String,     // today's opening price
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderBook {
    pub asks: Vec<Vec<String>>, // [[price, volume, timestamp], ...]
    pub bids: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trade {
    pub ordertxid: String,
    pub pair: String,
    pub time: f64,
    pub r#type: String,
    pub ordertype: String,
    pub price: String,
    pub cost: String,
    pub fee: String,
    pub vol: String,
    pub margin: String,
    pub misc: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Balance {
    #[serde(flatten)]
    pub balances: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub refid: Option<String>,
    pub userref: Option<i32>,
    pub status: String,
    pub opentm: f64,
    pub starttm: f64,
    pub expiretm: f64,
    pub descr: OrderDescription,
    pub vol: String,
    pub vol_exec: String,
    pub cost: String,
    pub fee: String,
    pub price: String,
    pub stopprice: String,
    pub limitprice: String,
    pub misc: String,
    pub oflags: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderDescription {
    pub pair: String,
    pub r#type: String,
    pub ordertype: String,
    pub price: String,
    pub price2: String,
    pub leverage: String,
    pub order: String,
    pub close: Option<String>,
}