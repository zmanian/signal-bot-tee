# Tool Use System Implementation Plan for Signal Bot TEE

## Executive Summary

This plan outlines the implementation of a tool use (function calling) system for the Signal TEE bot. The system will allow the LLM to invoke external tools like web search, weather lookup, and calculations, with results incorporated back into the conversation.

## Current Architecture Analysis

### Key Findings

1. **NEAR AI Client** (`crates/near-ai-client/src/client.rs`):
   - Uses OpenAI-compatible API with `ChatRequest` / `ChatResponse` types
   - Current `Message` type only supports `role` and `content` fields
   - No tool/function calling support currently implemented

2. **Chat Handler** (`crates/signal-bot/src/commands/chat.rs`):
   - Single round-trip: user message -> NEAR AI -> response
   - Stores messages with role and content only
   - No loop for tool execution

3. **Conversation Store** (`crates/conversation-store/src/types.rs`):
   - `StoredMessage` has `role` and `content` only
   - `OpenAiMessage` similarly limited
   - Needs extension for tool calls and tool results

---

## Implementation Plan

### Phase 1: Core Types and Infrastructure

#### 1.1 Create New Crate: `tools`

Create `crates/tools/` with the tool abstraction layer.

**Structure:**
```
crates/tools/
  Cargo.toml
  src/
    lib.rs           # Public exports
    types.rs         # Tool definitions, parameters, results
    registry.rs      # Tool registry (enable/disable, lookup)
    executor.rs      # Execute tools, handle errors
    error.rs         # Tool-specific errors
```

**Key Types** (`types.rs`):
```rust
/// Tool definition following OpenAI function calling schema
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,  // Always "function"
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}

/// Tool call from the LLM
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,  // JSON string
}

/// Result of executing a tool
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub success: bool,
}
```

**Tool Trait** (`lib.rs`):
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn name(&self) -> &str;
    async fn execute(&self, arguments: &str) -> Result<String, ToolError>;
}
```

#### 1.2 Extend NEAR AI Client Types

Modify `crates/near-ai-client/src/types.rs`:

```rust
/// Extended message with optional tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Add Tool role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Extended chat request with tools
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Mode(String),  // "auto", "none", "required"
    Specific { #[serde(rename = "type")] tool_type: String, function: FunctionRef },
}
```

#### 1.3 Extend Conversation Store

Modify `crates/conversation-store/src/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub role: String,
    pub content: Option<String>,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StoredToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
```

---

### Phase 2: Implement Specific Tools

#### 2.1 Web Search Tool

Create `crates/tools/src/web_search.rs`

**Recommended API: Brave Search API**
- Privacy-focused (aligns with TEE privacy model)
- Good free tier (2000 queries/month)
- Simple REST API
- No tracking

```rust
pub struct WebSearchTool {
    client: Client,
    api_key: SecretString,
    max_results: usize,
}

impl WebSearchTool {
    pub fn new(api_key: impl Into<String>, max_results: usize) -> Self {
        // Initialize with Brave Search API key
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "web_search".into(),
                description: "Search the web for current information".into(),
                parameters: json!({
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
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: WebSearchArgs = serde_json::from_str(arguments)?;

        let response = self.client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", self.api_key.expose_secret())
            .query(&[("q", &args.query), ("count", &self.max_results.to_string())])
            .send()
            .await?;

        // Parse and format results
        let results: BraveSearchResponse = response.json().await?;
        Ok(format_search_results(&results))
    }
}
```

**Alternative APIs (if Brave not suitable):**
- **SerpAPI**: More comprehensive, pay-per-query
- **Tavily**: AI-optimized search, good summaries
- **DuckDuckGo Instant Answer**: Free, but limited

#### 2.2 Weather Tool

Create `crates/tools/src/weather.rs`

**Recommended API: Open-Meteo**
- Free, no API key required
- No tracking
- Good for TEE (no auth secrets to protect)

```rust
pub struct WeatherTool {
    client: Client,
}

#[async_trait]
impl Tool for WeatherTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_weather".into(),
                description: "Get current weather for a location".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "City name or coordinates"
                        }
                    },
                    "required": ["location"]
                }),
            },
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        // 1. Geocode location using Open-Meteo geocoding API
        // 2. Fetch weather data
        // 3. Format response
    }
}
```

#### 2.3 Calculator Tool

Create `crates/tools/src/calculator.rs`

**No external API needed - pure Rust computation:**

```rust
pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "calculate".into(),
                description: "Evaluate mathematical expressions".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "Math expression to evaluate"
                        }
                    },
                    "required": ["expression"]
                }),
            },
        }
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: CalculatorArgs = serde_json::from_str(arguments)?;
        // Use `meval` or `evalexpr` crate for safe expression evaluation
        let result = meval::eval_str(&args.expression)?;
        Ok(result.to_string())
    }
}
```

#### 2.4 URL Fetch Tool (Optional)

For fetching and summarizing web page content:

```rust
pub struct UrlFetchTool {
    client: Client,
    max_content_length: usize,
}

// Fetches URL, extracts text content, truncates to limit
```

---

### Phase 3: Tool Registry and Configuration

#### 3.1 Tool Registry

Create `crates/tools/src/registry.rs`:

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    enabled: HashSet<String>,
}

impl ToolRegistry {
    pub fn new() -> Self { ... }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name.clone(), tool);
        self.enabled.insert(name);  // Enabled by default
    }

    pub fn enable(&mut self, name: &str) { ... }
    pub fn disable(&mut self, name: &str) { ... }

    pub fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|(name, _)| self.enabled.contains(*name))
            .map(|(_, tool)| tool.definition())
            .collect()
    }

    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.enabled.contains(name)
            .then(|| self.tools.get(name).cloned())
            .flatten()
    }
}
```

#### 3.2 Configuration Extension

Modify `crates/signal-bot/src/config.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsConfig {
    /// Enable tool use system
    #[serde(default = "default_tools_enabled")]
    pub enabled: bool,

    /// Maximum tool calls per message
    #[serde(default = "default_max_tool_calls")]
    pub max_tool_calls: usize,

    /// Web search configuration
    #[serde(default)]
    pub web_search: WebSearchConfig,

    /// Weather tool configuration
    #[serde(default)]
    pub weather: WeatherConfig,

    /// Calculator tool configuration
    #[serde(default)]
    pub calculator: CalculatorConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebSearchConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub max_results: usize,
}

// Defaults
fn default_tools_enabled() -> bool { true }
fn default_max_tool_calls() -> usize { 5 }
```

**Environment Variables:**
```
TOOLS__ENABLED=true
TOOLS__MAX_TOOL_CALLS=5
TOOLS__WEB_SEARCH__ENABLED=true
TOOLS__WEB_SEARCH__API_KEY=brave-api-key
TOOLS__WEB_SEARCH__MAX_RESULTS=5
TOOLS__WEATHER__ENABLED=true
TOOLS__CALCULATOR__ENABLED=true
```

---

### Phase 4: Chat Handler with Tool Loop

#### 4.1 New Chat Handler with Tool Execution

Modify `crates/signal-bot/src/commands/chat.rs`:

```rust
pub struct ChatHandler {
    near_ai: Arc<NearAiClient>,
    conversations: Arc<ConversationStore>,
    tool_registry: Arc<ToolRegistry>,
    system_prompt: String,
    max_tool_iterations: usize,
}

#[async_trait]
impl CommandHandler for ChatHandler {
    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let conversation_id = message.reply_target();

        // Add user message
        self.conversations.add_message(
            conversation_id, "user", &message.text, Some(&self.system_prompt)
        ).await?;

        // Get tool definitions
        let tools = self.tool_registry.get_definitions();

        // Tool execution loop
        for iteration in 0..self.max_tool_iterations {
            let messages = self.build_messages(conversation_id).await?;

            let response = self.near_ai.chat_with_tools(
                messages,
                Some(0.7),
                None,
                Some(&tools),
                Some(ToolChoice::Mode("auto".into())),
            ).await?;

            // Check if model wants to call tools
            if let Some(tool_calls) = &response.tool_calls {
                if tool_calls.is_empty() {
                    // No tool calls, return content
                    return self.finalize_response(conversation_id, &response).await;
                }

                // Store assistant message with tool calls
                self.conversations.add_assistant_with_tools(
                    conversation_id,
                    response.content.as_deref(),
                    tool_calls,
                ).await?;

                // Execute each tool
                for tool_call in tool_calls {
                    let result = self.execute_tool(tool_call).await;

                    // Store tool result
                    self.conversations.add_tool_result(
                        conversation_id,
                        &tool_call.id,
                        &result.content,
                    ).await?;
                }

                // Continue loop for next iteration
            } else {
                // No tool calls, return final response
                return self.finalize_response(conversation_id, &response).await;
            }
        }

        // Max iterations reached
        Ok("I've reached the limit for tool calls. Here's what I found so far...".into())
    }

    async fn execute_tool(&self, tool_call: &ToolCall) -> ToolResult {
        match self.tool_registry.get_tool(&tool_call.function.name) {
            Some(tool) => {
                match tool.execute(&tool_call.function.arguments).await {
                    Ok(content) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content,
                        success: true,
                    },
                    Err(e) => ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        content: format!("Error: {}", e),
                        success: false,
                    },
                }
            }
            None => ToolResult {
                tool_call_id: tool_call.id.clone(),
                content: format!("Tool '{}' not found or disabled", tool_call.function.name),
                success: false,
            },
        }
    }
}
```

#### 4.2 Extend NEAR AI Client

Add to `crates/near-ai-client/src/client.rs`:

```rust
/// Chat with tool support
pub async fn chat_with_tools(
    &self,
    messages: Vec<Message>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    tools: Option<&[ToolDefinition]>,
    tool_choice: Option<ToolChoice>,
) -> Result<ChatResponseWithTools, NearAiError> {
    let request = ChatRequest {
        model: self.model.clone(),
        messages,
        temperature,
        max_tokens,
        stream: Some(false),
        tools: tools.map(|t| t.to_vec()),
        tool_choice,
    };

    // ... send request and parse response
}

/// Response that may contain tool calls
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponseWithTools {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: String,
}
```

---

### Phase 5: Security Considerations

#### 5.1 TEE Security Analysis

**Safe Tools (minimal security impact):**
- **Calculator**: Pure computation, no external access
- **Weather (Open-Meteo)**: No API key, read-only public data
- **Time/Date**: System clock access only

**Tools Requiring API Keys:**
- **Web Search (Brave)**: API key stored as `SecretString`
  - Key managed same as NEAR AI API key
  - Encrypted in TEE memory
  - Never logged

**Network Metadata Leakage:**
Tool execution reveals to the operator:
- Which tools were called (via network traffic patterns)
- Timing of tool calls
- Size of requests/responses

This is **acceptable** given the existing metadata leakage from NEAR AI requests documented in CLAUDE.md.

#### 5.2 Tool Sandboxing

```rust
pub struct ToolExecutor {
    timeout: Duration,
    max_response_size: usize,
}

impl ToolExecutor {
    pub async fn execute(&self, tool: &dyn Tool, args: &str) -> Result<String, ToolError> {
        // Timeout wrapper
        let result = tokio::time::timeout(
            self.timeout,
            tool.execute(args)
        ).await
        .map_err(|_| ToolError::Timeout)?;

        // Truncate oversized responses
        let content = result?;
        if content.len() > self.max_response_size {
            Ok(content[..self.max_response_size].to_string() + "... [truncated]")
        } else {
            Ok(content)
        }
    }
}
```

#### 5.3 Input Validation

Each tool validates its input arguments:

```rust
// Web search - sanitize query
fn sanitize_query(query: &str) -> String {
    query
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".,!?-".contains(*c))
        .take(500)
        .collect()
}

// Calculator - use safe expression parser
// meval crate provides sandboxed evaluation (no system access)
```

#### 5.4 Rate Limiting

```rust
pub struct RateLimiter {
    calls_per_minute: usize,
    calls: Arc<RwLock<VecDeque<Instant>>>,
}

impl RateLimiter {
    pub async fn check(&self) -> bool {
        let mut calls = self.calls.write().await;
        let now = Instant::now();

        // Remove calls older than 1 minute
        while calls.front().map_or(false, |t| now.duration_since(*t) > Duration::from_secs(60)) {
            calls.pop_front();
        }

        if calls.len() < self.calls_per_minute {
            calls.push_back(now);
            true
        } else {
            false
        }
    }
}
```

---

### Phase 6: Error Handling

#### 6.1 Tool Error Types

Create `crates/tools/src/error.rs`:

```rust
#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool execution timeout")]
    Timeout,

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Tool disabled: {0}")]
    Disabled(String),

    #[error("External service error: {0}")]
    ExternalService(String),
}
```

#### 6.2 Graceful Degradation

When tools fail, the system should:
1. Return a descriptive error message to the LLM
2. Allow the LLM to continue without the tool result
3. Log the error for debugging

```rust
async fn execute_tool(&self, tool_call: &ToolCall) -> ToolResult {
    let result = match self.tool_registry.get_tool(&tool_call.function.name) {
        Some(tool) => {
            match self.executor.execute(&*tool, &tool_call.function.arguments).await {
                Ok(content) => ToolResult::success(&tool_call.id, content),
                Err(ToolError::Timeout) => {
                    warn!("Tool {} timed out", tool_call.function.name);
                    ToolResult::error(&tool_call.id, "Tool execution timed out")
                }
                Err(ToolError::RateLimit) => {
                    warn!("Tool {} rate limited", tool_call.function.name);
                    ToolResult::error(&tool_call.id, "Rate limit exceeded, try again later")
                }
                Err(e) => {
                    error!("Tool {} error: {}", tool_call.function.name, e);
                    ToolResult::error(&tool_call.id, &format!("Error: {}", e))
                }
            }
        }
        None => ToolResult::error(&tool_call.id, "Tool not available"),
    };

    result
}
```

---

### Phase 7: Testing Strategy

#### 7.1 Unit Tests

```rust
// Tool definition tests
#[test]
fn test_web_search_definition() {
    let tool = WebSearchTool::new("test-key", 5);
    let def = tool.definition();
    assert_eq!(def.function.name, "web_search");
    assert!(def.function.parameters.get("properties").is_some());
}

// Tool execution tests (with mocks)
#[tokio::test]
async fn test_calculator_execute() {
    let tool = CalculatorTool;
    let result = tool.execute(r#"{"expression": "2 + 2"}"#).await.unwrap();
    assert_eq!(result, "4");
}
```

#### 7.2 Integration Tests

```rust
#[tokio::test]
async fn test_tool_loop_single_call() {
    // Mock NEAR AI to return tool call, then final response
    // Verify tool was executed
    // Verify final response incorporates tool result
}

#[tokio::test]
async fn test_tool_loop_multiple_calls() {
    // Test multiple sequential tool calls
}

#[tokio::test]
async fn test_tool_failure_graceful() {
    // Test that tool failure doesn't crash the handler
}
```

---

### Phase 8: Deployment Updates

#### 8.1 Docker Compose Changes

Update `docker/docker-compose.yaml`:

```yaml
signal-bot:
  environment:
    # ... existing vars ...
    - TOOLS__ENABLED=${TOOLS_ENABLED:-true}
    - TOOLS__MAX_TOOL_CALLS=${TOOLS_MAX_TOOL_CALLS:-5}
    - TOOLS__WEB_SEARCH__ENABLED=${TOOLS_WEB_SEARCH_ENABLED:-true}
    - TOOLS__WEB_SEARCH__API_KEY=${BRAVE_API_KEY}
    - TOOLS__WEB_SEARCH__MAX_RESULTS=${TOOLS_WEB_SEARCH_MAX_RESULTS:-5}
    - TOOLS__WEATHER__ENABLED=${TOOLS_WEATHER_ENABLED:-true}
    - TOOLS__CALCULATOR__ENABLED=${TOOLS_CALCULATOR_ENABLED:-true}
```

#### 8.2 Documentation Updates

Update `README.md`:

```markdown
## Tools

The bot supports tool use for enhanced capabilities:

| Tool | Description | API Required |
|------|-------------|--------------|
| `web_search` | Search the web | Brave Search API key |
| `get_weather` | Current weather | None (Open-Meteo) |
| `calculate` | Math expressions | None |

Tools are invoked automatically by the AI when needed.

### Configuration

```env
TOOLS__ENABLED=true
TOOLS__WEB_SEARCH__API_KEY=your-brave-api-key
```

### Security

- Tool API keys are stored in TEE-protected memory
- Tool execution happens entirely within the TEE
- Network metadata (which tools, when) is visible to operator
```

---

## Implementation Sequence

1. **Phase 1: Core Infrastructure**
   - Create `tools` crate with types and traits
   - Extend `near-ai-client` types for tool calling
   - Extend `conversation-store` for tool messages

2. **Phase 2: Basic Tools**
   - Implement `CalculatorTool` (no external deps)
   - Implement `WeatherTool` (Open-Meteo)
   - Write unit tests

3. **Phase 3: Web Search**
   - Implement `WebSearchTool` (Brave API)
   - Add API key configuration
   - Integration tests

4. **Phase 4: Chat Handler Integration**
   - Modify `ChatHandler` with tool loop
   - Add `ToolRegistry` initialization in main
   - End-to-end testing

5. **Phase 5: Security & Polish**
   - Add rate limiting
   - Add timeout handling
   - Update documentation
   - Security review

---

## Recommended Additional Tools (Future)

1. **URL Summarizer**: Fetch and summarize web pages
2. **Code Executor**: Safe sandboxed Python/JS execution (complex, security-sensitive)
3. **Image Generation**: Via NEAR AI or external API
4. **Translation**: Language translation service
5. **News Headlines**: Current news feed

---

## Critical Files for Implementation

- `crates/near-ai-client/src/types.rs` - Must extend Message and ChatRequest types to support tool_calls, tool_call_id, tools, and tool_choice fields
- `crates/near-ai-client/src/client.rs` - Add chat_with_tools() method that handles tool calling responses
- `crates/signal-bot/src/commands/chat.rs` - Rewrite execute() with tool execution loop, add tool registry dependency
- `crates/conversation-store/src/types.rs` - Extend StoredMessage to include optional tool_calls and tool_call_id fields
- `crates/signal-bot/src/config.rs` - Add ToolsConfig section with per-tool enable/disable and API key configuration
