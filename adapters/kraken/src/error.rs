use thiserror::Error;
use serde_json::Value;

#[derive(Debug, Error)]
pub enum KrakenError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocketError(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Invalid API response: {0}")]
    InvalidResponse(String),

    #[error("Kraken API error: {0}")]
    ApiError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Invalid asset pair: {0}")]
    InvalidAssetPair(String),

    #[error("Order error: {0}")]
    OrderError(String),
}

impl KrakenError {
    pub fn from_api_error(error: Vec<String>) -> Self {
        KrakenError::ApiError(error.join(", "))
    }
}

pub type Result<T> = std::result::Result<T, KrakenError>;