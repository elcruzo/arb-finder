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
    pub side: OrderSide,
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
        let symbol_str = order.symbol.to_pair();
        match order.side {
            OrderSide::Buy => {
                // Lock quote currency (e.g., USDT for BTC/USDT)
                let quote_asset = self.extract_quote_asset(&symbol_str);
                let required_amount = order.price.unwrap_or_default() * order.quantity;
                self.lock_balance(&quote_asset, required_amount);
            }
            OrderSide::Sell => {
                // Lock base currency (e.g., BTC for BTC/USDT)
                let base_asset = self.extract_base_asset(&symbol_str);
                self.lock_balance(&base_asset, order.quantity);
            }
        }
        
        self.pending_orders.insert(order.id.to_string(), order);
        self.last_updated = Utc::now();
    }

    pub fn remove_pending_order(&mut self, order_id: &OrderId) {
        if let Some(order) = self.pending_orders.remove(&order_id.to_string()) {
            // Unlock funds
            let symbol_str = order.symbol.to_pair();
            match order.side {
                OrderSide::Buy => {
                    let quote_asset = self.extract_quote_asset(&symbol_str);
                    let locked_amount = order.price.unwrap_or_default() * order.remaining_quantity;
                    self.unlock_balance(&quote_asset, locked_amount);
                }
                OrderSide::Sell => {
                    let base_asset = self.extract_base_asset(&symbol_str);
                    self.unlock_balance(&base_asset, order.remaining_quantity);
                }
            }
        }
        self.last_updated = Utc::now();
    }

    pub fn update_order(&mut self, updated_order: Order) {
        let order_id_str = updated_order.id.to_string();
        let should_remove = updated_order.status == OrderStatus::Filled || 
                            updated_order.status == OrderStatus::Canceled;
        
        if let Some(existing_order) = self.pending_orders.get(&order_id_str) {
            let filled_diff = updated_order.filled_quantity - existing_order.filled_quantity;
            
            if filled_diff > Decimal::ZERO {
                // Process partial or full fill
                let symbol_str = updated_order.symbol.to_pair();
                let base_asset = self.extract_base_asset(&symbol_str);
                let quote_asset = self.extract_quote_asset(&symbol_str);
                
                match updated_order.side {
                    OrderSide::Buy => {
                        // Add base asset
                        self.add_balance(base_asset.clone(), filled_diff);
                        
                        // Unlock and remove quote asset
                        let quote_amount = updated_order.price.unwrap_or_default() * filled_diff;
                        self.unlock_balance(&quote_asset, quote_amount);
                        self.remove_balance(&quote_asset, quote_amount);
                    }
                    OrderSide::Sell => {
                        // Add quote asset
                        let quote_amount = updated_order.price.unwrap_or_default() * filled_diff;
                        self.add_balance(quote_asset.clone(), quote_amount);
                        
                        // Unlock and remove base asset
                        self.unlock_balance(&base_asset, filled_diff);
                        self.remove_balance(&base_asset, filled_diff);
                    }
                }
            }
        }
        
        // Update or insert the order
        self.pending_orders.insert(order_id_str.clone(), updated_order.clone());
        
        // Remove if fully filled or canceled
        if should_remove {
            self.remove_pending_order(&updated_order.id);
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
            // Calculate PnL directly to avoid borrowing issues
            let pnl = match position.side {
                OrderSide::Buy => (current_price - position.entry_price) * position.size,
                OrderSide::Sell => (position.entry_price - current_price) * position.size,
            };
            position.unrealized_pnl = pnl;
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

    fn calculate_unrealized_pnl_static(
        &self,
        side: OrderSide,
        size: Decimal,
        entry_price: Decimal,
        current_price: Decimal,
    ) -> Decimal {
        match side {
            OrderSide::Buy => (current_price - entry_price) * size,
            OrderSide::Sell => (entry_price - current_price) * size,
        }
    }

    fn calculate_pnl_for_trade(&mut self, trade: &Trade) {
        // Update or create position
        let trade_side = match trade.side {
            arbfinder_core::Side::Bid => OrderSide::Buy,
            arbfinder_core::Side::Ask => OrderSide::Sell,
        };
        let position = self.positions.entry(trade.symbol.to_pair()).or_insert(Position {
            symbol: trade.symbol.to_pair(),
            side: trade_side,
            size: Decimal::ZERO,
            entry_price: Decimal::ZERO,
            current_price: trade.price,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });

        // Simple position tracking (can be enhanced for more complex scenarios)
        match trade_side {
            OrderSide::Buy => {
                if position.side == OrderSide::Sell && position.size > Decimal::ZERO {
                    // Closing short position
                    let close_amount = trade.quantity.min(position.size);
                    let pnl = (position.entry_price - trade.price) * close_amount;
                    position.realized_pnl += pnl;
                    position.size -= close_amount;
                    
                    if position.size == Decimal::ZERO {
                        position.side = OrderSide::Buy;
                    }
                } else {
                    // Opening or adding to long position
                    let new_size = position.size + trade.quantity;
                    position.entry_price = ((position.entry_price * position.size) + (trade.price * trade.quantity)) / new_size;
                    position.size = new_size;
                    position.side = OrderSide::Buy;
                }
            }
            OrderSide::Sell => {
                if position.side == OrderSide::Buy && position.size > Decimal::ZERO {
                    // Closing long position
                    let close_amount = trade.quantity.min(position.size);
                    let pnl = (trade.price - position.entry_price) * close_amount;
                    position.realized_pnl += pnl;
                    position.size -= close_amount;
                    
                    if position.size == Decimal::ZERO {
                        position.side = OrderSide::Sell;
                    }
                } else {
                    // Opening or adding to short position
                    let new_size = position.size + trade.quantity;
                    position.entry_price = ((position.entry_price * position.size) + (trade.price * trade.quantity)) / new_size;
                    position.size = new_size;
                    position.side = OrderSide::Sell;
                }
            }
        }
        
        position.current_price = trade.price;
        position.updated_at = Utc::now();
    }

    fn calculate_unrealized_pnl(&self, position: &Position) -> Decimal {
        match position.side {
            OrderSide::Buy => (position.current_price - position.entry_price) * position.size,
            OrderSide::Sell => (position.entry_price - position.current_price) * position.size,
        }
    }
}

impl Default for Portfolio {
    fn default() -> Self {
        Self::new()
    }
}