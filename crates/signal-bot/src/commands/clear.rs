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
    fn trigger(&self) -> Option<&str> {
        Some("!clear")
    }

    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        // Use reply_target: clears group conversation in groups, personal in DMs
        let conversation_id = message.reply_target();
        let cleared = self.conversations.clear(conversation_id).await?;

        if cleared {
            if message.is_group {
                info!("Cleared group history for {}", &conversation_id[..12.min(conversation_id.len())]);
                Ok("Group conversation history cleared.".into())
            } else {
                info!("Cleared history for {}", &conversation_id[..8.min(conversation_id.len())]);
                Ok("Conversation history cleared.".into())
            }
        } else {
            Ok("No conversation history to clear.".into())
        }
    }
}
