//! OrderBook Builder Pattern Implementation
//!
//! Provides a builder pattern for creating and configuring order books

use arbfinder_core::{Symbol, VenueId};
use crate::{FastOrderBook, OrderBookSnapshot, PriceLevel};

/// Builder for FastOrderBook
pub struct OrderBookBuilder {
    symbol: Option<Symbol>,
    venue_id: Option<VenueId>,
    max_depth: Option<usize>,
    initial_bids: Vec<PriceLevel>,
    initial_asks: Vec<PriceLevel>,
    sequence: Option<u64>,
}

impl OrderBookBuilder {
    pub fn new() -> Self {
        Self {
            symbol: None,
            venue_id: None,
            max_depth: None,
            initial_bids: Vec::new(),
            initial_asks: Vec::new(),
            sequence: None,
        }
    }

    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    pub fn venue_id(mut self, venue_id: VenueId) -> Self {
        self.venue_id = Some(venue_id);
        self
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn with_bids(mut self, bids: Vec<PriceLevel>) -> Self {
        self.initial_bids = bids;
        self
    }

    pub fn with_asks(mut self, asks: Vec<PriceLevel>) -> Self {
        self.initial_asks = asks;
        self
    }

    pub fn sequence(mut self, seq: u64) -> Self {
        self.sequence = Some(seq);
        self
    }

    pub fn from_snapshot(mut self, snapshot: &OrderBookSnapshot) -> Self {
        self.symbol = Some(snapshot.symbol.clone());
        self.initial_bids = snapshot.bids.clone();
        self.initial_asks = snapshot.asks.clone();
        self.sequence = Some(snapshot.sequence);
        self
    }

    pub fn build(self) -> Result<FastOrderBook, String> {
        let symbol = self.symbol.ok_or("Symbol is required")?;
        
        let mut book = FastOrderBook::new(symbol, self.max_depth);
        
        if !self.initial_bids.is_empty() {
            book.replace_bids(self.initial_bids);
        }
        
        if !self.initial_asks.is_empty() {
            book.replace_asks(self.initial_asks);
        }
        
        if let Some(seq) = self.sequence {
            book.set_sequence(seq);
        }
        
        Ok(book)
    }
}

impl Default for OrderBookBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_builder_basic() {
        let symbol = Symbol::new("BTC", "USDT");
        let book = OrderBookBuilder::new()
            .symbol(symbol.clone())
            .max_depth(100)
            .build()
            .unwrap();

        assert_eq!(book.symbol, symbol);
        assert!(book.is_empty());
    }

    #[test]
    fn test_builder_with_levels() {
        let symbol = Symbol::new("BTC", "USDT");
        let bid = PriceLevel::new(Decimal::from(50000), Decimal::from(1));
        let ask = PriceLevel::new(Decimal::from(50001), Decimal::from(1));

        let book = OrderBookBuilder::new()
            .symbol(symbol)
            .with_bids(vec![bid])
            .with_asks(vec![ask])
            .build()
            .unwrap();

        assert!(!book.is_empty());
        assert!(book.best_bid().is_some());
        assert!(book.best_ask().is_some());
    }
}
