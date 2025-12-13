//! Signal client errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignalError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Not registered")]
    NotRegistered,

    #[error("Send failed: {0}")]
    SendFailed(String),
}
