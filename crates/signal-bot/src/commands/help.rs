//! Help command - displays available commands.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;

pub struct HelpHandler;

impl HelpHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HelpHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommandHandler for HelpHandler {
    fn name(&self) -> &str {
        "help"
    }

    fn trigger(&self) -> Option<&str> {
        Some("!help")
    }

    async fn execute(&self, _message: &BotMessage) -> AppResult<String> {
        Ok(r#"**Signal AI** (Private & Verifiable)

Just send a message to chat with AI.

**Commands:**
- !verify - Show privacy attestation proofs
- !clear - Clear conversation history
- !models - List available AI models
- !help - Show this message

**Privacy:**
Your messages are end-to-end encrypted via Signal, processed in a verified TEE (Intel TDX), and sent to NEAR AI Cloud's private inference (NVIDIA GPU TEE).

Neither the bot operator nor NEAR AI can read your messages."#
            .into())
    }
}
