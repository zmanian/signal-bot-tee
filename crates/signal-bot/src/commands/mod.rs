//! Bot command handlers.

mod chat;
mod clear;
mod help;
mod models;
mod verify;

pub use chat::ChatHandler;
pub use clear::ClearHandler;
pub use help::HelpHandler;
pub use models::ModelsHandler;
pub use verify::VerifyHandler;

use crate::error::AppResult;
use async_trait::async_trait;
use signal_client::BotMessage;

/// Command handler trait.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Command name (e.g., "help", "clear").
    fn name(&self) -> &str;

    /// Command trigger (e.g., "!help").
    fn trigger(&self) -> Option<&str> {
        None
    }

    /// Whether this is the default handler for non-command messages.
    fn is_default(&self) -> bool {
        false
    }

    /// Check if this handler matches the message.
    fn matches(&self, message: &BotMessage) -> bool {
        if let Some(trigger) = self.trigger() {
            message.text.starts_with(trigger)
        } else {
            self.is_default() && !message.text.starts_with('!')
        }
    }

    /// Execute the command.
    async fn execute(&self, message: &BotMessage) -> AppResult<String>;
}
