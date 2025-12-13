//! Clear command - resets conversation history.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use conversation_store::ConversationStore;
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::info;

pub struct ClearHandler {
    conversations: Arc<ConversationStore>,
}

impl ClearHandler {
    pub fn new(conversations: Arc<ConversationStore>) -> Self {
        Self { conversations }
    }
}

#[async_trait]
impl CommandHandler for ClearHandler {
    fn name(&self) -> &str {
        "clear"
    }

    fn trigger(&self) -> Option<&str> {
        Some("!clear")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let cleared = self.conversations.clear(&message.source).await?;

        if cleared {
            info!("Cleared history for {}", &message.source[..8.min(message.source.len())]);
            Ok("Conversation history cleared.".into())
        } else {
            Ok("No conversation history to clear.".into())
        }
    }
}
