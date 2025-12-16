//! Tool executor with timeout and error handling.

use crate::registry::ToolRegistry;
use crate::types::{ToolCall, ToolResult};
#[cfg(test)]
use crate::error::ToolError;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// Executor for running tools with safety limits.
pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
    timeout_secs: u64,
    max_response_len: usize,
}

impl ToolExecutor {
    /// Create a new executor.
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            timeout_secs: 10,
            max_response_len: 4000,
        }
    }

    /// Set execution timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set maximum response length.
    pub fn with_max_response_len(mut self, len: usize) -> Self {
        self.max_response_len = len;
        self
    }

    /// Execute a tool call.
    pub async fn execute(&self, tool_call: &ToolCall) -> ToolResult {
        let tool_name = &tool_call.function.name;
        info!(tool = %tool_name, "Executing tool");

        // Get the tool
        let tool = match self.registry.get_tool(tool_name) {
            Some(t) => t,
            None => {
                warn!(tool = %tool_name, "Tool not found or disabled");
                return ToolResult::error(
                    &tool_call.id,
                    format!("Tool '{}' not available", tool_name),
                );
            }
        };

        // Execute with timeout
        let result = timeout(
            Duration::from_secs(self.timeout_secs),
            tool.execute(&tool_call.function.arguments),
        )
        .await;

        match result {
            Ok(Ok(content)) => {
                // Truncate if needed
                let content = if content.len() > self.max_response_len {
                    format!(
                        "{}... [truncated, {} chars total]",
                        &content[..self.max_response_len],
                        content.len()
                    )
                } else {
                    content
                };
                info!(tool = %tool_name, len = content.len(), "Tool executed successfully");
                ToolResult::success(&tool_call.id, content)
            }
            Ok(Err(e)) => {
                error!(tool = %tool_name, error = %e, "Tool execution failed");
                ToolResult::error(&tool_call.id, format!("Error: {}", e))
            }
            Err(_) => {
                error!(tool = %tool_name, timeout = self.timeout_secs, "Tool timed out");
                ToolResult::error(
                    &tool_call.id,
                    format!("Tool timed out after {} seconds", self.timeout_secs),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FunctionDefinition, Tool, ToolDefinition};
    use async_trait::async_trait;

    struct SlowTool;

    #[async_trait]
    impl Tool for SlowTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                tool_type: "function".into(),
                function: FunctionDefinition {
                    name: "slow".into(),
                    description: "Slow tool".into(),
                    parameters: serde_json::json!({}),
                },
            }
        }

        fn name(&self) -> &str {
            "slow"
        }

        async fn execute(&self, _arguments: &str) -> Result<String, ToolError> {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok("done".into())
        }
    }

    struct FastTool;

    #[async_trait]
    impl Tool for FastTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                tool_type: "function".into(),
                function: FunctionDefinition {
                    name: "fast".into(),
                    description: "Fast tool".into(),
                    parameters: serde_json::json!({}),
                },
            }
        }

        fn name(&self) -> &str {
            "fast"
        }

        async fn execute(&self, _arguments: &str) -> Result<String, ToolError> {
            Ok("fast result".into())
        }
    }

    #[tokio::test]
    async fn test_execute_success() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(FastTool));
        let executor = ToolExecutor::new(Arc::new(registry));

        let call = ToolCall {
            id: "call-1".into(),
            call_type: "function".into(),
            function: crate::types::FunctionCall {
                name: "fast".into(),
                arguments: "{}".into(),
            },
        };

        let result = executor.execute(&call).await;
        assert!(result.success);
        assert_eq!(result.content, "fast result");
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(SlowTool));
        let executor = ToolExecutor::new(Arc::new(registry)).with_timeout(1);

        let call = ToolCall {
            id: "call-1".into(),
            call_type: "function".into(),
            function: crate::types::FunctionCall {
                name: "slow".into(),
                arguments: "{}".into(),
            },
        };

        let result = executor.execute(&call).await;
        assert!(!result.success);
        assert!(result.content.contains("timed out"));
    }

    #[tokio::test]
    async fn test_execute_tool_not_found() {
        let registry = ToolRegistry::new();
        let executor = ToolExecutor::new(Arc::new(registry));

        let call = ToolCall {
            id: "call-1".into(),
            call_type: "function".into(),
            function: crate::types::FunctionCall {
                name: "nonexistent".into(),
                arguments: "{}".into(),
            },
        };

        let result = executor.execute(&call).await;
        assert!(!result.success);
        assert!(result.content.contains("not available"));
    }
}
