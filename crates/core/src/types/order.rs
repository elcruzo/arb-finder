use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Symbol, VenueId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_string(s: &str) -> Option<Self> {
        Uuid::parse_str(s).ok().map(Self)
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
    PostOnly,
    FillOrKill,
    ImmediateOrCancel,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Market => write!(f, "market"),
            OrderType::Limit => write!(f, "limit"),
            OrderType::StopMarket => write!(f, "stop_market"),
            OrderType::StopLimit => write!(f, "stop_limit"),
            OrderType::PostOnly => write!(f, "post_only"),
            OrderType::FillOrKill => write!(f, "fill_or_kill"),
            OrderType::ImmediateOrCancel => write!(f, "immediate_or_cancel"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GoodTillCanceled,
    ImmediateOrCancel,
    FillOrKill,
    PostOnly,
}

impl std::fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeInForce::GoodTillCanceled => write!(f, "GTC"),
            TimeInForce::ImmediateOrCancel => write!(f, "IOC"),
            TimeInForce::FillOrKill => write!(f, "FOK"),
            TimeInForce::PostOnly => write!(f, "POST_ONLY"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "pending"),
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            OrderStatus::Filled => write!(f, "filled"),
            OrderStatus::Canceled => write!(f, "canceled"),
            OrderStatus::Rejected => write!(f, "rejected"),
            OrderStatus::Expired => write!(f, "expired"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub client_order_id: Option<String>,
    pub venue_id: VenueId,
    pub venue_order_id: Option<String>,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub remaining_quantity: Decimal,
    pub average_fill_price: Option<Decimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub fees: Vec<OrderFee>,
}

impl Order {
    pub fn new_market(
        venue_id: VenueId,
        symbol: Symbol,
        side: OrderSide,
        quantity: Decimal,
    ) -> Self {
        Self {
            id: OrderId::new(),
            client_order_id: None,
            venue_id,
            venue_order_id: None,
            symbol,
            side,
            order_type: OrderType::Market,
            quantity,
            price: None,
            stop_price: None,
            time_in_force: TimeInForce::ImmediateOrCancel,
            status: OrderStatus::Pending,
            filled_quantity: Decimal::ZERO,
            remaining_quantity: quantity,
            average_fill_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            fees: Vec::new(),
        }
    }

    pub fn new_limit(
        venue_id: VenueId,
        symbol: Symbol,
        side: OrderSide,
        quantity: Decimal,
        price: Decimal,
    ) -> Self {
        Self {
            id: OrderId::new(),
            client_order_id: None,
            venue_id,
            venue_order_id: None,
            symbol,
            side,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            stop_price: None,
            time_in_force: TimeInForce::GoodTillCanceled,
            status: OrderStatus::Pending,
            filled_quantity: Decimal::ZERO,
            remaining_quantity: quantity,
            average_fill_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            fees: Vec::new(),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Open | OrderStatus::PartiallyFilled
        )
    }

    pub fn is_filled(&self) -> bool {
        self.status == OrderStatus::Filled
    }

    pub fn is_canceled(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Canceled | OrderStatus::Rejected | OrderStatus::Expired
        )
    }

    pub fn fill_percentage(&self) -> Decimal {
        if self.quantity.is_zero() {
            Decimal::ZERO
        } else {
            (self.filled_quantity / self.quantity) * Decimal::from(100)
        }
    }

    pub fn update_fill(&mut self, fill: &OrderFill) {
        self.filled_quantity += fill.quantity;
        self.remaining_quantity = self.quantity - self.filled_quantity;
        
        if self.remaining_quantity.is_zero() {
            self.status = OrderStatus::Filled;
        } else if self.filled_quantity > Decimal::ZERO {
            self.status = OrderStatus::PartiallyFilled;
        }

        // Update average fill price
        if let Some(avg_price) = self.average_fill_price {
            let total_value = avg_price * (self.filled_quantity - fill.quantity) + fill.price * fill.quantity;
            self.average_fill_price = Some(total_value / self.filled_quantity);
        } else {
            self.average_fill_price = Some(fill.price);
        }

        // Add fee if present
        if let Some(fee) = &fill.fee {
            self.fees.push(fee.clone());
        }

        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderFill {
    pub id: String,
    pub order_id: OrderId,
    pub venue_order_id: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub fee: Option<OrderFee>,
    pub timestamp: DateTime<Utc>,
    pub is_maker: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderFee {
    pub asset: String,
    pub amount: Decimal,
    pub rate: Decimal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub order_id: OrderId,
    pub venue_order_id: Option<String>,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub remaining_quantity: Decimal,
    pub average_fill_price: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderRequest {
    pub client_order_id: Option<String>,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub stop_price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub post_only: bool,
    pub reduce_only: bool,
}

impl OrderRequest {
    pub fn new_market(symbol: Symbol, side: OrderSide, quantity: Decimal) -> Self {
        Self {
            client_order_id: None,
            symbol,
            side,
            order_type: OrderType::Market,
            quantity,
            price: None,
            stop_price: None,
            time_in_force: TimeInForce::ImmediateOrCancel,
            post_only: false,
            reduce_only: false,
        }
    }

    pub fn new_limit(symbol: Symbol, side: OrderSide, quantity: Decimal, price: Decimal) -> Self {
        Self {
            client_order_id: None,
            symbol,
            side,
            order_type: OrderType::Limit,
            quantity,
            price: Some(price),
            stop_price: None,
            time_in_force: TimeInForce::GoodTillCanceled,
            post_only: false,
            reduce_only: false,
        }
    }

    pub fn with_client_id(mut self, client_id: String) -> Self {
        self.client_order_id = Some(client_id);
        self
    }

    pub fn with_time_in_force(mut self, tif: TimeInForce) -> Self {
        self.time_in_force = tif;
        self
    }

    pub fn post_only(mut self) -> Self {
        self.post_only = true;
        self
    }
}