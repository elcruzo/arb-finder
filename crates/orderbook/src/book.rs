use arbfinder_core::{OrderBook, OrderBookLevel, Side, Symbol};
use chrono::{DateTime, Utc};
use ordered_float::OrderedFloat;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::debug;
use rust_decimal::prelude::ToPrimitive;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FastOrderBook {
    pub symbol: Symbol,
    pub bids: BTreeMap<OrderedFloat<f64>, PriceLevel>,
    pub asks: BTreeMap<OrderedFloat<f64>, PriceLevel>,
    pub sequence: u64,
    pub last_update: DateTime<Utc>,
    pub checksum: Option<u32>,
    max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_count: u32,
    pub last_updated: DateTime<Utc>,
}

impl PriceLevel {
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self {
            price,
            quantity,
            order_count: 1,
            last_updated: Utc::now(),
        }
    }

    pub fn with_order_count(price: Decimal, quantity: Decimal, order_count: u32) -> Self {
        Self {
            price,
            quantity,
            order_count,
            last_updated: Utc::now(),
        }
    }

    pub fn update(&mut self, quantity: Decimal, order_count: Option<u32>) {
        self.quantity = quantity;
        if let Some(count) = order_count {
            self.order_count = count;
        }
        self.last_updated = Utc::now();
    }

    pub fn is_empty(&self) -> bool {
        self.quantity.is_zero()
    }
}

impl FastOrderBook {
    pub fn new(symbol: Symbol, max_depth: Option<usize>) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            sequence: 0,
            last_update: Utc::now(),
            checksum: None,
            max_depth: max_depth.unwrap_or(1000),
        }
    }

    fn increment_sequence(&mut self) {
        self.sequence = self.sequence.wrapping_add(1);
    }

    pub fn get_sequence(&self) -> u64 {
        self.sequence
    }

    pub fn set_sequence(&mut self, sequence: u64) {
        self.sequence = sequence;
    }

    pub fn update_bid(&mut self, price: Decimal, quantity: Decimal, order_count: Option<u32>) {
        let price_key = OrderedFloat(price.to_f64().unwrap_or(0.0));
        
        if quantity.is_zero() {
            self.bids.remove(&price_key);
            debug!("Removed bid level at price: {}", price);
        } else {
            let level = PriceLevel::with_order_count(price, quantity, order_count.unwrap_or(1));
            self.bids.insert(price_key, level);
            debug!("Updated bid level: {} @ {}", quantity, price);
        }

        self.trim_depth(Side::Bid);
        self.increment_sequence();
        self.last_update = Utc::now();
    }

    pub fn update_ask(&mut self, price: Decimal, quantity: Decimal, order_count: Option<u32>) {
        let price_key = OrderedFloat(price.to_f64().unwrap_or(0.0));
        
        if quantity.is_zero() {
            self.asks.remove(&price_key);
            debug!("Removed ask level at price: {}", price);
        } else {
            let level = PriceLevel::with_order_count(price, quantity, order_count.unwrap_or(1));
            self.asks.insert(price_key, level);
            debug!("Updated ask level: {} @ {}", quantity, price);
        }

        self.trim_depth(Side::Ask);
        self.increment_sequence();
        self.last_update = Utc::now();
    }

    pub fn batch_update(&mut self, updates: Vec<OrderBookUpdate>) {
        for update in updates {
            match update.side {
                Side::Bid => self.update_bid(update.price, update.quantity, update.order_count),
                Side::Ask => self.update_ask(update.price, update.quantity, update.order_count),
            }
        }
    }

    pub fn replace_bids(&mut self, levels: Vec<PriceLevel>) {
        self.bids.clear();
        for level in levels {
            let price_key = OrderedFloat(level.price.to_f64().unwrap_or(0.0));
            self.bids.insert(price_key, level);
        }
        self.trim_depth(Side::Bid);
        self.increment_sequence();
        self.last_update = Utc::now();
    }

    pub fn replace_asks(&mut self, levels: Vec<PriceLevel>) {
        self.asks.clear();
        for level in levels {
            let price_key = OrderedFloat(level.price.to_f64().unwrap_or(0.0));
            self.asks.insert(price_key, level);
        }
        self.trim_depth(Side::Ask);
        self.increment_sequence();
        self.last_update = Utc::now();
    }

    pub fn best_bid(&self) -> Option<&PriceLevel> {
        self.bids.values().last()
    }

    pub fn best_ask(&self) -> Option<&PriceLevel> {
        self.asks.values().next()
    }

    pub fn best_bid_price(&self) -> Option<Decimal> {
        self.best_bid().map(|level| level.price)
    }

    pub fn best_ask_price(&self) -> Option<Decimal> {
        self.best_ask().map(|level| level.price)
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::from(2)),
            _ => None,
        }
    }

    pub fn spread_bps(&self) -> Option<i32> {
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) if bid > Decimal::ZERO => {
                let spread_pct = ((ask - bid) / bid) * Decimal::from(10000);
                spread_pct.to_i32()
            }
            _ => None,
        }
    }

    pub fn get_bids(&self, depth: Option<usize>) -> Vec<&PriceLevel> {
        let limit = depth.unwrap_or(self.max_depth);
        self.bids.values().rev().take(limit).collect()
    }

    pub fn get_asks(&self, depth: Option<usize>) -> Vec<&PriceLevel> {
        let limit = depth.unwrap_or(self.max_depth);
        self.asks.values().take(limit).collect()
    }

    pub fn get_depth(&self, side: Side, depth: usize) -> Vec<&PriceLevel> {
        match side {
            Side::Bid => self.get_bids(Some(depth)),
            Side::Ask => self.get_asks(Some(depth)),
        }
    }

    pub fn get_liquidity_at_price(&self, side: Side, target_price: Decimal) -> Decimal {
        match side {
            Side::Bid => {
                self.bids
                    .iter()
                    .filter(|(price, _)| Decimal::from_f64_retain(price.0).unwrap_or_default() >= target_price)
                    .map(|(_, level)| level.quantity)
                    .sum()
            }
            Side::Ask => {
                self.asks
                    .iter()
                    .filter(|(price, _)| Decimal::from_f64_retain(price.0).unwrap_or_default() <= target_price)
                    .map(|(_, level)| level.quantity)
                    .sum()
            }
        }
    }

    pub fn get_volume_weighted_price(&self, side: Side, quantity: Decimal) -> Option<Decimal> {
        if quantity.is_zero() {
            return None;
        }

        let levels = match side {
            Side::Bid => self.get_bids(None),
            Side::Ask => self.get_asks(None),
        };

        let mut remaining_quantity = quantity;
        let mut total_cost = Decimal::ZERO;

        for level in levels {
            if remaining_quantity.is_zero() {
                break;
            }

            let available_quantity = level.quantity.min(remaining_quantity);
            total_cost += available_quantity * level.price;
            remaining_quantity -= available_quantity;
        }

        if remaining_quantity.is_zero() {
            Some(total_cost / quantity)
        } else {
            None // Not enough liquidity
        }
    }

    pub fn calculate_slippage(&self, side: Side, quantity: Decimal) -> Option<Decimal> {
        let best_price = match side {
            Side::Bid => self.best_bid_price()?,
            Side::Ask => self.best_ask_price()?,
        };

        let avg_price = self.get_volume_weighted_price(side, quantity)?;
        
        let slippage = match side {
            Side::Bid => (best_price - avg_price) / best_price,
            Side::Ask => (avg_price - best_price) / best_price,
        };

        Some(slippage.abs() * Decimal::from(100)) // Return as percentage
    }

    pub fn is_crossed(&self) -> bool {
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) => bid >= ask,
            _ => false,
        }
    }

    pub fn total_bid_volume(&self, depth: Option<usize>) -> Decimal {
        self.get_bids(depth).iter().map(|level| level.quantity).sum()
    }

    pub fn total_ask_volume(&self, depth: Option<usize>) -> Decimal {
        self.get_asks(depth).iter().map(|level| level.quantity).sum()
    }

    pub fn imbalance_ratio(&self, depth: Option<usize>) -> Option<f64> {
        let bid_volume = self.total_bid_volume(depth);
        let ask_volume = self.total_ask_volume(depth);
        let total_volume = bid_volume + ask_volume;

        if total_volume.is_zero() {
            None
        } else {
            let ratio = bid_volume / total_volume;
            ratio.to_f64()
        }
    }

    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
        self.sequence = 0;
        self.last_update = Utc::now();
        self.checksum = None;
    }

    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }

    pub fn bid_count(&self) -> usize {
        self.bids.len()
    }

    pub fn ask_count(&self) -> usize {
        self.asks.len()
    }

    pub fn calculate_checksum(&self) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        
        // Include top 10 levels from each side for checksum
        for level in self.get_bids(Some(10)) {
            level.price.hash(&mut hasher);
            level.quantity.hash(&mut hasher);
        }
        
        for level in self.get_asks(Some(10)) {
            level.price.hash(&mut hasher);
            level.quantity.hash(&mut hasher);
        }

        hasher.finish() as u32
    }

    pub fn validate_checksum(&self, expected_checksum: u32) -> bool {
        self.calculate_checksum() == expected_checksum
    }

    fn trim_depth(&mut self, side: Side) {
        match side {
            Side::Bid => {
                while self.bids.len() > self.max_depth {
                    if let Some(min_key) = self.bids.keys().next().cloned() {
                        self.bids.remove(&min_key);
                    } else {
                        break;
                    }
                }
            }
            Side::Ask => {
                while self.asks.len() > self.max_depth {
                    if let Some(max_key) = self.asks.keys().last().cloned() {
                        self.asks.remove(&max_key);
                    } else {
                        break;
                    }
                }
            }
        }
    }

    pub fn to_core_orderbook(&self) -> OrderBook {
        let mut core_book = OrderBook::new(self.symbol.clone());
        
        for (price, level) in &self.bids {
            core_book.bids.insert(
                *price,
                OrderBookLevel::with_timestamp(level.price, level.quantity, level.last_updated),
            );
        }
        
        for (price, level) in &self.asks {
            core_book.asks.insert(
                *price,
                OrderBookLevel::with_timestamp(level.price, level.quantity, level.last_updated),
            );
        }
        
        core_book.timestamp = self.last_update;
        core_book.sequence = Some(self.get_sequence());
        
        core_book
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookUpdate {
    pub side: Side,
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_count: Option<u32>,
    pub timestamp: Option<DateTime<Utc>>,
}

impl OrderBookUpdate {
    pub fn new(side: Side, price: Decimal, quantity: Decimal) -> Self {
        Self {
            side,
            price,
            quantity,
            order_count: None,
            timestamp: Some(Utc::now()),
        }
    }

    pub fn with_order_count(side: Side, price: Decimal, quantity: Decimal, order_count: u32) -> Self {
        Self {
            side,
            price,
            quantity,
            order_count: Some(order_count),
            timestamp: Some(Utc::now()),
        }
    }

    pub fn is_delete(&self) -> bool {
        self.quantity.is_zero()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub symbol: Symbol,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
}

impl OrderBookSnapshot {
    pub fn from_fast_orderbook(book: &FastOrderBook) -> Self {
        Self {
            symbol: book.symbol.clone(),
            bids: book.get_bids(None).into_iter().cloned().collect(),
            asks: book.get_asks(None).into_iter().cloned().collect(),
            sequence: book.get_sequence(),
            timestamp: book.last_update,
        }
    }

    pub fn apply_to_book(&self, book: &mut FastOrderBook) {
        book.symbol = self.symbol.clone();
        book.replace_bids(self.bids.clone());
        book.replace_asks(self.asks.clone());
        book.set_sequence(self.sequence);
        book.last_update = self.timestamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orderbook_creation() {
        let symbol = Symbol::new("BTC", "USDT");
        let book = FastOrderBook::new(symbol.clone(), Some(100));
        
        assert_eq!(book.symbol, symbol);
        assert!(book.is_empty());
        assert_eq!(book.get_sequence(), 0);
    }

    #[test]
    fn test_bid_ask_updates() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);

        // Add some bids and asks
        book.update_bid(Decimal::from(50000), Decimal::from(1), None);
        book.update_bid(Decimal::from(49999), Decimal::from(2), None);
        book.update_ask(Decimal::from(50001), Decimal::from(1), None);
        book.update_ask(Decimal::from(50002), Decimal::from(2), None);

        assert_eq!(book.best_bid_price(), Some(Decimal::from(50000)));
        assert_eq!(book.best_ask_price(), Some(Decimal::from(50001)));
        assert_eq!(book.spread(), Some(Decimal::from(1)));
        assert_eq!(book.mid_price(), Some(Decimal::from_str("50000.5").unwrap()));
    }

    #[test]
    fn test_volume_calculations() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);

        book.update_bid(Decimal::from(50000), Decimal::from(1), None);
        book.update_bid(Decimal::from(49999), Decimal::from(2), None);
        book.update_ask(Decimal::from(50001), Decimal::from(1), None);
        book.update_ask(Decimal::from(50002), Decimal::from(2), None);

        assert_eq!(book.total_bid_volume(None), Decimal::from(3));
        assert_eq!(book.total_ask_volume(None), Decimal::from(3));
        
        let imbalance = book.imbalance_ratio(None).unwrap();
        assert!((imbalance - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_vwap_calculation() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);

        book.update_ask(Decimal::from(50001), Decimal::from(1), None);
        book.update_ask(Decimal::from(50002), Decimal::from(2), None);

        let vwap = book.get_volume_weighted_price(Side::Ask, Decimal::from(2)).unwrap();
        let expected = (Decimal::from(50001) * Decimal::from(1) + Decimal::from(50002) * Decimal::from(1)) / Decimal::from(2);
        assert_eq!(vwap, expected);
    }

    #[test]
    fn test_slippage_calculation() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);

        book.update_ask(Decimal::from(50000), Decimal::from(1), None);
        book.update_ask(Decimal::from(50100), Decimal::from(1), None);

        let slippage = book.calculate_slippage(Side::Ask, Decimal::from(2)).unwrap();
        assert!(slippage > Decimal::ZERO);
    }

    #[test]
    fn test_orderbook_crossing() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);

        book.update_bid(Decimal::from(50000), Decimal::from(1), None);
        book.update_ask(Decimal::from(50001), Decimal::from(1), None);
        assert!(!book.is_crossed());

        book.update_bid(Decimal::from(50002), Decimal::from(1), None);
        assert!(book.is_crossed());
    }

    #[test]
    fn test_sequence_management() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);

        assert_eq!(book.get_sequence(), 0);
        
        book.update_bid(Decimal::from(50000), Decimal::from(1), None);
        assert_eq!(book.get_sequence(), 1);
        
        book.update_ask(Decimal::from(50001), Decimal::from(1), None);
        assert_eq!(book.get_sequence(), 2);
    }

    #[test]
    fn test_checksum() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);

        book.update_bid(Decimal::from(50000), Decimal::from(1), None);
        book.update_ask(Decimal::from(50001), Decimal::from(1), None);

        let checksum = book.calculate_checksum();
        assert!(book.validate_checksum(checksum));
        assert!(!book.validate_checksum(checksum + 1));
    }
}