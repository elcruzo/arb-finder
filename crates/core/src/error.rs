use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArbFinderError>;

#[derive(Error, Debug)]
pub enum ArbFinderError {
    #[error("Exchange error: {0}")]
    Exchange(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Order book error: {0}")]
    OrderBook(String),

    #[error("Strategy error: {0}")]
    Strategy(String),

    #[error("Risk management error: {0}")]
    Risk(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Network timeout: {0}")]
    Timeout(String),

    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),

    #[error("Invalid order: {0}")]
    InvalidOrder(String),

    #[error("Market closed: {0}")]
    MarketClosed(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Internal error: {0}")]
    Internal(String),
}