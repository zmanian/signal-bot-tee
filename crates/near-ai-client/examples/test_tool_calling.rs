//! Test script for debugging NEAR AI tool calling
//! Run with: cargo run -p near-ai-client --example test_tool_calling

use near_ai_client::{Message, NearAiClient, Role, ToolDefinition, FunctionDefinitionApi};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    // Load environment
    dotenvy::dotenv().ok();

    let api_key = std::env::var("NEAR_AI_API_KEY")
        .expect("NEAR_AI_API_KEY must be set");
    let base_url = std::env::var("NEAR_AI_BASE_URL")
        .unwrap_or_else(|_| "https://cloud-api.near.ai/v1".to_string());
    let model = std::env::var("NEAR_AI_MODEL")
        .unwrap_or_else(|_| "deepseek-ai/DeepSeek-V3.1".to_string());

    println!("Using model: {}", model);
    println!("Base URL: {}", base_url);

    let client = NearAiClient::new(api_key, base_url, model, Duration::from_secs(60))?;

    // Define a simple tool
    let tools = vec![ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinitionApi {
            name: "web_search".to_string(),
            description: "Search the web for information".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                },
                "required": ["query"]
            }),
        },
    }];

    // Step 1: Send initial message
    println!("\n=== Step 1: Initial request ===");
    let messages = vec![
        Message::system("You are a helpful assistant with access to web search."),
        Message::user("What's the latest news about Bitcoin?"),
    ];

    let response = client.chat_with_tools(messages.clone(), Some(0.7), None, Some(&tools)).await?;
    println!("Response content: {:?}", response.content);
    println!("Tool calls: {:?}", response.tool_calls);
    println!("Finish reason: {}", response.finish_reason);

    // Step 2: If there are tool calls, simulate the result
    if let Some(tool_calls) = response.tool_calls {
        println!("\n=== Step 2: Sending tool result ===");

        let tool_call = &tool_calls[0];
        println!("Tool call ID: {}", tool_call.id);
        println!("Function name: {}", tool_call.function.name);
        println!("Arguments: {}", tool_call.function.arguments);

        // Build messages with tool result
        let mut messages_with_result = messages.clone();

        // Add assistant message with tool calls
        messages_with_result.push(Message {
            role: Role::Assistant,
            content: response.content.clone(),
            tool_calls: Some(tool_calls.clone()),
            tool_call_id: None,
        });

        // Add tool result
        messages_with_result.push(Message {
            role: Role::Tool,
            content: Some("Search results for 'Bitcoin news':\n\n1. Bitcoin reaches new highs\n   Bitcoin surged to $100,000...\n   URL: https://example.com/1\n\n2. Market analysis\n   Experts predict continued growth...\n   URL: https://example.com/2".to_string()),
            tool_calls: None,
            tool_call_id: Some(tool_call.id.clone()),
        });

        // Debug: Print the request JSON
        println!("\nMessages being sent:");
        for (i, msg) in messages_with_result.iter().enumerate() {
            println!("  [{}] role={:?}, tool_call_id={:?}, has_tool_calls={}, content_preview={:?}",
                i, msg.role, msg.tool_call_id, msg.tool_calls.is_some(),
                msg.content.as_ref().map(|c| &c[..c.len().min(50)]));
        }

        // Serialize to see the actual JSON
        println!("\n=== Serialized request (messages only) ===");
        let json = serde_json::to_string_pretty(&messages_with_result)?;
        println!("{}", json);

        // KEY FIX: Don't offer tools in the follow-up call - force model to respond
        println!("\n=== Sending to NEAR AI (WITHOUT tools to force response) ===");
        let response2 = client.chat_with_tools(messages_with_result, Some(0.7), None, None).await?;
        println!("\nResponse 2 content: {:?}", response2.content);
        println!("Response 2 tool calls: {:?}", response2.tool_calls);
        println!("Response 2 finish reason: {}", response2.finish_reason);

        if response2.tool_calls.is_some() {
            println!("\n=== WARNING: Still got tool calls (shouldn't happen without tools offered) ===");
        } else {
            println!("\n=== SUCCESS: Got final text response ===");
        }
    }

    Ok(())
}
