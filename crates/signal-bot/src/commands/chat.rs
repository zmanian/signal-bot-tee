//! Chat command - proxies messages to NEAR AI.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use conversation_store::ConversationStore;
use near_ai_client::{Message, NearAiClient, NearAiError, Role};
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::{error, info, instrument};

pub struct ChatHandler {
    near_ai: Arc<NearAiClient>,
    conversations: Arc<ConversationStore>,
    system_prompt: String,
}

impl ChatHandler {
    pub fn new(
        near_ai: Arc<NearAiClient>,
        conversations: Arc<ConversationStore>,
        system_prompt: String,
    ) -> Self {
        Self {
            near_ai,
            conversations,
            system_prompt,
        }
    }
}

#[async_trait]
impl CommandHandler for ChatHandler {
    fn name(&self) -> &str {
        "chat"
    }

    fn is_default(&self) -> bool {
        true
    }

    #[instrument(skip(self, message), fields(user = %message.source))]
    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let user_id = &message.source;

        info!(
            "Chat from {}: {}...",
            &user_id[..user_id.len().min(8)],
            &message.text[..message.text.len().min(50)]
        );

        // Add user message to history
        self.conversations
            .add_message(user_id, "user", &message.text, Some(&self.system_prompt))
            .await?;

        // Get full conversation for context
        let stored_messages = self
            .conversations
            .to_openai_messages(user_id, Some(&self.system_prompt))
            .await?;

        // Convert to NEAR AI message format
        let messages: Vec<Message> = stored_messages
            .into_iter()
            .map(|m| Message {
                role: match m.role.as_str() {
                    "system" => Role::System,
                    "assistant" => Role::Assistant,
                    _ => Role::User,
                },
                content: m.content,
            })
            .collect();

        // Query NEAR AI with automatic retry
        let response = match self.near_ai.chat_with_retry(messages, Some(0.7), None, None).await {
            Ok(r) => r,
            Err(NearAiError::RateLimit) => {
                return Ok(
                    "I'm receiving too many requests. Please wait a moment and try again."
                        .into(),
                );
            }
            Err(NearAiError::EmptyResponse) => {
                error!("NEAR AI returned empty response");
                return Ok(
                    "The AI service returned an empty response. Please try rephrasing your message."
                        .into(),
                );
            }
            Err(e) => {
                error!("NEAR AI error: {}", e);
                return Ok(
                    "Sorry, I encountered an error connecting to the AI service. Please try again."
                        .into(),
                );
            }
        };

        // Store assistant response
        self.conversations
            .add_message(user_id, "assistant", &response, None)
            .await?;

        info!(
            "Response to {}: {} chars",
            &user_id[..user_id.len().min(8)],
            response.len()
        );

        Ok(response)
    }
}
