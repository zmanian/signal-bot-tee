# Tool Use System Design

**Date:** 2024-12-15
**Status:** Approved

## Overview

Implement a tool use (function calling) system for the Signal TEE bot, enabling the LLM to invoke external tools like web search, weather lookup, and calculations.

## Decisions Made

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Web Search API | Brave Search | Privacy-focused, free tier (2000/month), aligns with TEE privacy model |
| System Prompt | Comprehensive | Cover tools, security awareness, conversational personality |
| Progress Indicators | Yes | Send "🔧 Using..." messages during tool execution for mobile UX |
| Default State | Enabled | Calculator + Weather on by default, Web Search requires API key |
| Timezone | UTC only | Keep simple, users can mention their timezone |

## Architecture

### New Crate Structure

```
crates/tools/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Tool trait, registry, executor exports
│   ├── types.rs         # ToolDefinition, ToolCall, ToolResult
│   ├── registry.rs      # ToolRegistry - manages available tools
│   ├── executor.rs      # ToolExecutor - timeout, rate limiting
│   ├── error.rs         # ToolError enum
│   └── builtin/
│       ├── mod.rs
│       ├── calculator.rs   # Pure Rust math (meval crate)
│       ├── weather.rs      # Open-Meteo API (free, no key)
│       └── web_search.rs   # Brave Search API
```

### Data Flow

```
User Message
    ↓
ChatHandler (tool loop)
    ↓
NEAR AI (with tool definitions) ──→ returns tool_calls?
    ↓                                    ↓ yes
    ↓ no                           ToolExecutor.execute()
    ↓                                    ↓
    ↓                              Send progress message ("🔧 Using...")
    ↓                                    ↓
    ↓                              Add tool results to conversation
    ↓                                    ↓
    ↓                              Loop back to NEAR AI
    ↓
Final response to user
```

## Core Types (OpenAI-Compatible)

### Tool Definition (sent to NEAR AI)

```rust
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
```

### Tool Call (from NEAR AI response)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,  // "function"
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,  // JSON string to parse
}
```

### Extended Message (for tool role)

```rust
pub enum Role {
    System,
    User,
    Assistant,
    Tool,  // NEW
}

pub struct Message {
    pub role: Role,
    pub content: Option<String>,           // Optional for assistant with tool_calls
    pub tool_calls: Option<Vec<ToolCall>>, // Assistant requesting tools
    pub tool_call_id: Option<String>,      // Tool response reference
}
```

## System Prompt

```
You are an AI assistant accessible via Signal, running in a Trusted Execution
Environment (TEE) for privacy protection.

Current date and time: {dynamic UTC timestamp}

## Privacy & Security
- Your conversations are protected by Intel TDX hardware encryption
- Neither the bot operator nor the AI provider can read your messages
- Users can verify this by sending "!verify" for cryptographic attestation

## Available Tools
You have access to these tools - use them when helpful:
- **web_search**: Search the web for current information, news, facts
- **get_weather**: Get current weather for any location
- **calculate**: Evaluate math expressions accurately

## Guidelines
- Be concise - this is mobile chat, not essays
- Use tools proactively for current information (don't guess dates, prices, weather)
- For calculations, use the calculate tool rather than mental math
- If a tool fails, explain what happened and try to help anyway
- Never fabricate search results or weather data
```

Dynamic timestamp injected on each message:
```rust
fn build_system_prompt(base_prompt: &str) -> String {
    let now = chrono::Utc::now();
    format!(
        "{}\n\nCurrent date and time: {} UTC",
        base_prompt,
        now.format("%A, %B %d, %Y at %H:%M")
    )
}
```

## Tool Implementations

### Calculator Tool (Pure Rust)

```rust
// Uses meval crate - safe sandboxed evaluation
// Handles: 2+2, sqrt(16), sin(pi/2), 2^10

fn definition() -> ToolDefinition {
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
                        "description": "Math expression to evaluate (e.g., '2+2', 'sqrt(16)')"
                    }
                },
                "required": ["expression"]
            }),
        },
    }
}
```

### Weather Tool (Open-Meteo)

- Free API, no key required
- Two-step: geocode location → fetch weather
- Endpoint: `https://api.open-meteo.com/v1/forecast`

### Web Search Tool (Brave Search)

- Requires `TOOLS__WEB_SEARCH__API_KEY`
- Endpoint: `https://api.search.brave.com/res/v1/web/search`
- Returns top 5 results with title, snippet, URL

## Configuration

### Config Structure

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_max_tool_calls")]
    pub max_tool_calls: usize,  // default: 5

    #[serde(default)]
    pub web_search: WebSearchConfig,

    #[serde(default)]
    pub weather: WeatherConfig,

    #[serde(default)]
    pub calculator: CalculatorConfig,
}
```

### Environment Variables

```bash
# Tools master switch
TOOLS__ENABLED=true
TOOLS__MAX_TOOL_CALLS=5

# Web search (requires API key)
TOOLS__WEB_SEARCH__ENABLED=true
TOOLS__WEB_SEARCH__API_KEY=your-brave-api-key
TOOLS__WEB_SEARCH__MAX_RESULTS=5

# Weather (no key needed)
TOOLS__WEATHER__ENABLED=true

# Calculator (no key needed)
TOOLS__CALCULATOR__ENABLED=true
```

## Error Handling

### Error Types

```rust
#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool execution timeout")]
    Timeout,

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Tool not configured: {0}")]
    NotConfigured(String),
}
```

### Graceful Degradation

- Tool errors become tool results for the AI to interpret
- AI explains failures naturally to users
- Missing API keys disable specific tools, not the whole system
- Default 10-second timeout per tool execution

## Files to Modify

| File | Changes |
|------|---------|
| `crates/near-ai-client/src/types.rs` | Add Tool role, tool_calls, tool_call_id fields |
| `crates/near-ai-client/src/client.rs` | Add `chat_with_tools()` method |
| `crates/conversation-store/src/types.rs` | Add tool_calls to StoredMessage |
| `crates/signal-bot/src/config.rs` | Add ToolsConfig + improved system prompt |
| `crates/signal-bot/src/commands/chat.rs` | Tool execution loop with progress messages |

## New Files

- `crates/tools/Cargo.toml`
- `crates/tools/src/lib.rs`
- `crates/tools/src/types.rs`
- `crates/tools/src/registry.rs`
- `crates/tools/src/executor.rs`
- `crates/tools/src/error.rs`
- `crates/tools/src/builtin/mod.rs`
- `crates/tools/src/builtin/calculator.rs`
- `crates/tools/src/builtin/weather.rs`
- `crates/tools/src/builtin/web_search.rs`

## External Dependencies

### Brave Search API Setup

1. Go to https://brave.com/search/api/
2. Click "Get Started for Free"
3. Create account and verify email
4. Generate API key from dashboard
5. Free tier: 2,000 queries/month

### New Cargo Dependencies

- `meval` - Safe math expression evaluation (calculator)
- `async-trait` - Async trait support (already in use)
- `thiserror` - Error derive macro (already in use)
