//! Chat command - proxies messages to NEAR AI.

use crate::commands::CommandHandler;
use crate::error::AppResult;
use async_trait::async_trait;
use conversation_store::{ConversationStore, StoredToolCall};
use near_ai_client::{
    FunctionDefinitionApi, Message, NearAiClient, NearAiError, Role,
    ToolDefinition as NearToolDefinition,
};
use signal_client::{BotMessage, SignalClient};
use std::sync::Arc;
use tools::{FunctionCall as ToolsFunctionCall, ToolCall as ToolsToolCall, ToolExecutor, ToolRegistry};
use tracing::{debug, error, info, instrument, warn};

pub struct ChatHandler {
    near_ai: Arc<NearAiClient>,
    conversations: Arc<ConversationStore>,
    signal_client: Arc<SignalClient>,
    tool_executor: Arc<ToolExecutor>,
    tool_registry: Arc<ToolRegistry>,
    system_prompt: String,
    max_tool_iterations: usize,
}

impl ChatHandler {
    pub fn new(
        near_ai: Arc<NearAiClient>,
        conversations: Arc<ConversationStore>,
        signal_client: Arc<SignalClient>,
        tool_registry: Arc<ToolRegistry>,
        system_prompt: String,
        max_tool_iterations: usize,
    ) -> Self {
        Self {
            near_ai,
            conversations,
            signal_client,
            tool_executor: Arc::new(ToolExecutor::new(tool_registry.clone())),
            tool_registry,
            system_prompt,
            max_tool_iterations,
        }
    }

    /// Build system prompt with current timestamp.
    fn build_system_prompt(&self) -> String {
        let now = chrono::Utc::now();
        format!(
            "{}\n\nCurrent date and time: {} UTC",
            self.system_prompt,
            now.format("%A, %B %d, %Y at %H:%M")
        )
    }

    /// Build messages for NEAR AI request from conversation store.
    async fn build_messages(&self, conversation_id: &str) -> AppResult<Vec<Message>> {
        let system_prompt = self.build_system_prompt();
        let stored_messages = self
            .conversations
            .to_openai_messages(conversation_id, Some(&system_prompt))
            .await?;

        // Convert to NEAR AI message format
        let messages: Vec<Message> = stored_messages
            .into_iter()
            .map(|m| {
                // Convert tool_calls from StoredToolCall to ToolCall if present
                let tool_calls = m.tool_calls.map(|calls| {
                    calls
                        .into_iter()
                        .map(|c| near_ai_client::ToolCall {
                            id: c.id,
                            call_type: "function".to_string(),
                            function: near_ai_client::FunctionCall {
                                name: c.name,
                                arguments: c.arguments,
                            },
                        })
                        .collect()
                });

                Message {
                    role: match m.role.as_str() {
                        "system" => Role::System,
                        "assistant" => Role::Assistant,
                        "tool" => Role::Tool,
                        _ => Role::User,
                    },
                    content: m.content,
                    tool_call_id: m.tool_call_id,
                    tool_calls,
                }
            })
            .collect();

        Ok(messages)
    }

    /// Finalize and store the response.
    async fn finalize_response(
        &self,
        conversation_id: &str,
        content: Option<String>,
    ) -> AppResult<String> {
        let response = content.unwrap_or_else(|| "I don't have a response.".into());
        self.conversations
            .add_message(conversation_id, "assistant", &response, None)
            .await?;
        Ok(response)
    }
}

#[async_trait]
impl CommandHandler for ChatHandler {
    fn is_default(&self) -> bool {
        true
    }

    #[instrument(skip(self, message), fields(user = %message.source, is_group = %message.is_group))]
    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        // Use reply_target as conversation key:
        // - For DMs: sender's phone number
        // - For groups: group_id (shared context for all members)
        let conversation_id = message.reply_target();

        if message.is_group {
            info!(
                "Group chat from {} in {}: {}...",
                &message.source[..message.source.len().min(8)],
                &conversation_id[..conversation_id.len().min(12)],
                &message.text[..message.text.len().min(50)]
            );
        } else {
            info!(
                "Chat from {}: {}...",
                &conversation_id[..conversation_id.len().min(8)],
                &message.text[..message.text.len().min(50)]
            );
        }

        // Add user message to history
        self.conversations
            .add_message(conversation_id, "user", &message.text, Some(&self.system_prompt))
            .await?;

        // Get tool definitions and convert to NEAR AI format
        let tool_defs = self.tool_registry.get_definitions();
        let near_tools: Vec<NearToolDefinition> = tool_defs
            .into_iter()
            .map(|d| NearToolDefinition {
                tool_type: d.tool_type,
                function: FunctionDefinitionApi {
                    name: d.function.name,
                    description: d.function.description,
                    parameters: d.function.parameters,
                },
            })
            .collect();

        // Tool execution loop - only offer tools on first iteration
        let mut tools_executed = false;
        for iteration in 0..self.max_tool_iterations {
            debug!("Tool execution loop iteration {}, tools_executed={}", iteration, tools_executed);

            // Build messages from conversation store
            let messages = self.build_messages(conversation_id).await?;

            // Only offer tools if we haven't executed any yet
            // After tools execute once, force the model to give a text response
            let tools_to_offer = if !tools_executed && !near_tools.is_empty() {
                Some(&near_tools[..])
            } else {
                None
            };

            // Call NEAR AI with tools (or without if already executed)
            let response = match self
                .near_ai
                .chat_with_tools(
                    messages,
                    Some(0.7),
                    None,
                    tools_to_offer,
                )
                .await
            {
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

            // Check if response has tool calls (must be non-empty)
            if let Some(tool_calls) = response.tool_calls {
                if tool_calls.is_empty() {
                    // Empty tool_calls array - treat as final response
                    debug!("LLM returned empty tool_calls array, treating as final response");
                } else {
                debug!("LLM requested {} tool calls", tool_calls.len());

                // Store assistant message with tool calls
                let stored_calls: Vec<StoredToolCall> = tool_calls
                    .iter()
                    .map(|tc| StoredToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    })
                    .collect();

                self.conversations
                    .add_assistant_with_tools(conversation_id, response.content.as_deref(), &stored_calls)
                    .await?;

                // Execute each tool call
                for tool_call in tool_calls {
                    // Send progress message
                    let progress_msg = format!("🔧 Using {}...", tool_call.function.name);
                    if let Err(e) = self
                        .signal_client
                        .send(&message.receiving_account, message.reply_target(), &progress_msg)
                        .await
                    {
                        warn!("Failed to send progress message: {}", e);
                    }

                    // Convert to tools crate format and execute
                    let tools_call = ToolsToolCall {
                        id: tool_call.id.clone(),
                        call_type: tool_call.call_type.clone(),
                        function: ToolsFunctionCall {
                            name: tool_call.function.name.clone(),
                            arguments: tool_call.function.arguments.clone(),
                        },
                    };

                    let result = self.tool_executor.execute(&tools_call).await;
                    let result_content = if result.success {
                        debug!("Tool {} succeeded: {}...", tool_call.function.name, &result.content[..result.content.len().min(100)]);
                        result.content
                    } else {
                        warn!("Tool {} failed: {}", tool_call.function.name, result.content);
                        result.content
                    };

                    // Store tool result
                    self.conversations
                        .add_tool_result(conversation_id, &tool_call.id, &result_content)
                        .await?;
                }

                // Mark that tools have been executed - don't offer them again
                tools_executed = true;

                // Continue loop to let LLM process tool results
                continue;
                }  // close else (non-empty tool_calls)
            }  // close if let Some(tool_calls)

            // No tool calls (or empty array) - this is the final response
            let final_response = self.finalize_response(conversation_id, response.content).await?;

            info!(
                "Response to {}: {} chars",
                &conversation_id[..conversation_id.len().min(12)],
                final_response.len()
            );

            return Ok(final_response);
        }

        // Max iterations reached
        warn!("Max tool iterations ({}) reached for {}", self.max_tool_iterations, conversation_id);
        Ok("I've reached my maximum number of tool uses for this request. Please start a new conversation.".into())
    }
}
