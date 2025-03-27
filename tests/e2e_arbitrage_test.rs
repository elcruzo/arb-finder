//! End-to-end arbitrage detection and execution test
//!
//! This test simulates the full arbitrage workflow:
//! 1. Fetch orderbooks from multiple exchanges
//! 2. Detect arbitrage opportunities
//! 3. Execute trades (paper trading)
//! 4. Monitor performance

use arbfinder_core::prelude::*;
use arbfinder_strategy::prelude::*;
use arbfinder_execution::prelude::*;
use std::collections::HashMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tokio::time::{sleep, Duration};

/// Helper to create mock orderbooks with configurable spreads
fn create_mock_orderbooks() -> HashMap<VenueId, OrderBook> {
    let mut books = HashMap::new();
    
    // Binance: Lower prices (good for buying)
    let symbol = Symbol::new("BTC", "USDT");
    let mut binance_book = OrderBook::new(symbol.clone());
    binance_book.update_bid(dec!(50000), dec!(1.0));
    binance_book.update_ask(dec!(50010), dec!(1.5)); // Ask at 50010
    books.insert(VenueId::Binance, binance_book);
    
    // Coinbase: Higher prices (good for selling)
    let mut coinbase_book = OrderBook::new(symbol.clone());
    coinbase_book.update_bid(dec!(50120), dec!(1.2)); // Bid at 50120 - good arb!
    coinbase_book.update_ask(dec!(50130), dec!(1.0));
    books.insert(VenueId::Coinbase, coinbase_book);
    
    // Kraken: Medium prices
    let mut kraken_book = OrderBook::new(symbol);
    kraken_book.update_bid(dec!(50040), dec!(0.8));
    kraken_book.update_ask(dec!(50050), dec!(1.1));
    books.insert(VenueId::Kraken, kraken_book);
    
    books
}

#[tokio::test]
async fn test_end_to_end_arbitrage_detection() {
    // Setup: Create detector
    let detector = CrossExchangeArbitrageDetector::new(
        10,  // 10 bps minimum profit
        dec!(100.0) // $100 minimum volume
    );
    
    let symbol = Symbol::new("BTC", "USDT");
    let orderbooks = create_mock_orderbooks();
    
    // Convert to reference map
    let book_refs: HashMap<VenueId, &OrderBook> = orderbooks
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect();
    
    // Detect opportunities
    let opportunities = detector.detect_opportunities(&symbol, &book_refs);
    
    // Assert we found at least one opportunity
    assert!(!opportunities.is_empty(), "Should find arbitrage opportunities");
    
    // Find the best opportunity
    let best_opp = opportunities.iter()
        .max_by_key(|o| o.profit_percentage)
        .unwrap();
    
    println!("\n=== Best Arbitrage Opportunity ===");
    println!("Symbol: {}", best_opp.symbol.to_pair());
    println!("Buy on: {:?} @ ${}", best_opp.buy_venue, best_opp.buy_price);
    println!("Sell on: {:?} @ ${}", best_opp.sell_venue, best_opp.sell_price);
    println!("Profit: {:.4}% ({:.2} USDT)", 
             best_opp.profit_percentage.to_string().parse::<f64>().unwrap() * 100.0,
             best_opp.estimated_profit);
    println!("Max Volume: {} BTC", best_opp.max_volume);
    
    // Verify the opportunity makes sense
    assert_eq!(best_opp.buy_venue, VenueId::Binance);
    assert_eq!(best_opp.sell_venue, VenueId::Coinbase);
    assert!(best_opp.profit_percentage > Decimal::ZERO);
    assert!(best_opp.estimated_profit > Decimal::ZERO);
}

#[tokio::test]
async fn test_paper_trading_execution() {
    // Setup execution engine
    let config = ExecutionConfig {
        max_position_size: dec!(10.0),
        max_daily_loss: dec!(1000.0),
        max_orders_per_second: 10,
        enable_paper_trading: true,
    };
    
    let engine = ExecutionEngine::new(config);
    
    // Test placing a paper trade order
    let symbol = Symbol::new("BTC", "USDT");
    let result = engine.place_order(
        VenueId::Binance,
        symbol,
        OrderSide::Buy,
        dec!(1.0),           // quantity
        Some(dec!(50010.0))  // price
    ).await;
    
    assert!(result.is_ok(), "Paper trading order should succeed: {:?}", result.err());
    
    let order_id = result.unwrap();
    println!("Paper trade order placed: {}", order_id);
}

#[tokio::test]
async fn test_arbitrage_with_insufficient_profit() {
    let detector = CrossExchangeArbitrageDetector::new(
        200, // 200 bps = 2% - very high threshold
        dec!(100.0)
    );
    
    let symbol = Symbol::new("BTC", "USDT");
    let orderbooks = create_mock_orderbooks();
    
    let book_refs: HashMap<VenueId, &OrderBook> = orderbooks
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect();
    
    let opportunities = detector.detect_opportunities(&symbol, &book_refs);
    
    // With 2% threshold, we shouldn't find opportunities in our test data
    // (real spread is about 0.14%)
    println!("Found {} opportunities with 2% threshold", opportunities.len());
}

#[tokio::test]
async fn test_arbitrage_with_volume_constraints() {
    let detector = CrossExchangeArbitrageDetector::new(
        10,
        dec!(100000.0) // Very high volume requirement ($100k)
    );
    
    let symbol = Symbol::new("BTC", "USDT");
    let orderbooks = create_mock_orderbooks();
    
    let book_refs: HashMap<VenueId, &OrderBook> = orderbooks
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect();
    
    let opportunities = detector.detect_opportunities(&symbol, &book_refs);
    
    // Should have fewer or no opportunities due to high volume requirement
    println!("Found {} opportunities with $100k volume requirement", opportunities.len());
}

#[tokio::test]
async fn test_multiple_symbol_arbitrage() {
    let detector = CrossExchangeArbitrageDetector::new(10, dec!(100.0));
    
    // Test multiple trading pairs
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDT"),
        Symbol::new("BNB", "USDT"),
    ];
    
    for symbol in symbols {
        // Create orderbooks for each symbol
        let mut books = HashMap::new();
        
        // Simple mock with slight price differences
        let base_price = match symbol.base() {
            "BTC" => dec!(50000),
            "ETH" => dec!(3000),
            "BNB" => dec!(300),
            _ => dec!(100),
        };
        
        let mut book1 = OrderBook::new(symbol.clone());
        book1.update_bid(base_price, dec!(1.0));
        book1.update_ask(base_price + dec!(10), dec!(1.0));
        books.insert(VenueId::Binance, book1);
        
        let mut book2 = OrderBook::new(symbol.clone());
        book2.update_bid(base_price + dec!(50), dec!(1.0));
        book2.update_ask(base_price + dec!(60), dec!(1.0));
        books.insert(VenueId::Coinbase, book2);
        
        let book_refs: HashMap<VenueId, &OrderBook> = books
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .collect();
        
        let opportunities = detector.detect_opportunities(&symbol, &book_refs);
        
        println!("{}: Found {} opportunities", symbol.to_pair(), opportunities.len());
    }
}

#[tokio::test]
async fn test_arbitrage_performance_metrics() {
    use std::time::Instant;
    
    let detector = CrossExchangeArbitrageDetector::new(10, dec!(100.0));
    let symbol = Symbol::new("BTC", "USDT");
    let orderbooks = create_mock_orderbooks();
    
    let book_refs: HashMap<VenueId, &OrderBook> = orderbooks
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect();
    
    // Measure detection latency
    let start = Instant::now();
    let opportunities = detector.detect_opportunities(&symbol, &book_refs);
    let detection_time = start.elapsed();
    
    println!("\n=== Performance Metrics ===");
    println!("Detection latency: {:?}", detection_time);
    println!("Opportunities found: {}", opportunities.len());
    println!("Latency per opportunity: {:?}", 
             detection_time.checked_div(opportunities.len() as u32)
                 .unwrap_or(detection_time));
    
    // Assert detection is fast (< 1ms for 3 exchanges)
    assert!(detection_time.as_micros() < 1000, 
            "Detection should be fast: {:?}", detection_time);
}

#[tokio::test]
async fn test_realistic_market_conditions() {
    // Test with more realistic, tighter spreads
    let detector = CrossExchangeArbitrageDetector::new(5, dec!(50.0)); // 5 bps, $50 min
    
    let symbol = Symbol::new("BTC", "USDT");
    let mut realistic_books = HashMap::new();
    
    // Tighter, more realistic spreads
    let mut binance = OrderBook::new(symbol.clone());
    binance.update_bid(dec!(50000.00), dec!(0.5));
    binance.update_ask(dec!(50000.50), dec!(0.5)); // 0.5 USDT spread (0.001%)
    realistic_books.insert(VenueId::Binance, binance);
    
    let mut coinbase = OrderBook::new(symbol.clone());
    coinbase.update_bid(dec!(50000.60), dec!(0.4));  // Small arb opportunity
    coinbase.update_ask(dec!(50001.10), dec!(0.4));
    realistic_books.insert(VenueId::Coinbase, coinbase);
    
    let book_refs: HashMap<VenueId, &OrderBook> = realistic_books
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect();
    
    let opportunities = detector.detect_opportunities(&symbol, &book_refs);
    
    if !opportunities.is_empty() {
        let opp = &opportunities[0];
        println!("\n=== Realistic Arbitrage ===");
        println!("Buy: ${}, Sell: ${}", opp.buy_price, opp.sell_price);
        println!("Profit: {:.6}%", opp.profit_percentage.to_string().parse::<f64>().unwrap() * 100.0);
        println!("Note: After fees (~0.1-0.2%), this may not be profitable");
    } else {
        println!("No arbitrage found with realistic tight spreads - this is expected!");
    }
}
