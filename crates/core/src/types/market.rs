use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    pub base: String,
    pub quote: String,
}

impl Symbol {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }

    pub fn from_pair(pair: &str) -> Option<Self> {
        if let Some(idx) = pair.find('/') {
            Some(Self::new(&pair[..idx], &pair[idx + 1..]))
        } else {
            None
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn quote(&self) -> &str {
        &self.quote
    }

    pub fn to_pair(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Bid,
    Ask,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Bid => write!(f, "bid"),
            Side::Ask => write!(f, "ask"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub quantity: Decimal,
    pub timestamp: DateTime<Utc>,
}

impl OrderBookLevel {
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self {
            price,
            quantity,
            timestamp: Utc::now(),
        }
    }

    pub fn with_timestamp(price: Decimal, quantity: Decimal, timestamp: DateTime<Utc>) -> Self {
        Self {
            price,
            quantity,
            timestamp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: Symbol,
    pub bids: BTreeMap<ordered_float::OrderedFloat<f64>, OrderBookLevel>,
    pub asks: BTreeMap<ordered_float::OrderedFloat<f64>, OrderBookLevel>,
    pub timestamp: DateTime<Utc>,
    pub sequence: Option<u64>,
}

impl OrderBook {
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            timestamp: Utc::now(),
            sequence: None,
        }
    }

    pub fn best_bid(&self) -> Option<&OrderBookLevel> {
        self.bids.values().last()
    }

    pub fn best_ask(&self) -> Option<&OrderBookLevel> {
        self.asks.values().next()
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / Decimal::from(2)),
            _ => None,
        }
    }

    pub fn update_bid(&mut self, price: Decimal, quantity: Decimal) {
        let key = ordered_float::OrderedFloat(price.to_f64().unwrap_or(0.0));
        if quantity.is_zero() {
            self.bids.remove(&key);
        } else {
            self.bids.insert(key, OrderBookLevel::new(price, quantity));
        }
        self.timestamp = Utc::now();
    }

    pub fn update_ask(&mut self, price: Decimal, quantity: Decimal) {
        let key = ordered_float::OrderedFloat(price.to_f64().unwrap_or(0.0));
        if quantity.is_zero() {
            self.asks.remove(&key);
        } else {
            self.asks.insert(key, OrderBookLevel::new(price, quantity));
        }
        self.timestamp = Utc::now();
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: Symbol,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: Side,
    pub timestamp: DateTime<Utc>,
    pub trade_id: String,
}

impl Trade {
    pub fn new(
        symbol: Symbol,
        price: Decimal,
        quantity: Decimal,
        side: Side,
        trade_id: String,
    ) -> Self {
        Self {
            symbol,
            price,
            quantity,
            side,
            timestamp: Utc::now(),
            trade_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ticker {
    pub symbol: Symbol,
    pub price: Decimal,
    pub volume_24h: Decimal,
    pub change_24h: Decimal,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: Symbol,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub timestamp: DateTime<Utc>,
    pub interval: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketDataType {
    OrderBook,
    Trade,
    Ticker,
    Candle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketData {
    OrderBook(OrderBook),
    Trade(Trade),
    Ticker(Ticker),
    Candle(Candle),
}

impl MarketData {
    pub fn symbol(&self) -> &Symbol {
        match self {
            MarketData::OrderBook(data) => &data.symbol,
            MarketData::Trade(data) => &data.symbol,
            MarketData::Ticker(data) => &data.symbol,
            MarketData::Candle(data) => &data.symbol,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            MarketData::OrderBook(data) => data.timestamp,
            MarketData::Trade(data) => data.timestamp,
            MarketData::Ticker(data) => data.timestamp,
            MarketData::Candle(data) => data.timestamp,
        }
    }

    pub fn data_type(&self) -> MarketDataType {
        match self {
            MarketData::OrderBook(_) => MarketDataType::OrderBook,
            MarketData::Trade(_) => MarketDataType::Trade,
            MarketData::Ticker(_) => MarketDataType::Ticker,
            MarketData::Candle(_) => MarketDataType::Candle,
        }
    }
}