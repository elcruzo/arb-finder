use std::collections::HashMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use chrono::{DateTime, Utc};
use chrono::Duration as ChronoDuration;
use tracing::warn;

use arbfinder_core::prelude::*;

#[derive(Debug, Clone)]
pub struct RiskConfig {
    pub max_position_size: Decimal,
    pub max_daily_loss: Decimal,
    pub max_drawdown: Decimal,
    pub max_leverage: Decimal,
    pub max_orders_per_minute: u32,
    pub max_order_size: Decimal,
    pub min_order_size: Decimal,
    pub allowed_symbols: Vec<String>,
    pub blocked_symbols: Vec<String>,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_size: Decimal::from(100000), // $100k max position
            max_daily_loss: Decimal::from(10000),     // $10k max daily loss
            max_drawdown: Decimal::from(50000),       // $50k max drawdown
            max_leverage: Decimal::from(3),
            max_orders_per_minute: 60,
            max_order_size: Decimal::from(100000),    // $100k max order
            min_order_size: Decimal::from(1),         // $1 min order (allows small test orders)
            allowed_symbols: Vec::new(),              // Empty = allow all symbols
            blocked_symbols: Vec::new(),
        }
    }
}

pub struct RiskManager {
    config: RiskConfig,
    daily_pnl: Decimal,
    daily_reset_time: DateTime<Utc>,
    order_history: Vec<(DateTime<Utc>, String)>, // (timestamp, symbol)
    position_sizes: HashMap<String, Decimal>,
    max_drawdown_reached: Decimal,
}

impl RiskManager {
    pub fn new() -> Self {
        Self::with_config(RiskConfig::default())
    }

    pub fn with_config(config: RiskConfig) -> Self {
        Self {
            config,
            daily_pnl: Decimal::ZERO,
            daily_reset_time: Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
            order_history: Vec::new(),
            position_sizes: HashMap::new(),
            max_drawdown_reached: Decimal::ZERO,
        }
    }

    pub async fn check_order_risk(
        &self,
        symbol: &str,
        side: OrderSide,
        price: Decimal,
        amount: Decimal,
    ) -> bool {
        // Check if symbol is allowed
        if !self.is_symbol_allowed(symbol) {
            warn!("Symbol {} is not allowed for trading", symbol);
            return false;
        }

        // Check order size limits
        let order_value = price * amount;
        if order_value > self.config.max_order_size {
            warn!("Order size {} exceeds maximum allowed {}", order_value, self.config.max_order_size);
            return false;
        }

        if order_value < self.config.min_order_size {
            warn!("Order size {} below minimum required {}", order_value, self.config.min_order_size);
            return false;
        }

        // Check position size limits
        if !self.check_position_size_limit(symbol, side, amount) {
            warn!("Position size limit exceeded for {}", symbol);
            return false;
        }

        // Check daily loss limit
        if !self.check_daily_loss_limit() {
            warn!("Daily loss limit exceeded");
            return false;
        }

        // Check drawdown limit
        if !self.check_drawdown_limit() {
            warn!("Maximum drawdown limit exceeded");
            return false;
        }

        // Check order rate limit
        if !self.check_order_rate_limit(symbol) {
            warn!("Order rate limit exceeded for {}", symbol);
            return false;
        }

        true
    }

    pub fn update_daily_pnl(&mut self, pnl_change: Decimal) {
        self.reset_daily_if_needed();
        self.daily_pnl += pnl_change;
        
        // Update max drawdown
        if self.daily_pnl < self.max_drawdown_reached {
            self.max_drawdown_reached = self.daily_pnl;
        }
    }

    pub fn update_position_size(&mut self, symbol: &str, new_size: Decimal) {
        self.position_sizes.insert(symbol.to_string(), new_size);
    }

    pub fn record_order(&mut self, symbol: &str) {
        self.order_history.push((Utc::now(), symbol.to_string()));
        
        // Clean old entries (keep only last hour)
        let cutoff = Utc::now() - ChronoDuration::hours(1);
        self.order_history.retain(|(timestamp, _)| *timestamp > cutoff);
    }

    fn is_symbol_allowed(&self, symbol: &str) -> bool {
        // Normalize symbol for comparison (handle BTC_USDT, BTC/USDT, BTCUSDT formats)
        let normalized = Self::normalize_symbol(symbol);
        
        // Check if symbol is blocked
        for blocked in &self.config.blocked_symbols {
            if Self::normalize_symbol(blocked) == normalized {
                return false;
            }
        }

        // If allowed list is empty, allow all (except blocked)
        if self.config.allowed_symbols.is_empty() {
            return true;
        }

        // Check if symbol is in allowed list (normalized comparison)
        self.config.allowed_symbols.iter().any(|allowed| {
            Self::normalize_symbol(allowed) == normalized
        })
    }
    
    /// Normalize symbol format: BTC_USDT, BTC/USDT, BTCUSDT all become BTCUSDT
    fn normalize_symbol(symbol: &str) -> String {
        symbol
            .to_uppercase()
            .replace('_', "")
            .replace('/', "")
            .replace('-', "")
    }

    fn check_position_size_limit(&self, symbol: &str, side: OrderSide, amount: Decimal) -> bool {
        let current_size = self.position_sizes.get(symbol).copied().unwrap_or(Decimal::ZERO);
        
        let new_size = match side {
            OrderSide::Buy => current_size + amount,
            OrderSide::Sell => (current_size - amount).abs(),
        };

        new_size <= self.config.max_position_size
    }

    fn check_daily_loss_limit(&self) -> bool {
        self.daily_pnl >= -self.config.max_daily_loss
    }

    fn check_drawdown_limit(&self) -> bool {
        self.max_drawdown_reached >= -self.config.max_drawdown
    }

    fn check_order_rate_limit(&self, symbol: &str) -> bool {
        let cutoff = Utc::now() - ChronoDuration::minutes(1);
        let recent_orders = self.order_history.iter()
            .filter(|(timestamp, order_symbol)| {
                *timestamp > cutoff && order_symbol == symbol
            })
            .count();

        recent_orders < self.config.max_orders_per_minute as usize
    }

    fn reset_daily_if_needed(&mut self) {
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        
        if today_start > self.daily_reset_time {
            self.daily_pnl = Decimal::ZERO;
            self.max_drawdown_reached = Decimal::ZERO;
            self.daily_reset_time = today_start;
        }
    }

    pub fn get_risk_metrics(&self) -> RiskMetrics {
        RiskMetrics {
            daily_pnl: self.daily_pnl,
            max_drawdown: self.max_drawdown_reached,
            position_count: self.position_sizes.len(),
            largest_position: self.position_sizes.values().copied().max().unwrap_or(Decimal::ZERO),
            orders_last_minute: self.get_orders_last_minute(),
            risk_score: self.calculate_risk_score(),
        }
    }

    fn get_orders_last_minute(&self) -> u32 {
        let cutoff = Utc::now() - ChronoDuration::minutes(1);
        self.order_history.iter()
            .filter(|(timestamp, _)| *timestamp > cutoff)
            .count() as u32
    }

    fn calculate_risk_score(&self) -> f64 {
        let mut score = 0.0;

        // Daily PnL component (0-40 points)
        let pnl_ratio = (self.daily_pnl / self.config.max_daily_loss).to_f64().unwrap_or(0.0);
        score += (pnl_ratio.abs() * 40.0).min(40.0);

        // Drawdown component (0-30 points)
        let drawdown_ratio = (self.max_drawdown_reached / self.config.max_drawdown).to_f64().unwrap_or(0.0);
        score += (drawdown_ratio.abs() * 30.0).min(30.0);

        // Position size component (0-20 points)
        let max_position_ratio = self.position_sizes.values()
            .map(|size| (size / self.config.max_position_size).to_f64().unwrap_or(0.0))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        score += (max_position_ratio * 20.0).min(20.0);

        // Order rate component (0-10 points)
        let order_rate_ratio = self.get_orders_last_minute() as f64 / self.config.max_orders_per_minute as f64;
        score += (order_rate_ratio * 10.0).min(10.0);

        score.min(100.0)
    }

    pub fn is_emergency_stop_required(&self) -> bool {
        let metrics = self.get_risk_metrics();
        
        // Emergency stop conditions
        metrics.daily_pnl <= -self.config.max_daily_loss ||
        metrics.max_drawdown <= -self.config.max_drawdown ||
        metrics.risk_score >= 90.0
    }

    pub fn get_position_limit_remaining(&self, symbol: &str) -> Decimal {
        let current_size = self.position_sizes.get(symbol).copied().unwrap_or(Decimal::ZERO);
        self.config.max_position_size - current_size
    }
}

#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub daily_pnl: Decimal,
    pub max_drawdown: Decimal,
    pub position_count: usize,
    pub largest_position: Decimal,
    pub orders_last_minute: u32,
    pub risk_score: f64,
}

impl Default for RiskManager {
    fn default() -> Self {
        Self::new()
    }
}