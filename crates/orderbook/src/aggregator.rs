//! OrderBook Aggregator
//!
//! Aggregates order books from multiple venues into a unified view

use std::collections::HashMap;
use arbfinder_core::{Symbol, VenueId};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

use crate::{FastOrderBook, PriceLevel};

/// Aggregated order book combining data from multiple venues
pub struct AggregatedOrderBook {
    pub symbol: Symbol,
    pub venues: HashMap<VenueId, FastOrderBook>,
    pub last_update: DateTime<Utc>,
}

impl AggregatedOrderBook {
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            venues: HashMap::new(),
            last_update: Utc::now(),
        }
    }

    pub fn add_venue(&mut self, venue_id: VenueId, orderbook: FastOrderBook) {
        self.venues.insert(venue_id, orderbook);
        self.last_update = Utc::now();
    }

    pub fn remove_venue(&mut self, venue_id: &VenueId) -> Option<FastOrderBook> {
        let result = self.venues.remove(venue_id);
        self.last_update = Utc::now();
        result
    }

    pub fn get_venue_book(&self, venue_id: &VenueId) -> Option<&FastOrderBook> {
        self.venues.get(venue_id)
    }

    pub fn get_venue_book_mut(&mut self, venue_id: &VenueId) -> Option<&mut FastOrderBook> {
        self.venues.get_mut(venue_id)
    }

    /// Get best bid across all venues
    pub fn best_bid_across_venues(&self) -> Option<(VenueId, &PriceLevel)> {
        self.venues
            .iter()
            .filter_map(|(venue_id, book)| {
                book.best_bid().map(|level| (venue_id.clone(), level))
            })
            .max_by(|(_, a), (_, b)| a.price.cmp(&b.price))
    }

    /// Get best ask across all venues
    pub fn best_ask_across_venues(&self) -> Option<(VenueId, &PriceLevel)> {
        self.venues
            .iter()
            .filter_map(|(venue_id, book)| {
                book.best_ask().map(|level| (venue_id.clone(), level))
            })
            .min_by(|(_, a), (_, b)| a.price.cmp(&b.price))
    }

    /// Calculate best cross-venue spread
    pub fn cross_venue_spread(&self) -> Option<Decimal> {
        let best_bid = self.best_bid_across_venues()?.1.price;
        let best_ask = self.best_ask_across_venues()?.1.price;
        Some(best_ask - best_bid)
    }

    /// Get total liquidity at a price level across all venues
    pub fn total_liquidity_at_price(&self, price: Decimal, is_bid: bool) -> Decimal {
        self.venues
            .values()
            .map(|book| {
                let side = if is_bid { arbfinder_core::Side::Bid } else { arbfinder_core::Side::Ask };
                book.get_liquidity_at_price(side, price)
            })
            .sum()
    }

    /// Get combined orderbook depth across all venues
    pub fn aggregate_depth(&self, depth: usize) -> (Vec<AggregatedLevel>, Vec<AggregatedLevel>) {
        let mut all_bids: Vec<AggregatedLevel> = Vec::new();
        let mut all_asks: Vec<AggregatedLevel> = Vec::new();

        for (venue_id, book) in &self.venues {
            for level in book.get_bids(Some(depth)) {
                all_bids.push(AggregatedLevel {
                    price: level.price,
                    quantity: level.quantity,
                    venue: venue_id.clone(),
                    order_count: level.order_count,
                });
            }

            for level in book.get_asks(Some(depth)) {
                all_asks.push(AggregatedLevel {
                    price: level.price,
                    quantity: level.quantity,
                    venue: venue_id.clone(),
                    order_count: level.order_count,
                });
            }
        }

        // Sort bids descending by price
        all_bids.sort_by(|a, b| b.price.cmp(&a.price));
        // Sort asks ascending by price
        all_asks.sort_by(|a, b| a.price.cmp(&b.price));

        (all_bids, all_asks)
    }

    /// Check if any venues have crossed order books
    pub fn has_crossed_venues(&self) -> bool {
        self.venues.values().any(|book| book.is_crossed())
    }

    /// Get venue count
    pub fn venue_count(&self) -> usize {
        self.venues.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.venues.is_empty() || self.venues.values().all(|book| book.is_empty())
    }
}

#[derive(Debug, Clone)]
pub struct AggregatedLevel {
    pub price: Decimal,
    pub quantity: Decimal,
    pub venue: VenueId,
    pub order_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregated_orderbook() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut agg_book = AggregatedOrderBook::new(symbol.clone());

        assert!(agg_book.is_empty());
        assert_eq!(agg_book.venue_count(), 0);

        let book1 = FastOrderBook::new(symbol.clone(), None);
        agg_book.add_venue(VenueId::Binance, book1);

        assert_eq!(agg_book.venue_count(), 1);
    }
}
