//! Cross-Exchange Arbitrage Detection
//!
//! Detects price discrepancies across multiple exchanges

use std::collections::HashMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tracing::{info, debug};

use arbfinder_core::prelude::*;

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub symbol: Symbol,
    pub buy_venue: VenueId,
    pub sell_venue: VenueId,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub profit_percentage: Decimal,
    pub max_volume: Decimal,
    pub estimated_profit: Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ArbitrageOpportunity {
    pub fn calculate_net_profit(&self, trading_fees: &TradingFeePair) -> Decimal {
        let gross_profit = self.sell_price - self.buy_price;
        let buy_fee = self.buy_price * trading_fees.buy_exchange_fee;
        let sell_fee = self.sell_price * trading_fees.sell_exchange_fee;
        let net_profit = gross_profit - buy_fee - sell_fee;
        net_profit
    }
}

#[derive(Debug, Clone)]
pub struct TradingFeePair {
    pub buy_exchange_fee: Decimal, // as decimal (0.001 = 0.1%)
    pub sell_exchange_fee: Decimal,
}

pub struct CrossExchangeArbitrageDetector {
    min_profit_threshold: Decimal, // Minimum profit percentage
    min_volume_threshold: Decimal, // Minimum volume in quote currency
    trading_fees: HashMap<VenueId, Decimal>, // Default trading fees per exchange
}

impl CrossExchangeArbitrageDetector {
    pub fn new(min_profit_bps: i32, min_volume: Decimal) -> Self {
        let mut trading_fees = HashMap::new();
        // Default fees (0.1% = 10 bps)
        trading_fees.insert(VenueId::Binance, Decimal::new(1, 3));
        trading_fees.insert(VenueId::Coinbase, Decimal::new(5, 3));
        trading_fees.insert(VenueId::Kraken, Decimal::new(26, 4));
        
        Self {
            min_profit_threshold: Decimal::new(min_profit_bps as i64, 4), // Convert bps to decimal
            min_volume_threshold: min_volume,
            trading_fees,
        }
    }

    /// Detect arbitrage opportunities across multiple orderbooks
    pub fn detect_opportunities(
        &self,
        symbol: &Symbol,
        orderbooks: &HashMap<VenueId, &OrderBook>,
    ) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        // Compare all pairs of exchanges
        let venues: Vec<&VenueId> = orderbooks.keys().collect();
        
        for i in 0..venues.len() {
            for j in (i + 1)..venues.len() {
                let venue_a = venues[i];
                let venue_b = venues[j];
                
                let book_a = orderbooks.get(venue_a).unwrap();
                let book_b = orderbooks.get(venue_b).unwrap();
                
                // Check A->B direction
                if let Some(opp) = self.check_arbitrage_direction(
                    symbol,
                    venue_a.clone(),
                    venue_b.clone(),
                    book_a,
                    book_b,
                ) {
                    opportunities.push(opp);
                }
                
                // Check B->A direction
                if let Some(opp) = self.check_arbitrage_direction(
                    symbol,
                    venue_b.clone(),
                    venue_a.clone(),
                    book_b,
                    book_a,
                ) {
                    opportunities.push(opp);
                }
            }
        }

        opportunities
    }

    fn check_arbitrage_direction(
        &self,
        symbol: &Symbol,
        buy_venue: VenueId,
        sell_venue: VenueId,
        buy_book: &OrderBook,
        sell_book: &OrderBook,
    ) -> Option<ArbitrageOpportunity> {
        let best_ask = buy_book.best_ask()?;
        let best_bid = sell_book.best_bid()?;
        
        let buy_price = best_ask.price;
        let sell_price = best_bid.price;
        
        // No arbitrage if sell price <= buy price
        if sell_price <= buy_price {
            return None;
        }
        
        // Calculate gross profit percentage
        let gross_profit_pct = ((sell_price - buy_price) / buy_price) * Decimal::from(10000); // in bps
        
        // Calculate fees
        let buy_fee = self.trading_fees.get(&buy_venue)
            .copied()
            .unwrap_or(Decimal::new(1, 3));
        let sell_fee = self.trading_fees.get(&sell_venue)
            .copied()
            .unwrap_or(Decimal::new(1, 3));
        
        // Calculate net profit after fees
        let fee_pct = (buy_fee + sell_fee) * Decimal::from(10000); // Convert to bps
        let net_profit_pct = gross_profit_pct - fee_pct;
        
        // Check if profit meets threshold
        if net_profit_pct < self.min_profit_threshold * Decimal::from(10000) {
            return None;
        }
        
        // Calculate maximum volume (limited by available liquidity)
        let max_volume = best_ask.quantity.min(best_bid.quantity);
        let volume_value = max_volume * buy_price;
        
        // Check if volume meets threshold
        if volume_value < self.min_volume_threshold {
            return None;
        }
        
        // Calculate estimated profit
        let net_profit_per_unit = (sell_price - buy_price) - (buy_price * buy_fee) - (sell_price * sell_fee);
        let estimated_profit = net_profit_per_unit * max_volume;
        
        debug!(
            "Found arbitrage: Buy {} on {:?} @ {}, Sell on {:?} @ {}, Profit: {:.2} bps, Volume: {}",
            symbol.to_pair(), buy_venue, buy_price, sell_venue, sell_price,
            net_profit_pct.to_f64().unwrap_or(0.0), max_volume
        );
        
        Some(ArbitrageOpportunity {
            symbol: symbol.clone(),
            buy_venue,
            sell_venue,
            buy_price,
            sell_price,
            profit_percentage: net_profit_pct / Decimal::from(10000), // Convert back from bps
            max_volume,
            estimated_profit,
            timestamp: chrono::Utc::now(),
        })
    }

    pub fn set_trading_fee(&mut self, venue: VenueId, fee: Decimal) {
        self.trading_fees.insert(venue, fee);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_orderbook(best_bid_price: Decimal, best_ask_price: Decimal, quantity: Decimal) -> OrderBook {
        let symbol = Symbol::new("BTC", "USDT");
        let mut book = OrderBook::new(symbol);
        book.update_bid(best_bid_price, quantity);
        book.update_ask(best_ask_price, quantity);
        book
    }

    #[test]
    fn test_detect_simple_arbitrage() {
        let detector = CrossExchangeArbitrageDetector::new(10, dec!(100)); // 10 bps min profit, $100 min volume
        
        let symbol = Symbol::new("BTC", "USDT");
        let mut orderbooks = HashMap::new();
        
        // Exchange A: Lower prices (buy here)
        let book_a = create_test_orderbook(dec!(100), dec!(101), dec!(1.0));
        orderbooks.insert(VenueId::Binance, &book_a);
        
        // Exchange B: Higher prices (sell here)
        let book_b = create_test_orderbook(dec!(102), dec!(103), dec!(1.0));
        orderbooks.insert(VenueId::Coinbase, &book_b);
        
        let opportunities = detector.detect_opportunities(&symbol, &orderbooks);
        
        // Should find one opportunity: buy on A (ask=101), sell on B (bid=102)
        assert!(!opportunities.is_empty(), "Should find at least one arbitrage opportunity");
        
        let opp = &opportunities[0];
        assert_eq!(opp.buy_venue, VenueId::Binance);
        assert_eq!(opp.sell_venue, VenueId::Coinbase);
        assert_eq!(opp.buy_price, dec!(101));
        assert_eq!(opp.sell_price, dec!(102));
        
        println!("Found arbitrage: {:?}", opp);
    }

    #[test]
    fn test_no_arbitrage_when_prices_equal() {
        let detector = CrossExchangeArbitrageDetector::new(10, dec!(100));
        
        let symbol = Symbol::new("BTC", "USDT");
        let mut orderbooks = HashMap::new();
        
        // Same prices on both exchanges
        let book_a = create_test_orderbook(dec!(100), dec!(101), dec!(1.0));
        let book_b = create_test_orderbook(dec!(100), dec!(101), dec!(1.0));
        
        orderbooks.insert(VenueId::Binance, &book_a);
        orderbooks.insert(VenueId::Coinbase, &book_b);
        
        let opportunities = detector.detect_opportunities(&symbol, &orderbooks);
        
        assert!(opportunities.is_empty(), "Should not find arbitrage with equal prices");
    }

    #[test]
    fn test_no_arbitrage_below_profit_threshold() {
        let detector = CrossExchangeArbitrageDetector::new(100, dec!(100)); // 100 bps = 1% threshold
        
        let symbol = Symbol::new("BTC", "USDT");
        let mut orderbooks = HashMap::new();
        
        // Small price difference (only 0.5%)
        let book_a = create_test_orderbook(dec!(100), dec!(100), dec!(1.0));
        let book_b = create_test_orderbook(dec!(100.5), dec!(100.5), dec!(1.0));
        
        orderbooks.insert(VenueId::Binance, &book_a);
        orderbooks.insert(VenueId::Coinbase, &book_b);
        
        let opportunities = detector.detect_opportunities(&symbol, &orderbooks);
        
        assert!(opportunities.is_empty(), "Should not find arbitrage below profit threshold");
    }

    #[test]
    fn test_volume_limit() {
        let detector = CrossExchangeArbitrageDetector::new(10, dec!(100));
        
        let symbol = Symbol::new("BTC", "USDT");
        let mut orderbooks = HashMap::new();
        
        // Good price spread but limited volume
        let book_a = create_test_orderbook(dec!(100), dec!(101), dec!(0.5)); // Only 0.5 BTC
        let book_b = create_test_orderbook(dec!(105), dec!(106), dec!(2.0));
        
        orderbooks.insert(VenueId::Binance, &book_a);
        orderbooks.insert(VenueId::Coinbase, &book_b);
        
        let opportunities = detector.detect_opportunities(&symbol, &orderbooks);
        
        if let Some(opp) = opportunities.first() {
            assert!(opp.max_volume <= dec!(0.5), "Volume should be limited by smaller side");
            println!("Max volume: {}", opp.max_volume);
        }
    }

    #[test]
    fn test_fees_reduce_profit() {
        let mut detector = CrossExchangeArbitrageDetector::new(10, dec!(100));
        
        // Set high fees
        detector.set_trading_fee(VenueId::Binance, dec!(0.005)); // 0.5%
        detector.set_trading_fee(VenueId::Coinbase, dec!(0.005)); // 0.5%
        
        let symbol = Symbol::new("BTC", "USDT");
        let mut orderbooks = HashMap::new();
        
        // 1.5% price difference - but 1% in fees
        let book_a = create_test_orderbook(dec!(100), dec!(100), dec!(1.0));
        let book_b = create_test_orderbook(dec!(101.5), dec!(101.5), dec!(1.0));
        
        orderbooks.insert(VenueId::Binance, &book_a);
        orderbooks.insert(VenueId::Coinbase, &book_b);
        
        let opportunities = detector.detect_opportunities(&symbol, &orderbooks);
        
        if let Some(opp) = opportunities.first() {
            // Profit should be around 0.5% after 1% fees
            let profit_pct = opp.profit_percentage.to_f64().unwrap();
            println!("Net profit: {:.4}%", profit_pct * 100.0);
            assert!(profit_pct < 0.01, "Profit should be less than 1% after fees");
            assert!(profit_pct > 0.0, "Should still be profitable");
        }
    }
}
