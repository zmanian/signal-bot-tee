//! Dstack client errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DstackError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Hyper error: {0}")]
    Hyper(#[from] hyper::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Quote generation failed: {0}")]
    QuoteGeneration(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Not running in TEE")]
    NotInTee,

    #[error("Socket not found: {0}")]
    SocketNotFound(String),
}
