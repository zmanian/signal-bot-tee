//! Application error types.

use thiserror::Error;

/// Main application error type.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] anyhow::Error),

    #[error("Signal error: {0}")]
    Signal(#[from] signal_client::SignalError),

    #[error("NEAR AI error: {0}")]
    NearAi(#[from] near_ai_client::NearAiError),

    #[error("Conversation error: {0}")]
    Conversation(#[from] conversation_store::ConversationError),

    #[error("Dstack error: {0}")]
    Dstack(#[from] dstack_client::DstackError),
}

/// Result type alias for application errors.
pub type AppResult<T> = Result<T, AppError>;
