use std::collections::HashMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use arbfinder_core::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub balances: HashMap<String, Balance>,
    pub positions: HashMap<String, Position>,
    pub pending_orders: HashMap<String, Order>,
    pub trades: Vec<Trade>,
    pub pnl: Decimal,
    pub total_value: Decimal,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub total: Decimal,
    pub available: Decimal,
    pub locked: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: Side,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Portfolio {
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
            positions: HashMap::new(),
            pending_orders: HashMap::new(),
            trades: Vec::new(),
            pnl: Decimal::ZERO,
            total_value: Decimal::ZERO,
            last_updated: Utc::now(),
        }
    }

    pub fn add_balance(&mut self, asset: String, amount: Decimal) {
        let balance = self.balances.entry(asset.clone()).or_insert(Balance {
            asset: asset.clone(),
            total: Decimal::ZERO,
            available: Decimal::ZERO,
            locked: Decimal::ZERO,
        });
        
        balance.total += amount;
        balance.available += amount;
        self.last_updated = Utc::now();
    }

    pub fn update_balance(&mut self, asset: String, total: Decimal, available: Decimal, locked: Decimal) {
        let balance = self.balances.entry(asset.clone()).or_insert(Balance {
            asset: asset.clone(),
            total: Decimal::ZERO,
            available: Decimal::ZERO,
            locked: Decimal::ZERO,
        });
        
        balance.total = total;
        balance.available = available;
        balance.locked = locked;
        self.last_updated = Utc::now();
    }

    pub fn get_balance(&self, asset: &str) -> Option<&Balance> {
        self.balances.get(asset)
    }

    pub fn get_available_balance(&self, asset: &str) -> Decimal {
        self.balances.get(asset)
            .map(|b| b.available)
            .unwrap_or(Decimal::ZERO)
    }

    pub fn add_pending_order(&mut self, order: Order) {
        // Lock funds for the order
        match order.side {
            Side::Buy => {
                // Lock quote currency (e.g., USDT for BTC/USDT)
                let quote_asset = self.extract_quote_asset(&order.symbol);
                let required_amount = order.price * order.amount;
                self.lock_balance(&quote_asset, required_amount);
            }
            Side::Sell => {
                // Lock base currency (e.g., BTC for BTC/USDT)
                let base_asset = self.extract_base_asset(&order.symbol);
                self.lock_balance(&base_asset, order.amount);
            }
        }
        
        self.pending_orders.insert(order.id.clone(), order);
        self.last_updated = Utc::now();
    }

    pub fn remove_pending_order(&mut self, order_id: &str) {
        if let Some(order) = self.pending_orders.remove(order_id) {
            // Unlock funds
            match order.side {
                Side::Buy => {
                    let quote_asset = self.extract_quote_asset(&order.symbol);
                    let locked_amount = order.price * (order.amount - order.filled_amount);
                    self.unlock_balance(&quote_asset, locked_amount);
                }
                Side::Sell => {
                    let base_asset = self.extract_base_asset(&order.symbol);
                    let locked_amount = order.amount - order.filled_amount;
                    self.unlock_balance(&base_asset, locked_amount);
                }
            }
        }
        self.last_updated = Utc::now();
    }

    pub fn update_order(&mut self, updated_order: Order) {
        if let Some(existing_order) = self.pending_orders.get_mut(&updated_order.id) {
            let filled_diff = updated_order.filled_amount - existing_order.filled_amount;
            
            if filled_diff > Decimal::ZERO {
                // Process partial or full fill
                match updated_order.side {
                    Side::Buy => {
                        let base_asset = self.extract_base_asset(&updated_order.symbol);
                        let quote_asset = self.extract_quote_asset(&updated_order.symbol);
                        
                        // Add base asset
                        self.add_balance(base_asset, filled_diff);
                        
                        // Unlock and remove quote asset
                        let quote_amount = updated_order.price * filled_diff;
                        self.unlock_balance(&quote_asset, quote_amount);
                        self.remove_balance(&quote_asset, quote_amount);
                    }
                    Side::Sell => {
                        let base_asset = self.extract_base_asset(&updated_order.symbol);
                        let quote_asset = self.extract_quote_asset(&updated_order.symbol);
                        
                        // Add quote asset
                        let quote_amount = updated_order.price * filled_diff;
                        self.add_balance(quote_asset, quote_amount);
                        
                        // Unlock and remove base asset
                        self.unlock_balance(&base_asset, filled_diff);
                        self.remove_balance(&base_asset, filled_diff);
                    }
                }
            }
            
            *existing_order = updated_order.clone();
            
            // Remove if fully filled or canceled
            if updated_order.status == OrderStatus::Filled || 
               updated_order.status == OrderStatus::Canceled {
                self.remove_pending_order(&updated_order.id);
            }
        }
        self.last_updated = Utc::now();
    }

    pub fn add_trade(&mut self, trade: Trade) {
        // Update PnL calculation
        self.calculate_pnl_for_trade(&trade);
        self.trades.push(trade);
        self.last_updated = Utc::now();
    }

    pub fn update_position_price(&mut self, symbol: &str, current_price: Decimal) {
        if let Some(position) = self.positions.get_mut(symbol) {
            position.current_price = current_price;
            position.unrealized_pnl = self.calculate_unrealized_pnl(position);
            position.updated_at = Utc::now();
        }
        self.last_updated = Utc::now();
    }

    pub fn get_total_value(&self, prices: &HashMap<String, Decimal>) -> Decimal {
        let mut total = Decimal::ZERO;
        
        for balance in self.balances.values() {
            if balance.asset == "USDT" || balance.asset == "USD" {
                total += balance.total;
            } else if let Some(price) = prices.get(&format!("{}USDT", balance.asset)) {
                total += balance.total * price;
            }
        }
        
        total
    }

    pub fn get_unrealized_pnl(&self) -> Decimal {
        self.positions.values()
            .map(|p| p.unrealized_pnl)
            .sum()
    }

    pub fn get_realized_pnl(&self) -> Decimal {
        self.positions.values()
            .map(|p| p.realized_pnl)
            .sum()
    }

    fn lock_balance(&mut self, asset: &str, amount: Decimal) {
        if let Some(balance) = self.balances.get_mut(asset) {
            if balance.available >= amount {
                balance.available -= amount;
                balance.locked += amount;
            }
        }
    }

    fn unlock_balance(&mut self, asset: &str, amount: Decimal) {
        if let Some(balance) = self.balances.get_mut(asset) {
            balance.locked -= amount.min(balance.locked);
            balance.available += amount.min(balance.locked);
        }
    }

    fn remove_balance(&mut self, asset: &str, amount: Decimal) {
        if let Some(balance) = self.balances.get_mut(asset) {
            balance.total -= amount.min(balance.total);
        }
    }

    fn extract_base_asset(&self, symbol: &str) -> String {
        // Simple implementation - assumes format like "BTCUSDT"
        if symbol.ends_with("USDT") {
            symbol[..symbol.len() - 4].to_string()
        } else if symbol.ends_with("USD") {
            symbol[..symbol.len() - 3].to_string()
        } else if symbol.contains('/') {
            symbol.split('/').next().unwrap_or(symbol).to_string()
        } else {
            // Fallback - take first 3 characters
            symbol[..3.min(symbol.len())].to_string()
        }
    }

    fn extract_quote_asset(&self, symbol: &str) -> String {
        // Simple implementation - assumes format like "BTCUSDT"
        if symbol.ends_with("USDT") {
            "USDT".to_string()
        } else if symbol.ends_with("USD") {
            "USD".to_string()
        } else if symbol.contains('/') {
            symbol.split('/').nth(1).unwrap_or("USDT").to_string()
        } else {
            "USDT".to_string()
        }
    }

    fn calculate_pnl_for_trade(&mut self, trade: &Trade) {
        // Update or create position
        let position = self.positions.entry(trade.symbol.clone()).or_insert(Position {
            symbol: trade.symbol.clone(),
            side: trade.side,
            size: Decimal::ZERO,
            entry_price: Decimal::ZERO,
            current_price: trade.price,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });

        // Simple position tracking (can be enhanced for more complex scenarios)
        match trade.side {
            Side::Buy => {
                if position.side == Side::Sell && position.size > Decimal::ZERO {
                    // Closing short position
                    let close_amount = trade.amount.min(position.size);
                    let pnl = (position.entry_price - trade.price) * close_amount;
                    position.realized_pnl += pnl;
                    position.size -= close_amount;
                    
                    if position.size == Decimal::ZERO {
                        position.side = Side::Buy;
                    }
                } else {
                    // Opening or adding to long position
                    let new_size = position.size + trade.amount;
                    position.entry_price = ((position.entry_price * position.size) + (trade.price * trade.amount)) / new_size;
                    position.size = new_size;
                    position.side = Side::Buy;
                }
            }
            Side::Sell => {
                if position.side == Side::Buy && position.size > Decimal::ZERO {
                    // Closing long position
                    let close_amount = trade.amount.min(position.size);
                    let pnl = (trade.price - position.entry_price) * close_amount;
                    position.realized_pnl += pnl;
                    position.size -= close_amount;
                    
                    if position.size == Decimal::ZERO {
                        position.side = Side::Sell;
                    }
                } else {
                    // Opening or adding to short position
                    let new_size = position.size + trade.amount;
                    position.entry_price = ((position.entry_price * position.size) + (trade.price * trade.amount)) / new_size;
                    position.size = new_size;
                    position.side = Side::Sell;
                }
            }
        }
        
        position.current_price = trade.price;
        position.updated_at = Utc::now();
    }

    fn calculate_unrealized_pnl(&self, position: &Position) -> Decimal {
        match position.side {
            Side::Buy => (position.current_price - position.entry_price) * position.size,
            Side::Sell => (position.entry_price - position.current_price) * position.size,
        }
    }
}

impl Default for Portfolio {
    fn default() -> Self {
        Self::new()
    }
}