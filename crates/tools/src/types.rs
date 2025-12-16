//! Tool type definitions following OpenAI function calling schema.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::error::ToolError;

/// Tool definition sent to LLM (OpenAI-compatible schema).
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    /// Always "function".
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function details.
    pub function: FunctionDefinition,
}

/// Function definition within a tool.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    /// Function name (e.g., "web_search").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for parameters.
    pub parameters: serde_json::Value,
}

/// Tool call requested by LLM.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    /// Unique ID for this call.
    pub id: String,
    /// Always "function".
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function to call.
    pub function: FunctionCall,
}

/// Function call details.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    /// Function name.
    pub name: String,
    /// JSON string of arguments.
    pub arguments: String,
}

/// Result of executing a tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// ID of the tool call this responds to.
    pub tool_call_id: String,
    /// Result content (or error message).
    pub content: String,
    /// Whether execution succeeded.
    pub success: bool,
}

impl ToolResult {
    /// Create a successful result.
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            success: true,
        }
    }

    /// Create an error result.
    pub fn error(tool_call_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: message.into(),
            success: false,
        }
    }
}

/// Trait for implementing tools.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool definition for the LLM.
    fn definition(&self) -> ToolDefinition;

    /// Get the tool name.
    fn name(&self) -> &str;

    /// Execute the tool with JSON arguments.
    async fn execute(&self, arguments: &str) -> Result<String, ToolError>;
}
