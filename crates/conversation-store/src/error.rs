//! Conversation storage errors.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConversationError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
