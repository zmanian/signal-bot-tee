//! Models command - lists available AI models.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use near_ai_client::NearAiClient;
use signal_client::BotMessage;
use std::sync::Arc;
use tracing::error;

pub struct ModelsHandler {
    near_ai: Arc<NearAiClient>,
}

impl ModelsHandler {
    pub fn new(near_ai: Arc<NearAiClient>) -> Self {
        Self { near_ai }
    }
}

#[async_trait]
impl CommandHandler for ModelsHandler {
    fn trigger(&self) -> Option<&str> {
        Some("!models")
    }

    async fn execute(&self, _message: &BotMessage) -> AppResult<String> {
        match self.near_ai.list_models().await {
            Ok(models) => {
                let model_list: String = models
                    .iter()
                    .take(10)
                    .map(|m| format!("- {}", m.id))
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(format!(
                    "**Available Models:**\n{}\n\n_Current: {}_",
                    model_list,
                    self.near_ai.model()
                ))
            }
            Err(e) => {
                error!("Failed to list models: {}", e);
                Ok("Could not fetch model list.".into())
            }
        }
    }
}
