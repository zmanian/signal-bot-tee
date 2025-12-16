//! Tool execution errors.

use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Error, Debug)]
pub enum ToolError {
    /// Tool execution timed out.
    #[error("Tool execution timed out after {0} seconds")]
    Timeout(u64),

    /// Invalid arguments provided to tool.
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parsing failed.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded")]
    RateLimit,

    /// Tool is not configured (missing API key, etc.).
    #[error("Tool not configured: {0}")]
    NotConfigured(String),

    /// External service returned an error.
    #[error("External service error: {0}")]
    ExternalService(String),

    /// Math evaluation error.
    #[error("Math evaluation error: {0}")]
    MathError(String),
}
