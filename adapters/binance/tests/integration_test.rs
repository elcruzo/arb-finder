use arbfinder_binance::BinanceAdapter;
use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

#[tokio::test]
async fn test_binance_connection() {
    let mut adapter = BinanceAdapter::new();
    
    // Test connection
    let result = adapter.connect().await;
    assert!(result.is_ok(), "Failed to connect to Binance: {:?}", result.err());
    assert!(adapter.is_connected().await, "Adapter should be connected");
}

#[tokio::test]
async fn test_binance_server_time() {
    let adapter = BinanceAdapter::new();
    
    let result = adapter.get_server_time().await;
    assert!(result.is_ok(), "Failed to get server time: {:?}", result.err());
    
    let server_time = result.unwrap();
    println!("Binance server time: {}", server_time);
    
    // Server time should be reasonable (within 1 hour of now)
    let now = chrono::Utc::now();
    let diff = (now - server_time).num_seconds().abs();
    assert!(diff < 3600, "Server time differs by more than 1 hour");
}

#[tokio::test]
async fn test_binance_ping() {
    let adapter = BinanceAdapter::new();
    
    let result = adapter.ping().await;
    assert!(result.is_ok(), "Failed to ping Binance: {:?}", result.err());
    
    let latency = result.unwrap();
    println!("Binance ping latency: {}ms", latency);
    
    // Latency should be reasonable (< 5 seconds)
    assert!(latency < 5000, "Ping latency too high: {}ms", latency);
}

#[tokio::test]
async fn test_get_symbols() {
    let adapter = BinanceAdapter::new();
    
    let result = adapter.get_symbols().await;
    assert!(result.is_ok(), "Failed to get symbols: {:?}", result.err());
    
    let symbols = result.unwrap();
    assert!(!symbols.is_empty(), "Should have at least one symbol");
    println!("Found {} symbols", symbols.len());
    
    // Check for known symbols
    let btc_usdt = symbols.iter().find(|s| s.base() == "BTC" && s.quote() == "USDT");
    assert!(btc_usdt.is_some(), "BTC/USDT should be available");
}

#[tokio::test]
async fn test_get_symbol_info() {
    let adapter = BinanceAdapter::new();
    let symbol = Symbol::new("BTC", "USDT");
    
    let result = adapter.get_symbol_info(&symbol).await;
    assert!(result.is_ok(), "Failed to get symbol info: {:?}", result.err());
    
    let info = result.unwrap();
    println!("BTC/USDT info: {:?}", info);
    assert_eq!(info.symbol.base(), "BTC");
    assert_eq!(info.symbol.quote(), "USDT");
    assert_eq!(info.status, "TRADING");
}

#[tokio::test]
async fn test_fetch_orderbook() {
    let adapter = BinanceAdapter::new();
    let symbol = Symbol::new("BTC", "USDT");
    
    let result = adapter.get_orderbook(&symbol, Some(20)).await;
    assert!(result.is_ok(), "Failed to fetch orderbook: {:?}", result.err());
    
    let orderbook = result.unwrap();
    
    // Verify orderbook has data
    let best_bid = orderbook.best_bid();
    let best_ask = orderbook.best_ask();
    
    assert!(best_bid.is_some(), "Orderbook should have at least one bid");
    assert!(best_ask.is_some(), "Orderbook should have at least one ask");
    
    let bid_price = best_bid.unwrap().price;
    let ask_price = best_ask.unwrap().price;
    
    println!("BTC/USDT Best Bid: {}, Best Ask: {}", bid_price, ask_price);
    
    // Sanity checks
    assert!(bid_price > Decimal::ZERO, "Bid price should be positive");
    assert!(ask_price > Decimal::ZERO, "Ask price should be positive");
    assert!(ask_price > bid_price, "Ask should be higher than bid");
    
    // Spread check - should be reasonable for BTC/USDT
    let spread = orderbook.spread().unwrap();
    let spread_bps = (spread / bid_price * Decimal::from(10000)).to_f64().unwrap_or(0.0);
    println!("Spread: {} ({:.2} bps)", spread, spread_bps);
    
    assert!(spread_bps < 100.0, "Spread should be less than 100bps for BTC/USDT");
}

#[tokio::test]
async fn test_orderbook_has_depth() {
    let adapter = BinanceAdapter::new();
    let symbol = Symbol::new("ETH", "USDT");
    
    let result = adapter.get_orderbook(&symbol, Some(100)).await;
    assert!(result.is_ok(), "Failed to fetch orderbook");
    
    let orderbook = result.unwrap();
    
    // Check we have multiple levels
    assert!(orderbook.bids.len() > 10, "Should have multiple bid levels");
    assert!(orderbook.asks.len() > 10, "Should have multiple ask levels");
    
    println!("ETH/USDT orderbook depth: {} bids, {} asks", 
             orderbook.bids.len(), orderbook.asks.len());
}

#[tokio::test]
async fn test_multiple_symbols_orderbooks() {
    let adapter = BinanceAdapter::new();
    let symbols = vec![
        Symbol::new("BTC", "USDT"),
        Symbol::new("ETH", "USDT"),
        Symbol::new("BNB", "USDT"),
    ];
    
    for symbol in symbols {
        let result = adapter.get_orderbook(&symbol, Some(10)).await;
        assert!(result.is_ok(), "Failed to fetch orderbook for {}", symbol.to_pair());
        
        let orderbook = result.unwrap();
        assert!(orderbook.best_bid().is_some());
        assert!(orderbook.best_ask().is_some());
        
        println!("{}: Bid={}, Ask={}", 
                 symbol.to_pair(),
                 orderbook.best_bid().unwrap().price,
                 orderbook.best_ask().unwrap().price);
    }
}
