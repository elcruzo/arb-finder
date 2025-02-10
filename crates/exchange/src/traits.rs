use async_trait::async_trait;
use arbfinder_core::{
    ArbFinderError, Result, Balance, MarketData, Order, OrderFill, OrderId, OrderRequest,
    OrderUpdate, Symbol, VenueId,
};
use chrono::{DateTime, Utc};
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

pub type MarketDataStream = Pin<Box<dyn Stream<Item = Result<MarketData>> + Send>>;
pub type OrderUpdateStream = Pin<Box<dyn Stream<Item = Result<OrderUpdate>> + Send>>;

#[async_trait]
pub trait ExchangeAdapter: Send + Sync {
    fn venue_id(&self) -> VenueId;
    
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn is_connected(&self) -> bool;
    
    async fn get_server_time(&self) -> Result<DateTime<Utc>>;
    async fn ping(&self) -> Result<u64>;
    
    async fn get_symbols(&self) -> Result<Vec<Symbol>>;
    async fn get_symbol_info(&self, symbol: &Symbol) -> Result<SymbolInfo>;
    
    async fn subscribe_orderbook(&mut self, symbol: &Symbol, depth: Option<u32>) -> Result<()>;
    async fn subscribe_trades(&mut self, symbol: &Symbol) -> Result<()>;
    async fn subscribe_ticker(&mut self, symbol: &Symbol) -> Result<()>;
    async fn unsubscribe_orderbook(&mut self, symbol: &Symbol) -> Result<()>;
    async fn unsubscribe_trades(&mut self, symbol: &Symbol) -> Result<()>;
    async fn unsubscribe_ticker(&mut self, symbol: &Symbol) -> Result<()>;
    
    async fn market_data_stream(&self) -> Result<MarketDataStream>;
    async fn order_update_stream(&self) -> Result<OrderUpdateStream>;
    
    async fn place_order(&mut self, request: &OrderRequest) -> Result<Order>;
    async fn cancel_order(&mut self, order_id: &OrderId) -> Result<()>;
    async fn cancel_all_orders(&mut self, symbol: Option<&Symbol>) -> Result<Vec<OrderId>>;
    async fn get_order(&self, order_id: &OrderId) -> Result<Option<Order>>;
    async fn get_open_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>>;
    async fn get_order_history(&self, symbol: Option<&Symbol>, limit: Option<u32>) -> Result<Vec<Order>>;
    
    async fn get_balances(&self) -> Result<Vec<Balance>>;
    async fn get_balance(&self, asset: &str) -> Result<Option<Balance>>;
    
    async fn get_trade_history(&self, symbol: Option<&Symbol>, limit: Option<u32>) -> Result<Vec<OrderFill>>;
    
    async fn get_account_info(&self) -> Result<AccountInfo>;
}

#[async_trait]
pub trait WebSocketHandler: Send + Sync {
    async fn on_message(&mut self, message: &str) -> Result<()>;
    async fn on_connect(&mut self) -> Result<()>;
    async fn on_disconnect(&mut self) -> Result<()>;
    async fn on_error(&mut self, error: &ArbFinderError) -> Result<()>;
    async fn on_ping(&mut self) -> Result<()>;
    async fn on_pong(&mut self) -> Result<()>;
}

#[async_trait]
pub trait RestClient: Send + Sync {
    async fn get(&self, endpoint: &str, params: Option<&HashMap<String, String>>) -> Result<Value>;
    async fn post(&self, endpoint: &str, body: Option<&Value>) -> Result<Value>;
    async fn put(&self, endpoint: &str, body: Option<&Value>) -> Result<Value>;
    async fn delete(&self, endpoint: &str, params: Option<&HashMap<String, String>>) -> Result<Value>;
    
    fn sign_request(&self, method: &str, endpoint: &str, params: &str, timestamp: u64) -> Result<String>;
}

#[async_trait]
pub trait OrderBookManager: Send + Sync {
    async fn handle_snapshot(&mut self, symbol: &Symbol, data: &Value) -> Result<()>;
    async fn handle_update(&mut self, symbol: &Symbol, data: &Value) -> Result<()>;
    async fn get_orderbook(&self, symbol: &Symbol) -> Result<Option<arbfinder_core::OrderBook>>;
    fn is_synchronized(&self, symbol: &Symbol) -> bool;
}

#[async_trait]
pub trait SymbolNormalizer: Send + Sync {
    fn normalize_symbol(&self, exchange_symbol: &str) -> Result<Symbol>;
    fn denormalize_symbol(&self, symbol: &Symbol) -> Result<String>;
    fn normalize_side(&self, exchange_side: &str) -> Result<arbfinder_core::OrderSide>;
    fn denormalize_side(&self, side: arbfinder_core::OrderSide) -> String;
    fn normalize_order_type(&self, exchange_type: &str) -> Result<arbfinder_core::OrderType>;
    fn denormalize_order_type(&self, order_type: arbfinder_core::OrderType) -> String;
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub symbol: Symbol,
    pub status: String,
    pub base_asset_precision: u32,
    pub quote_asset_precision: u32,
    pub tick_size: rust_decimal::Decimal,
    pub lot_size: rust_decimal::Decimal,
    pub min_order_size: rust_decimal::Decimal,
    pub max_order_size: rust_decimal::Decimal,
    pub min_notional: rust_decimal::Decimal,
    pub trading_fees: TradingFees,
}

#[derive(Debug, Clone)]
pub struct TradingFees {
    pub maker_fee: rust_decimal::Decimal,
    pub taker_fee: rust_decimal::Decimal,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub account_type: String,
    pub trading_enabled: bool,
    pub withdraw_enabled: bool,
    pub deposit_enabled: bool,
    pub balances: Vec<Balance>,
    pub permissions: Vec<String>,
    pub commission_rates: TradingFees,
}

#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub last_ping: Option<DateTime<Utc>>,
    pub reconnect_count: u32,
    pub error_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    pub symbol: Symbol,
    pub data_type: String,
    pub subscribed_at: DateTime<Utc>,
    pub message_count: u64,
    pub last_message: Option<DateTime<Utc>>,
}

pub trait ExchangeConfig {
    fn base_url(&self) -> &str;
    fn websocket_url(&self) -> &str;
    fn api_key(&self) -> Option<&str>;
    fn secret_key(&self) -> Option<&str>;
    fn passphrase(&self) -> Option<&str>;
    fn sandbox(&self) -> bool;
    fn rate_limit_requests_per_second(&self) -> u32;
    fn rate_limit_orders_per_second(&self) -> u32;
    fn reconnect_attempts(&self) -> u32;
    fn reconnect_delay_ms(&self) -> u64;
    fn heartbeat_interval_ms(&self) -> u64;
    fn request_timeout_ms(&self) -> u64;
}

#[derive(Debug, Clone)]
pub struct DefaultExchangeConfig {
    pub base_url: String,
    pub websocket_url: String,
    pub api_key: Option<String>,
    pub secret_key: Option<String>,
    pub passphrase: Option<String>,
    pub sandbox: bool,
    pub rate_limit_requests_per_second: u32,
    pub rate_limit_orders_per_second: u32,
    pub reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub request_timeout_ms: u64,
}

impl ExchangeConfig for DefaultExchangeConfig {
    fn base_url(&self) -> &str { &self.base_url }
    fn websocket_url(&self) -> &str { &self.websocket_url }
    fn api_key(&self) -> Option<&str> { self.api_key.as_deref() }
    fn secret_key(&self) -> Option<&str> { self.secret_key.as_deref() }
    fn passphrase(&self) -> Option<&str> { self.passphrase.as_deref() }
    fn sandbox(&self) -> bool { self.sandbox }
    fn rate_limit_requests_per_second(&self) -> u32 { self.rate_limit_requests_per_second }
    fn rate_limit_orders_per_second(&self) -> u32 { self.rate_limit_orders_per_second }
    fn reconnect_attempts(&self) -> u32 { self.reconnect_attempts }
    fn reconnect_delay_ms(&self) -> u64 { self.reconnect_delay_ms }
    fn heartbeat_interval_ms(&self) -> u64 { self.heartbeat_interval_ms }
    fn request_timeout_ms(&self) -> u64 { self.request_timeout_ms }
}

impl Default for DefaultExchangeConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            websocket_url: String::new(),
            api_key: None,
            secret_key: None,
            passphrase: None,
            sandbox: false,
            rate_limit_requests_per_second: 10,
            rate_limit_orders_per_second: 5,
            reconnect_attempts: 10,
            reconnect_delay_ms: 5000,
            heartbeat_interval_ms: 30000,
            request_timeout_ms: 10000,
        }
    }
}