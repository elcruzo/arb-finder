use arbfinder_core::{Side, Symbol, VenueId};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{FastOrderBook, OrderBookSnapshot, OrderBookUpdate, PriceLevel};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderBookEvent {
    Snapshot(OrderBookSnapshotEvent),
    Update(OrderBookUpdateEvent),
    BestBidAskUpdate(BestBidAskEvent),
    SpreadUpdate(SpreadUpdateEvent),
    VolumeUpdate(VolumeUpdateEvent),
    CrossingDetected(CrossingEvent),
    LiquidityGap(LiquidityGapEvent),
    PriceMovement(PriceMovementEvent),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookSnapshotEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub snapshot: OrderBookSnapshot,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookUpdateEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub updates: Vec<OrderBookUpdate>,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BestBidAskEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub best_bid: Option<PriceLevel>,
    pub best_ask: Option<PriceLevel>,
    pub previous_best_bid: Option<PriceLevel>,
    pub previous_best_ask: Option<PriceLevel>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpreadUpdateEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub spread: Option<Decimal>,
    pub spread_bps: Option<i32>,
    pub previous_spread: Option<Decimal>,
    pub mid_price: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolumeUpdateEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub total_bid_volume: Decimal,
    pub total_ask_volume: Decimal,
    pub imbalance_ratio: Option<f64>,
    pub depth: usize,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossingEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub best_bid: PriceLevel,
    pub best_ask: PriceLevel,
    pub cross_amount: Decimal,
    pub severity: CrossingSeverity,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossingSeverity {
    Minor,    // Small crossing, likely due to latency
    Moderate, // Significant crossing
    Severe,   // Large crossing, possible error
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiquidityGapEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub side: Side,
    pub gap_start: Decimal,
    pub gap_end: Decimal,
    pub gap_size: Decimal,
    pub depth_level: usize,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceMovementEvent {
    pub venue_id: VenueId,
    pub symbol: Symbol,
    pub side: Side,
    pub old_price: Decimal,
    pub new_price: Decimal,
    pub change_bps: i32,
    pub movement_type: PriceMovementType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriceMovementType {
    Improvement, // Better price for the side
    Degradation, // Worse price for the side
    Neutral,     // Same price
}

pub trait OrderBookEventHandler: Send + Sync {
    fn handle_event(&mut self, event: OrderBookEvent);
}

#[derive(Debug)]
pub struct OrderBookEventProcessor {
    handlers: Vec<Box<dyn OrderBookEventHandler>>,
    previous_state: Option<OrderBookState>,
}

#[derive(Debug, Clone)]
struct OrderBookState {
    best_bid: Option<PriceLevel>,
    best_ask: Option<PriceLevel>,
    spread: Option<Decimal>,
    total_bid_volume: Decimal,
    total_ask_volume: Decimal,
    imbalance_ratio: Option<f64>,
}

impl OrderBookEventProcessor {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            previous_state: None,
        }
    }

    pub fn add_handler(&mut self, handler: Box<dyn OrderBookEventHandler>) {
        self.handlers.push(handler);
    }

    pub fn process_book_update(&mut self, venue_id: VenueId, book: &FastOrderBook) {
        let current_state = OrderBookState {
            best_bid: book.best_bid().cloned(),
            best_ask: book.best_ask().cloned(),
            spread: book.spread(),
            total_bid_volume: book.total_bid_volume(Some(10)),
            total_ask_volume: book.total_ask_volume(Some(10)),
            imbalance_ratio: book.imbalance_ratio(Some(10)),
        };

        let events = self.generate_events(venue_id, book, &current_state);
        
        for event in events {
            self.emit_event(event);
        }

        self.previous_state = Some(current_state);
    }

    fn generate_events(
        &self,
        venue_id: VenueId,
        book: &FastOrderBook,
        current_state: &OrderBookState,
    ) -> Vec<OrderBookEvent> {
        let mut events = Vec::new();
        let timestamp = Utc::now();

        // Check for best bid/ask changes
        if let Some(prev_state) = &self.previous_state {
            if prev_state.best_bid != current_state.best_bid 
                || prev_state.best_ask != current_state.best_ask {
                events.push(OrderBookEvent::BestBidAskUpdate(BestBidAskEvent {
                    venue_id: venue_id.clone(),
                    symbol: book.symbol.clone(),
                    best_bid: current_state.best_bid.clone(),
                    best_ask: current_state.best_ask.clone(),
                    previous_best_bid: prev_state.best_bid.clone(),
                    previous_best_ask: prev_state.best_ask.clone(),
                    timestamp,
                }));
            }

            // Check for spread changes
            if prev_state.spread != current_state.spread {
                events.push(OrderBookEvent::SpreadUpdate(SpreadUpdateEvent {
                    venue_id: venue_id.clone(),
                    symbol: book.symbol.clone(),
                    spread: current_state.spread,
                    spread_bps: book.spread_bps(),
                    previous_spread: prev_state.spread,
                    mid_price: book.mid_price(),
                    timestamp,
                }));
            }

            // Check for significant volume changes
            let volume_change_threshold = Decimal::from_str("0.1").unwrap(); // 10% change
            let prev_total = prev_state.total_bid_volume + prev_state.total_ask_volume;
            let current_total = current_state.total_bid_volume + current_state.total_ask_volume;
            
            if prev_total > Decimal::ZERO {
                let volume_change = ((current_total - prev_total) / prev_total).abs();
                if volume_change > volume_change_threshold {
                    events.push(OrderBookEvent::VolumeUpdate(VolumeUpdateEvent {
                        venue_id: venue_id.clone(),
                        symbol: book.symbol.clone(),
                        total_bid_volume: current_state.total_bid_volume,
                        total_ask_volume: current_state.total_ask_volume,
                        imbalance_ratio: current_state.imbalance_ratio,
                        depth: 10,
                        timestamp,
                    }));
                }
            }

            // Check for price movements
            self.check_price_movements(
                venue_id.clone(),
                book,
                &prev_state,
                &current_state,
                &mut events,
                timestamp,
            );
        }

        // Check for order book crossing
        if book.is_crossed() {
            if let (Some(best_bid), Some(best_ask)) = (&current_state.best_bid, &current_state.best_ask) {
                let cross_amount = best_bid.price - best_ask.price;
                let severity = if cross_amount > Decimal::from(100) {
                    CrossingSeverity::Severe
                } else if cross_amount > Decimal::from(10) {
                    CrossingSeverity::Moderate
                } else {
                    CrossingSeverity::Minor
                };

                events.push(OrderBookEvent::CrossingDetected(CrossingEvent {
                    venue_id: venue_id.clone(),
                    symbol: book.symbol.clone(),
                    best_bid: best_bid.clone(),
                    best_ask: best_ask.clone(),
                    cross_amount,
                    severity,
                    timestamp,
                }));
            }
        }

        // Check for liquidity gaps
        let gaps = self.detect_liquidity_gaps(book);
        for gap in gaps {
            events.push(OrderBookEvent::LiquidityGap(LiquidityGapEvent {
                venue_id: venue_id.clone(),
                symbol: book.symbol.clone(),
                side: gap.0,
                gap_start: gap.1,
                gap_end: gap.2,
                gap_size: gap.2 - gap.1,
                depth_level: gap.3,
                timestamp,
            }));
        }

        events
    }

    fn check_price_movements(
        &self,
        venue_id: VenueId,
        book: &FastOrderBook,
        prev_state: &OrderBookState,
        current_state: &OrderBookState,
        events: &mut Vec<OrderBookEvent>,
        timestamp: DateTime<Utc>,
    ) {
        // Check bid movement
        if let (Some(prev_bid), Some(current_bid)) = (&prev_state.best_bid, &current_state.best_bid) {
            if prev_bid.price != current_bid.price {
                let change_bps = self.calculate_change_bps(prev_bid.price, current_bid.price);
                let movement_type = if current_bid.price > prev_bid.price {
                    PriceMovementType::Improvement
                } else {
                    PriceMovementType::Degradation
                };

                events.push(OrderBookEvent::PriceMovement(PriceMovementEvent {
                    venue_id: venue_id.clone(),
                    symbol: book.symbol.clone(),
                    side: Side::Bid,
                    old_price: prev_bid.price,
                    new_price: current_bid.price,
                    change_bps,
                    movement_type,
                    timestamp,
                }));
            }
        }

        // Check ask movement
        if let (Some(prev_ask), Some(current_ask)) = (&prev_state.best_ask, &current_state.best_ask) {
            if prev_ask.price != current_ask.price {
                let change_bps = self.calculate_change_bps(prev_ask.price, current_ask.price);
                let movement_type = if current_ask.price < prev_ask.price {
                    PriceMovementType::Improvement
                } else {
                    PriceMovementType::Degradation
                };

                events.push(OrderBookEvent::PriceMovement(PriceMovementEvent {
                    venue_id: venue_id.clone(),
                    symbol: book.symbol.clone(),
                    side: Side::Ask,
                    old_price: prev_ask.price,
                    new_price: current_ask.price,
                    change_bps,
                    movement_type,
                    timestamp,
                }));
            }
        }
    }

    fn calculate_change_bps(&self, old_price: Decimal, new_price: Decimal) -> i32 {
        if old_price.is_zero() {
            return 0;
        }
        let change_pct = ((new_price - old_price) / old_price) * Decimal::from(10000);
        change_pct.to_i32().unwrap_or(0)
    }

    fn detect_liquidity_gaps(&self, book: &FastOrderBook) -> Vec<(Side, Decimal, Decimal, usize)> {
        let mut gaps = Vec::new();
        let gap_threshold = Decimal::from_str("0.01").unwrap(); // 1% gap threshold

        // Check bid side gaps
        let bids = book.get_bids(Some(20));
        for (i, window) in bids.windows(2).enumerate() {
            let higher_bid = &window[0];
            let lower_bid = &window[1];
            let gap_size = higher_bid.price - lower_bid.price;
            let gap_percentage = gap_size / lower_bid.price;

            if gap_percentage > gap_threshold {
                gaps.push((Side::Bid, lower_bid.price, higher_bid.price, i + 1));
            }
        }

        // Check ask side gaps
        let asks = book.get_asks(Some(20));
        for (i, window) in asks.windows(2).enumerate() {
            let lower_ask = &window[0];
            let higher_ask = &window[1];
            let gap_size = higher_ask.price - lower_ask.price;
            let gap_percentage = gap_size / lower_ask.price;

            if gap_percentage > gap_threshold {
                gaps.push((Side::Ask, lower_ask.price, higher_ask.price, i + 1));
            }
        }

        gaps
    }

    fn emit_event(&mut self, event: OrderBookEvent) {
        for handler in &mut self.handlers {
            handler.handle_event(event.clone());
        }
    }
}

impl Default for OrderBookEventProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// Example event handlers
#[derive(Debug)]
pub struct LoggingEventHandler;

impl OrderBookEventHandler for LoggingEventHandler {
    fn handle_event(&mut self, event: OrderBookEvent) {
        match event {
            OrderBookEvent::BestBidAskUpdate(evt) => {
                tracing::debug!(
                    "Best bid/ask update for {} on {}: bid={:?}, ask={:?}",
                    evt.symbol, evt.venue_id, evt.best_bid, evt.best_ask
                );
            }
            OrderBookEvent::SpreadUpdate(evt) => {
                tracing::debug!(
                    "Spread update for {} on {}: spread={:?}, bps={:?}",
                    evt.symbol, evt.venue_id, evt.spread, evt.spread_bps
                );
            }
            OrderBookEvent::CrossingDetected(evt) => {
                tracing::warn!(
                    "Order book crossing detected for {} on {}: severity={:?}, amount={}",
                    evt.symbol, evt.venue_id, evt.severity, evt.cross_amount
                );
            }
            OrderBookEvent::LiquidityGap(evt) => {
                tracing::info!(
                    "Liquidity gap detected for {} on {} {:?} side: {}% gap at level {}",
                    evt.symbol, evt.venue_id, evt.side, 
                    (evt.gap_size / evt.gap_start * Decimal::from(100)), evt.depth_level
                );
            }
            OrderBookEvent::PriceMovement(evt) => {
                tracing::debug!(
                    "Price movement for {} on {} {:?} side: {} -> {} ({} bps)",
                    evt.symbol, evt.venue_id, evt.side, evt.old_price, evt.new_price, evt.change_bps
                );
            }
            _ => {
                tracing::trace!("Order book event: {:?}", event);
            }
        }
    }
}

#[derive(Debug)]
pub struct MetricsEventHandler {
    pub event_counts: std::collections::HashMap<String, u64>,
}

impl MetricsEventHandler {
    pub fn new() -> Self {
        Self {
            event_counts: std::collections::HashMap::new(),
        }
    }

    pub fn get_event_count(&self, event_type: &str) -> u64 {
        self.event_counts.get(event_type).copied().unwrap_or(0)
    }
}

impl Default for MetricsEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBookEventHandler for MetricsEventHandler {
    fn handle_event(&mut self, event: OrderBookEvent) {
        let event_type = match event {
            OrderBookEvent::Snapshot(_) => "snapshot",
            OrderBookEvent::Update(_) => "update",
            OrderBookEvent::BestBidAskUpdate(_) => "best_bid_ask_update",
            OrderBookEvent::SpreadUpdate(_) => "spread_update",
            OrderBookEvent::VolumeUpdate(_) => "volume_update",
            OrderBookEvent::CrossingDetected(_) => "crossing_detected",
            OrderBookEvent::LiquidityGap(_) => "liquidity_gap",
            OrderBookEvent::PriceMovement(_) => "price_movement",
        };

        *self.event_counts.entry(event_type.to_string()).or_insert(0) += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_processor() {
        let mut processor = OrderBookEventProcessor::new();
        let mut logging_handler = LoggingEventHandler;
        let mut metrics_handler = MetricsEventHandler::new();

        processor.add_handler(Box::new(LoggingEventHandler));
        processor.add_handler(Box::new(MetricsEventHandler::new()));

        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);
        
        book.update_bid(Decimal::from(50000), Decimal::from(1), None);
        book.update_ask(Decimal::from(50001), Decimal::from(1), None);

        processor.process_book_update(VenueId::Binance, &book);

        // Update the book to trigger events
        book.update_bid(Decimal::from(50001), Decimal::from(1), None);
        processor.process_book_update(VenueId::Binance, &book);
    }

    #[test]
    fn test_crossing_detection() {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = FastOrderBook::new(symbol, None);
        let mut processor = OrderBookEventProcessor::new();
        
        book.update_bid(Decimal::from(50000), Decimal::from(1), None);
        book.update_ask(Decimal::from(49999), Decimal::from(1), None); // Create crossing

        processor.process_book_update(VenueId::Binance, &book);
    }
}