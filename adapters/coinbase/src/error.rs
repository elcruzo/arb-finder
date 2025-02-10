use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoinbaseError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocketError(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Invalid API response: {0}")]
    InvalidResponse(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Invalid product ID: {0}")]
    InvalidProductId(String),

    #[error("Order error: {0}")]
    OrderError(String),
}

pub type Result<T> = std::result::Result<T, CoinbaseError>;