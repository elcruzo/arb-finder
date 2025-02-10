use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::{OrderSide, Symbol, VenueId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub id: String,
    pub symbol: Symbol,
    pub buy_venue: VenueId,
    pub sell_venue: VenueId,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub spread_bps: i32,
    pub spread_percentage: Decimal,
    pub max_quantity: Decimal,
    pub estimated_profit: Decimal,
    pub confidence: f64,
    pub signal_strength: f64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub strategy_type: ArbitrageStrategy,
}

impl ArbitrageOpportunity {
    pub fn new(
        symbol: Symbol,
        buy_venue: VenueId,
        sell_venue: VenueId,
        buy_price: Decimal,
        sell_price: Decimal,
        max_quantity: Decimal,
        strategy_type: ArbitrageStrategy,
    ) -> Self {
        let spread = sell_price - buy_price;
        let spread_percentage = if buy_price > Decimal::ZERO {
            (spread / buy_price) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        let spread_bps = (spread_percentage * Decimal::from(100)).to_i32().unwrap_or(0);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            symbol,
            buy_venue,
            sell_venue,
            buy_price,
            sell_price,
            spread_bps,
            spread_percentage,
            max_quantity,
            estimated_profit: spread * max_quantity,
            confidence: 0.0,
            signal_strength: 0.0,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(30),
            strategy_type,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.sell_price > self.buy_price && 
        self.max_quantity > Decimal::ZERO &&
        Utc::now() < self.expires_at
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    pub fn profit_after_fees(&self, buy_fee_rate: Decimal, sell_fee_rate: Decimal) -> Decimal {
        let buy_cost = self.buy_price * self.max_quantity;
        let buy_fee = buy_cost * buy_fee_rate;
        let sell_revenue = self.sell_price * self.max_quantity;
        let sell_fee = sell_revenue * sell_fee_rate;
        
        sell_revenue - sell_fee - buy_cost - buy_fee
    }

    pub fn roi_percentage(&self, buy_fee_rate: Decimal, sell_fee_rate: Decimal) -> Decimal {
        let profit = self.profit_after_fees(buy_fee_rate, sell_fee_rate);
        let investment = self.buy_price * self.max_quantity;
        
        if investment > Decimal::ZERO {
            (profit / investment) * Decimal::from(100)
        } else {
            Decimal::ZERO
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArbitrageStrategy {
    Simple,
    Triangular,
    StatisticalMeanReversion,
    SpotFutures,
    CrossExchange,
}

impl std::fmt::Display for ArbitrageStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArbitrageStrategy::Simple => write!(f, "simple"),
            ArbitrageStrategy::Triangular => write!(f, "triangular"),
            ArbitrageStrategy::StatisticalMeanReversion => write!(f, "statistical_mean_reversion"),
            ArbitrageStrategy::SpotFutures => write!(f, "spot_futures"),
            ArbitrageStrategy::CrossExchange => write!(f, "cross_exchange"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriangularArbitrageOpportunity {
    pub id: String,
    pub venue: VenueId,
    pub symbol_a: Symbol,
    pub symbol_b: Symbol,
    pub symbol_c: Symbol,
    pub path: Vec<ArbitrageStep>,
    pub total_return: Decimal,
    pub min_quantity: Decimal,
    pub estimated_profit: Decimal,
    pub confidence: f64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageStep {
    pub symbol: Symbol,
    pub side: OrderSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub venue: VenueId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageExecution {
    pub opportunity_id: String,
    pub legs: Vec<ArbitrageLeg>,
    pub status: ExecutionStatus,
    pub total_profit: Decimal,
    pub total_fees: Decimal,
    pub execution_time_ms: u64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageLeg {
    pub venue: VenueId,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub target_quantity: Decimal,
    pub target_price: Decimal,
    pub actual_quantity: Decimal,
    pub actual_price: Decimal,
    pub order_id: Option<String>,
    pub status: LegStatus,
    pub fees: Decimal,
    pub slippage: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    InProgress,
    Completed,
    PartiallyCompleted,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegStatus {
    Pending,
    Submitted,
    PartiallyFilled,
    Filled,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageSignal {
    pub symbol: Symbol,
    pub signal_type: SignalType,
    pub strength: f64,
    pub confidence: f64,
    pub expected_return: Decimal,
    pub risk_score: f64,
    pub holding_period: chrono::Duration,
    pub entry_price: Decimal,
    pub target_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    MeanReversion,
    Momentum,
    Spread,
    Volatility,
    Volume,
    News,
    Technical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketInefficiency {
    pub id: String,
    pub symbol: Symbol,
    pub venues: Vec<VenueId>,
    pub inefficiency_type: InefficiencyType,
    pub severity: f64,
    pub duration_ms: u64,
    pub potential_profit: Decimal,
    pub detected_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InefficiencyType {
    PriceDiscrepancy,
    VolumeImbalance,
    LatencyArbitrage,
    FundingRateDeviation,
    LiquidityGap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioPosition {
    pub symbol: Symbol,
    pub venue: VenueId,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub market_value: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub max_drawdown: Decimal,
    pub sharpe_ratio: f64,
    pub var_95: Decimal,
    pub var_99: Decimal,
    pub beta: f64,
    pub alpha: f64,
    pub volatility: f64,
    pub information_ratio: f64,
    pub calmar_ratio: f64,
    pub sortino_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_return: Decimal,
    pub annualized_return: Decimal,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub max_consecutive_wins: u32,
    pub max_consecutive_losses: u32,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
}