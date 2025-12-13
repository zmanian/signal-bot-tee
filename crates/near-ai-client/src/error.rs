//! NEAR AI client errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NearAiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Authentication failed")]
    Unauthorized,

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Stream error: {0}")]
    Stream(String),

    #[error("Empty response from AI service")]
    EmptyResponse,
}
