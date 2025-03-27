use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use rust_decimal::Decimal;
use tracing::{info, warn};

use arbfinder_core::prelude::*;
use arbfinder_exchange::prelude::*;
use arbfinder_orderbook::FastOrderBook;
use crate::Strategy;

pub struct TriangularArbitrage {
    exchange: String,
    base_currency: String,
    min_profit_threshold: Decimal,
    market_data: HashMap<String, Arc<FastOrderBook>>,
}

impl TriangularArbitrage {
    pub fn new(exchange: String, base_currency: String, min_profit_threshold: Decimal) -> Self {
        Self {
            exchange,
            base_currency,
            min_profit_threshold,
            market_data: HashMap::new(),
        }
    }

    fn find_triangular_opportunities(&self) -> Vec<(Vec<String>, Decimal)> {
        let mut opportunities = Vec::new();
        
        // Get all markets that involve our base currency
        let base_markets: Vec<_> = self.market_data.keys()
            .filter(|symbol| {
                let parts: Vec<_> = symbol.split('_').collect();
                parts[0] == self.base_currency || parts[1] == self.base_currency
            })
            .collect();

        // For each base market, look for connected pairs that form a triangle
        for &base_market in &base_markets {
            let base_parts: Vec<_> = base_market.split('_').collect();
            let other_currency = if base_parts[0] == self.base_currency {
                base_parts[1]
            } else {
                base_parts[0]
            };

            // Find markets that connect with our other currency
            let connected_markets: Vec<_> = self.market_data.keys()
                .filter(|&symbol| {
                    let parts: Vec<_> = symbol.split('_').collect();
                    (parts[0] == other_currency || parts[1] == other_currency) &&
                    symbol != base_market
                })
                .collect();

            for &second_market in &connected_markets {
                let second_parts: Vec<_> = second_market.split('_').collect();
                let third_currency = if second_parts[0] == other_currency {
                    second_parts[1]
                } else {
                    second_parts[0]
                };

                // Find the closing market that connects back to base currency
                let closing_market = format!("{}_{}", self.base_currency, third_currency);
                if self.market_data.contains_key(&closing_market) {
                    if let Some(profit) = self.calculate_profit(&[
                        base_market.to_string(),
                        second_market.to_string(),
                        closing_market.clone(),
                    ]) {
                        if profit > self.min_profit_threshold {
                            opportunities.push((
                                vec![
                                    base_market.to_string(),
                                    second_market.to_string(),
                                    closing_market,
                                ],
                                profit,
                            ));
                        }
                    }
                }
            }
        }

        opportunities
    }

    fn calculate_profit(&self, path: &[String]) -> Option<Decimal> {
        let mut amount = Decimal::ONE; // Start with 1 unit of base currency

        for market in path {
            let orderbook = self.market_data.get(market)?;
            let parts: Vec<_> = market.split('_').collect();
            
            // Determine if we're buying or selling based on the market direction
            let is_buying = parts[0] == self.base_currency;
            
            if is_buying {
                if let Some(level) = orderbook.best_ask() {
                    amount = amount / level.price;
                } else {
                    return None;
                }
            } else {
                if let Some(level) = orderbook.best_bid() {
                    amount = amount * level.price;
                } else {
                    return None;
                }
            }
        }

        // Calculate profit percentage
        Some((amount - Decimal::ONE) * Decimal::from(100))
    }
}

#[async_trait]
impl Strategy for TriangularArbitrage {
    fn name(&self) -> String {
        "TriangularArbitrage".to_string()
    }

    async fn on_tick(&mut self, symbol: &Symbol, _ticker: &Ticker, orderbook: Arc<FastOrderBook>) {
        // Update market data
        self.market_data.insert(symbol.to_pair(), orderbook);

        // Look for opportunities
        let opportunities = self.find_triangular_opportunities();
        
        for (path, profit) in opportunities {
            info!(
                "Found triangular arbitrage opportunity: Path: {:?}, Profit: {}%",
                path, profit
            );
        }
    }

    async fn on_order(&mut self, order: &Order) {
        info!("Order update: {:?}", order);
    }

    async fn on_trade(&mut self, trade: &Trade) {
        info!("Trade executed: {:?}", trade);
    }
}