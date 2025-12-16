//! Conversation and message types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Stored tool call info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StoredToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl StoredMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn with_tool_calls(role: impl Into<String>, content: Option<String>, tool_calls: Vec<StoredToolCall>) -> Self {
        Self {
            role: role.into(),
            content,
            timestamp: Utc::now(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            timestamp: Utc::now(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// A conversation with a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub user_id: String,
    pub messages: Vec<StoredMessage>,
    pub system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    pub fn new(user_id: impl Into<String>, system_prompt: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            user_id: user_id.into(),
            messages: Vec::new(),
            system_prompt,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a message to the conversation.
    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(StoredMessage::new(role, content));
        self.updated_at = Utc::now();
    }

    /// Trim to max messages, keeping most recent.
    pub fn trim(&mut self, max_messages: usize) {
        if self.messages.len() > max_messages {
            let start = self.messages.len() - max_messages;
            self.messages = self.messages[start..].to_vec();
        }
    }
}

/// OpenAI-compatible message format.
#[derive(Debug, Clone, Serialize)]
pub struct OpenAiMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StoredToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
