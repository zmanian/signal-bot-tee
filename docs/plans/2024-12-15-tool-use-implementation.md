# Tool Use System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable the Signal bot to call external tools (calculator, weather, web search) during conversations.

**Architecture:** Create a `tools` crate with trait-based tool definitions, extend NEAR AI client for function calling, modify chat handler with tool execution loop, add progress messages to Signal.

**Tech Stack:** Rust, async-trait, meval (calculator), reqwest (HTTP), serde_json (tool schemas)

---

## Phase 1: Core Types (crates/tools)

### Task 1: Create tools crate structure

**Files:**
- Create: `crates/tools/Cargo.toml`
- Create: `crates/tools/src/lib.rs`
- Modify: `Cargo.toml` (workspace)

**Step 1: Create Cargo.toml for tools crate**

```toml
[package]
name = "tools"
version = "0.1.0"
edition = "2021"

[dependencies]
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
reqwest = { version = "0.11", features = ["json"] }
secrecy = "0.8"
tracing = "0.1"
tokio = { version = "1", features = ["time"] }
meval = "0.2"

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
wiremock = "0.5"
```

**Step 2: Create minimal lib.rs**

```rust
//! Tool use system for Signal bot.

mod error;
mod types;
mod registry;
mod executor;
pub mod builtin;

pub use error::ToolError;
pub use types::*;
pub use registry::ToolRegistry;
pub use executor::ToolExecutor;
```

**Step 3: Add to workspace Cargo.toml**

Find the `[workspace]` section and add `"crates/tools"` to the members list.

**Step 4: Create placeholder modules**

Create empty files:
- `crates/tools/src/error.rs` with `// TODO`
- `crates/tools/src/types.rs` with `// TODO`
- `crates/tools/src/registry.rs` with `// TODO`
- `crates/tools/src/executor.rs` with `// TODO`
- `crates/tools/src/builtin/mod.rs` with `// TODO`

**Step 5: Verify it compiles**

Run: `cargo check -p tools`
Expected: Compiles with no errors

**Step 6: Commit**

```bash
git add crates/tools Cargo.toml
git commit -m "feat(tools): create tools crate skeleton"
```

---

### Task 2: Implement ToolError

**Files:**
- Modify: `crates/tools/src/error.rs`

**Step 1: Write error types**

```rust
//! Tool execution errors.

use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Error, Debug)]
pub enum ToolError {
    /// Tool execution timed out.
    #[error("Tool execution timed out after {0} seconds")]
    Timeout(u64),

    /// Invalid arguments provided to tool.
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parsing failed.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded")]
    RateLimit,

    /// Tool is not configured (missing API key, etc.).
    #[error("Tool not configured: {0}")]
    NotConfigured(String),

    /// External service returned an error.
    #[error("External service error: {0}")]
    ExternalService(String),

    /// Math evaluation error.
    #[error("Math evaluation error: {0}")]
    MathError(String),
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p tools`
Expected: Compiles

**Step 3: Commit**

```bash
git add crates/tools/src/error.rs
git commit -m "feat(tools): add ToolError types"
```

---

### Task 3: Implement core types (ToolDefinition, ToolCall, ToolResult)

**Files:**
- Modify: `crates/tools/src/types.rs`

**Step 1: Write type definitions**

```rust
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
```

**Step 2: Verify it compiles**

Run: `cargo check -p tools`
Expected: Compiles

**Step 3: Commit**

```bash
git add crates/tools/src/types.rs
git commit -m "feat(tools): add core types (ToolDefinition, ToolCall, ToolResult, Tool trait)"
```

---

### Task 4: Implement ToolRegistry

**Files:**
- Modify: `crates/tools/src/registry.rs`

**Step 1: Write registry implementation**

```rust
//! Tool registry for managing available tools.

use crate::types::{Tool, ToolDefinition};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Registry of available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    enabled: HashSet<String>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            enabled: HashSet::new(),
        }
    }

    /// Register a tool (enabled by default).
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name.clone(), tool);
        self.enabled.insert(name);
    }

    /// Enable a tool by name.
    pub fn enable(&mut self, name: &str) {
        if self.tools.contains_key(name) {
            self.enabled.insert(name.to_string());
        }
    }

    /// Disable a tool by name.
    pub fn disable(&mut self, name: &str) {
        self.enabled.remove(name);
    }

    /// Check if a tool is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled.contains(name)
    }

    /// Get definitions for all enabled tools.
    pub fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|(name, _)| self.enabled.contains(*name))
            .map(|(_, tool)| tool.definition())
            .collect()
    }

    /// Get a tool by name (only if enabled).
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        if self.enabled.contains(name) {
            self.tools.get(name).cloned()
        } else {
            None
        }
    }

    /// List all registered tool names.
    pub fn list_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// List enabled tool names.
    pub fn list_enabled(&self) -> Vec<&str> {
        self.enabled.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FunctionDefinition, ToolDefinition};
    use async_trait::async_trait;
    use crate::error::ToolError;

    struct MockTool {
        name: String,
    }

    #[async_trait]
    impl Tool for MockTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                tool_type: "function".into(),
                function: FunctionDefinition {
                    name: self.name.clone(),
                    description: "Mock tool".into(),
                    parameters: serde_json::json!({}),
                },
            }
        }

        fn name(&self) -> &str {
            &self.name
        }

        async fn execute(&self, _arguments: &str) -> Result<String, ToolError> {
            Ok("mock result".into())
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(MockTool { name: "test".into() });
        registry.register(tool);

        assert!(registry.get_tool("test").is_some());
        assert!(registry.is_enabled("test"));
    }

    #[test]
    fn test_disable_tool() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(MockTool { name: "test".into() });
        registry.register(tool);

        registry.disable("test");
        assert!(registry.get_tool("test").is_none());
        assert!(!registry.is_enabled("test"));
    }

    #[test]
    fn test_get_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(MockTool { name: "tool1".into() }));
        registry.register(Arc::new(MockTool { name: "tool2".into() }));
        registry.disable("tool2");

        let defs = registry.get_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].function.name, "tool1");
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p tools`
Expected: All tests pass

**Step 3: Commit**

```bash
git add crates/tools/src/registry.rs
git commit -m "feat(tools): add ToolRegistry with enable/disable"
```

---

### Task 5: Implement ToolExecutor

**Files:**
- Modify: `crates/tools/src/executor.rs`

**Step 1: Write executor implementation**

```rust
//! Tool executor with timeout and error handling.

use crate::error::ToolError;
use crate::registry::ToolRegistry;
use crate::types::{ToolCall, ToolResult};
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
```

**Step 2: Run tests**

Run: `cargo test -p tools`
Expected: All tests pass

**Step 3: Commit**

```bash
git add crates/tools/src/executor.rs
git commit -m "feat(tools): add ToolExecutor with timeout handling"
```

---

## Phase 2: Built-in Tools

### Task 6: Implement Calculator Tool

**Files:**
- Create: `crates/tools/src/builtin/calculator.rs`
- Modify: `crates/tools/src/builtin/mod.rs`

**Step 1: Create calculator implementation**

```rust
//! Calculator tool using meval for safe expression evaluation.

use crate::error::ToolError;
use crate::types::{FunctionDefinition, Tool, ToolDefinition};
use async_trait::async_trait;
use serde::Deserialize;

/// Calculator tool for evaluating math expressions.
pub struct CalculatorTool;

#[derive(Deserialize)]
struct CalculatorArgs {
    expression: String,
}

impl CalculatorTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CalculatorTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CalculatorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "calculate".into(),
                description: "Evaluate mathematical expressions. Supports basic arithmetic (+, -, *, /), exponents (^), parentheses, and functions like sqrt(), sin(), cos(), tan(), log(), ln(), abs(), floor(), ceil().".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "Mathematical expression to evaluate (e.g., '2 + 2', 'sqrt(16)', '2^10')"
                        }
                    },
                    "required": ["expression"]
                }),
            },
        }
    }

    fn name(&self) -> &str {
        "calculate"
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: CalculatorArgs = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let expression = args.expression.trim();

        // Validate expression isn't empty
        if expression.is_empty() {
            return Err(ToolError::InvalidArguments("Empty expression".into()));
        }

        // Evaluate using meval
        let result = meval::eval_str(expression)
            .map_err(|e| ToolError::MathError(e.to_string()))?;

        // Format result nicely
        if result.fract() == 0.0 && result.abs() < 1e15 {
            Ok(format!("{} = {}", expression, result as i64))
        } else {
            Ok(format!("{} = {}", expression, result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_arithmetic() {
        let tool = CalculatorTool::new();

        let result = tool.execute(r#"{"expression": "2 + 2"}"#).await.unwrap();
        assert!(result.contains("= 4"));

        let result = tool.execute(r#"{"expression": "10 * 5"}"#).await.unwrap();
        assert!(result.contains("= 50"));

        let result = tool.execute(r#"{"expression": "100 / 4"}"#).await.unwrap();
        assert!(result.contains("= 25"));
    }

    #[tokio::test]
    async fn test_functions() {
        let tool = CalculatorTool::new();

        let result = tool.execute(r#"{"expression": "sqrt(16)"}"#).await.unwrap();
        assert!(result.contains("= 4"));

        let result = tool.execute(r#"{"expression": "2^10"}"#).await.unwrap();
        assert!(result.contains("= 1024"));
    }

    #[tokio::test]
    async fn test_complex_expression() {
        let tool = CalculatorTool::new();

        let result = tool.execute(r#"{"expression": "(2 + 3) * 4"}"#).await.unwrap();
        assert!(result.contains("= 20"));
    }

    #[tokio::test]
    async fn test_invalid_expression() {
        let tool = CalculatorTool::new();

        let result = tool.execute(r#"{"expression": "2 +"}"#).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_definition() {
        let tool = CalculatorTool::new();
        let def = tool.definition();

        assert_eq!(def.tool_type, "function");
        assert_eq!(def.function.name, "calculate");
    }
}
```

**Step 2: Update builtin/mod.rs**

```rust
//! Built-in tools.

mod calculator;

pub use calculator::CalculatorTool;
```

**Step 3: Run tests**

Run: `cargo test -p tools`
Expected: All tests pass

**Step 4: Commit**

```bash
git add crates/tools/src/builtin/
git commit -m "feat(tools): add CalculatorTool with meval"
```

---

### Task 7: Implement Weather Tool

**Files:**
- Create: `crates/tools/src/builtin/weather.rs`
- Modify: `crates/tools/src/builtin/mod.rs`

**Step 1: Create weather implementation**

```rust
//! Weather tool using Open-Meteo API (free, no API key required).

use crate::error::ToolError;
use crate::types::{FunctionDefinition, Tool, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

/// Weather tool using Open-Meteo API.
pub struct WeatherTool {
    client: Client,
}

#[derive(Deserialize)]
struct WeatherArgs {
    location: String,
}

#[derive(Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
}

#[derive(Deserialize)]
struct WeatherResponse {
    current_weather: CurrentWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    temperature: f64,
    windspeed: f64,
    weathercode: i32,
}

impl WeatherTool {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn weather_code_to_description(code: i32) -> &'static str {
        match code {
            0 => "Clear sky",
            1 | 2 | 3 => "Partly cloudy",
            45 | 48 => "Foggy",
            51 | 53 | 55 => "Drizzle",
            61 | 63 | 65 => "Rain",
            66 | 67 => "Freezing rain",
            71 | 73 | 75 => "Snow",
            77 => "Snow grains",
            80 | 81 | 82 => "Rain showers",
            85 | 86 => "Snow showers",
            95 => "Thunderstorm",
            96 | 99 => "Thunderstorm with hail",
            _ => "Unknown",
        }
    }
}

impl Default for WeatherTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WeatherTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_weather".into(),
                description: "Get current weather for a location. Returns temperature, conditions, and wind speed.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "City name (e.g., 'San Francisco', 'London', 'Tokyo')"
                        }
                    },
                    "required": ["location"]
                }),
            },
        }
    }

    fn name(&self) -> &str {
        "get_weather"
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: WeatherArgs = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let location = args.location.trim();
        if location.is_empty() {
            return Err(ToolError::InvalidArguments("Empty location".into()));
        }

        // Step 1: Geocode the location
        debug!(location = %location, "Geocoding location");
        let geocode_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
            urlencoding::encode(location)
        );

        let geo_response: GeocodingResponse = self
            .client
            .get(&geocode_url)
            .send()
            .await?
            .json()
            .await?;

        let geo = geo_response
            .results
            .and_then(|r| r.into_iter().next())
            .ok_or_else(|| ToolError::ExternalService(format!("Location '{}' not found", location)))?;

        // Step 2: Get weather data
        debug!(lat = geo.latitude, lon = geo.longitude, "Fetching weather");
        let weather_url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
            geo.latitude, geo.longitude
        );

        let weather: WeatherResponse = self
            .client
            .get(&weather_url)
            .send()
            .await?
            .json()
            .await?;

        // Format nice location name
        let location_name = match (&geo.admin1, &geo.country) {
            (Some(admin), Some(country)) => format!("{}, {}, {}", geo.name, admin, country),
            (None, Some(country)) => format!("{}, {}", geo.name, country),
            _ => geo.name,
        };

        let description = Self::weather_code_to_description(weather.current_weather.weathercode);
        let temp_f = weather.current_weather.temperature * 9.0 / 5.0 + 32.0;

        Ok(format!(
            "Weather in {}: {:.1}°C ({:.1}°F), {}. Wind: {:.1} km/h",
            location_name,
            weather.current_weather.temperature,
            temp_f,
            description,
            weather.current_weather.windspeed
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_code_descriptions() {
        assert_eq!(WeatherTool::weather_code_to_description(0), "Clear sky");
        assert_eq!(WeatherTool::weather_code_to_description(61), "Rain");
        assert_eq!(WeatherTool::weather_code_to_description(95), "Thunderstorm");
    }

    #[test]
    fn test_definition() {
        let tool = WeatherTool::new();
        let def = tool.definition();

        assert_eq!(def.tool_type, "function");
        assert_eq!(def.function.name, "get_weather");
    }

    // Integration test - requires network
    #[tokio::test]
    #[ignore] // Run with: cargo test -p tools -- --ignored
    async fn test_weather_integration() {
        let tool = WeatherTool::new();
        let result = tool.execute(r#"{"location": "San Francisco"}"#).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("San Francisco"));
        assert!(content.contains("°C"));
    }
}
```

**Step 2: Update builtin/mod.rs**

```rust
//! Built-in tools.

mod calculator;
mod weather;

pub use calculator::CalculatorTool;
pub use weather::WeatherTool;
```

**Step 3: Add urlencoding dependency to Cargo.toml**

Add to `[dependencies]` section: `urlencoding = "2.1"`

**Step 4: Run tests**

Run: `cargo test -p tools`
Expected: Unit tests pass (integration test is ignored)

**Step 5: Commit**

```bash
git add crates/tools/
git commit -m "feat(tools): add WeatherTool with Open-Meteo API"
```

---

### Task 8: Implement Web Search Tool

**Files:**
- Create: `crates/tools/src/builtin/web_search.rs`
- Modify: `crates/tools/src/builtin/mod.rs`

**Step 1: Create web search implementation**

```rust
//! Web search tool using Brave Search API.

use crate::error::ToolError;
use crate::types::{FunctionDefinition, Tool, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tracing::debug;

/// Web search tool using Brave Search API.
pub struct WebSearchTool {
    client: Client,
    api_key: SecretString,
    max_results: usize,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
}

#[derive(Deserialize)]
struct BraveSearchResponse {
    web: Option<WebResults>,
}

#[derive(Deserialize)]
struct WebResults {
    results: Vec<WebResult>,
}

#[derive(Deserialize)]
struct WebResult {
    title: String,
    url: String,
    description: Option<String>,
}

impl WebSearchTool {
    /// Create a new web search tool with Brave API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: SecretString::new(api_key.into()),
            max_results: 5,
        }
    }

    /// Set maximum number of results to return.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "web_search".into(),
                description: "Search the web for current information. Use for news, facts, prices, events, or anything that may have changed recently.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query (e.g., 'latest news about AI', 'weather in Tokyo')"
                        }
                    },
                    "required": ["query"]
                }),
            },
        }
    }

    fn name(&self) -> &str {
        "web_search"
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: SearchArgs = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let query = args.query.trim();
        if query.is_empty() {
            return Err(ToolError::InvalidArguments("Empty query".into()));
        }

        debug!(query = %query, "Performing web search");

        let response = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", self.api_key.expose_secret())
            .header("Accept", "application/json")
            .query(&[
                ("q", query),
                ("count", &self.max_results.to_string()),
            ])
            .send()
            .await?;

        if response.status() == 429 {
            return Err(ToolError::RateLimit);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ToolError::ExternalService(format!(
                "Brave Search API error: {} - {}",
                status, body
            )));
        }

        let search_response: BraveSearchResponse = response.json().await?;

        let results = search_response
            .web
            .map(|w| w.results)
            .unwrap_or_default();

        if results.is_empty() {
            return Ok(format!("No results found for '{}'", query));
        }

        // Format results
        let mut output = format!("Search results for '{}':\n\n", query);
        for (i, result) in results.iter().take(self.max_results).enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, result.title));
            if let Some(desc) = &result.description {
                output.push_str(&format!("   {}\n", desc));
            }
            output.push_str(&format!("   URL: {}\n\n", result.url));
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition() {
        let tool = WebSearchTool::new("test-key");
        let def = tool.definition();

        assert_eq!(def.tool_type, "function");
        assert_eq!(def.function.name, "web_search");
    }

    #[test]
    fn test_max_results_config() {
        let tool = WebSearchTool::new("test-key").with_max_results(10);
        assert_eq!(tool.max_results, 10);
    }

    // Integration test - requires valid API key
    #[tokio::test]
    #[ignore] // Run with: BRAVE_API_KEY=xxx cargo test -p tools -- --ignored
    async fn test_search_integration() {
        let api_key = std::env::var("BRAVE_API_KEY").expect("BRAVE_API_KEY not set");
        let tool = WebSearchTool::new(api_key);
        let result = tool.execute(r#"{"query": "rust programming language"}"#).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("rust"));
    }
}
```

**Step 2: Update builtin/mod.rs**

```rust
//! Built-in tools.

mod calculator;
mod weather;
mod web_search;

pub use calculator::CalculatorTool;
pub use weather::WeatherTool;
pub use web_search::WebSearchTool;
```

**Step 3: Run tests**

Run: `cargo test -p tools`
Expected: Unit tests pass

**Step 4: Commit**

```bash
git add crates/tools/
git commit -m "feat(tools): add WebSearchTool with Brave API"
```

---

## Phase 3: Extend NEAR AI Client

### Task 9: Add Tool role and tool_calls to Message type

**Files:**
- Modify: `crates/near-ai-client/src/types.rs`

**Step 1: Read current file**

Review current types.rs content to understand structure.

**Step 2: Update Role enum**

Add `Tool` variant to the Role enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}
```

**Step 3: Update Message struct**

Replace the simple Message struct with an extended version:

```rust
/// A single chat message with optional tool call support.
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

/// Tool call from LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}
```

**Step 4: Update Message constructors**

```rust
impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}
```

**Step 5: Add tool types to ChatRequest**

```rust
/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinitionApi,
}

/// Function definition for API.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinitionApi {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Chat completion request.
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
    pub tool_choice: Option<String>,
}
```

**Step 6: Update Choice to include tool_calls**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

/// Response message from API (may have tool_calls).
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMessage {
    pub role: Role,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}
```

**Step 7: Verify it compiles**

Run: `cargo check -p near-ai-client`
Expected: Compiles (may have warnings about unused fields)

**Step 8: Fix any broken tests**

Run: `cargo test -p near-ai-client`
Fix any tests that break due to the new Message structure.

**Step 9: Commit**

```bash
git add crates/near-ai-client/
git commit -m "feat(near-ai-client): add tool calling types to Message and ChatRequest"
```

---

### Task 10: Add chat_with_tools method to NearAiClient

**Files:**
- Modify: `crates/near-ai-client/src/client.rs`

**Step 1: Add chat_with_tools method**

Add this method to the NearAiClient impl block:

```rust
/// Send a chat completion request with tool support.
#[instrument(skip(self, messages, tools), fields(message_count = messages.len()))]
pub async fn chat_with_tools(
    &self,
    messages: Vec<Message>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    tools: Option<&[ToolDefinition]>,
) -> Result<ChatResponseWithTools, NearAiError> {
    let request = ChatRequest {
        model: self.model.clone(),
        messages,
        temperature,
        max_tokens,
        stream: Some(false),
        tools: tools.map(|t| t.to_vec()),
        tool_choice: tools.map(|_| "auto".to_string()),
    };

    let response = self
        .client
        .post(format!("{}/chat/completions", self.base_url))
        .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    let chat_response = self.handle_response::<ChatResponse>(response).await?;

    // Extract from first choice
    let choice = chat_response
        .choices
        .into_iter()
        .next()
        .ok_or(NearAiError::EmptyResponse)?;

    Ok(ChatResponseWithTools {
        content: choice.message.content,
        tool_calls: choice.message.tool_calls,
        finish_reason: choice.finish_reason.unwrap_or_default(),
    })
}
```

**Step 2: Add ChatResponseWithTools type**

Add to types.rs:

```rust
/// Response from chat with tools.
#[derive(Debug, Clone)]
pub struct ChatResponseWithTools {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: String,
}
```

**Step 3: Export new types from lib.rs**

Update `pub use types::*;` includes the new types, or add explicit exports.

**Step 4: Run tests**

Run: `cargo test -p near-ai-client`
Expected: All tests pass

**Step 5: Commit**

```bash
git add crates/near-ai-client/
git commit -m "feat(near-ai-client): add chat_with_tools method"
```

---

## Phase 4: Extend Conversation Store

### Task 11: Add tool_calls support to StoredMessage

**Files:**
- Modify: `crates/conversation-store/src/types.rs`

**Step 1: Update StoredMessage struct**

```rust
/// A single message in a conversation.
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

/// Stored tool call info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
```

**Step 2: Update StoredMessage::new**

```rust
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
```

**Step 3: Update OpenAiMessage**

```rust
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
```

**Step 4: Fix broken tests**

Run: `cargo test -p conversation-store`
Fix any tests that fail due to the new structure.

**Step 5: Commit**

```bash
git add crates/conversation-store/
git commit -m "feat(conversation-store): add tool_calls support to StoredMessage"
```

---

### Task 12: Add helper methods to ConversationStore

**Files:**
- Modify: `crates/conversation-store/src/lib.rs` (or wherever the store impl is)

**Step 1: Add add_assistant_with_tools method**

```rust
/// Add an assistant message with tool calls.
pub async fn add_assistant_with_tools(
    &self,
    conversation_id: &str,
    content: Option<&str>,
    tool_calls: &[near_ai_client::ToolCall],
) -> Result<(), StoreError> {
    let stored_calls: Vec<StoredToolCall> = tool_calls
        .iter()
        .map(|tc| StoredToolCall {
            id: tc.id.clone(),
            name: tc.function.name.clone(),
            arguments: tc.function.arguments.clone(),
        })
        .collect();

    let message = StoredMessage::with_tool_calls(
        "assistant",
        content.map(String::from),
        stored_calls,
    );

    // Add to conversation (reuse existing logic)
    self.add_message_internal(conversation_id, message).await
}

/// Add a tool result message.
pub async fn add_tool_result(
    &self,
    conversation_id: &str,
    tool_call_id: &str,
    content: &str,
) -> Result<(), StoreError> {
    let message = StoredMessage::tool_result(tool_call_id, content);
    self.add_message_internal(conversation_id, message).await
}
```

**Step 2: Update to_openai_messages to handle tool messages**

Ensure the conversion handles the new fields properly.

**Step 3: Run tests**

Run: `cargo test -p conversation-store`
Expected: All tests pass

**Step 4: Commit**

```bash
git add crates/conversation-store/
git commit -m "feat(conversation-store): add helper methods for tool messages"
```

---

## Phase 5: Configuration

### Task 13: Add ToolsConfig to signal-bot config

**Files:**
- Modify: `crates/signal-bot/src/config.rs`

**Step 1: Add ToolsConfig struct**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsConfig {
    /// Enable tool use system
    #[serde(default = "default_true")]
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
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub api_key: Option<String>,
    #[serde(default = "default_search_results")]
    pub max_results: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeatherConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CalculatorConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

// Defaults
fn default_true() -> bool { true }
fn default_max_tool_calls() -> usize { 5 }
fn default_search_results() -> usize { 5 }

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_tool_calls: 5,
            web_search: WebSearchConfig::default(),
            weather: WeatherConfig::default(),
            calculator: CalculatorConfig::default(),
        }
    }
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: None,
            max_results: 5,
        }
    }
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for CalculatorConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}
```

**Step 2: Add to main Config struct**

```rust
pub struct Config {
    // ... existing fields ...

    /// Tools configuration
    #[serde(default)]
    pub tools: ToolsConfig,
}
```

**Step 3: Update default_system_prompt**

```rust
fn default_system_prompt() -> String {
    r#"You are an AI assistant accessible via Signal, running in a Trusted Execution Environment (TEE) for privacy protection.

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
- Never fabricate search results or weather data"#.into()
}
```

**Step 4: Verify it compiles**

Run: `cargo check -p signal-bot`
Expected: Compiles

**Step 5: Commit**

```bash
git add crates/signal-bot/src/config.rs
git commit -m "feat(signal-bot): add ToolsConfig and improved system prompt"
```

---

## Phase 6: Chat Handler with Tool Loop

### Task 14: Update ChatHandler with tool execution loop

**Files:**
- Modify: `crates/signal-bot/src/commands/chat.rs`
- Modify: `crates/signal-bot/Cargo.toml` (add tools dependency)

**Step 1: Add tools dependency**

In `crates/signal-bot/Cargo.toml`, add:
```toml
tools = { path = "../tools" }
```

**Step 2: Update ChatHandler struct**

```rust
use tools::{ToolExecutor, ToolRegistry};

pub struct ChatHandler {
    near_ai: Arc<NearAiClient>,
    conversations: Arc<ConversationStore>,
    signal_client: Arc<SignalClient>,  // NEW - for progress messages
    tool_executor: Arc<ToolExecutor>,  // NEW
    system_prompt: String,
    max_tool_iterations: usize,        // NEW
}
```

**Step 3: Update ChatHandler::new**

```rust
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
            tool_executor: Arc::new(ToolExecutor::new(tool_registry)),
            system_prompt,
            max_tool_iterations,
        }
    }
}
```

**Step 4: Implement tool execution loop**

Replace the execute method with the tool loop implementation:

```rust
#[async_trait]
impl CommandHandler for ChatHandler {
    fn is_default(&self) -> bool {
        true
    }

    #[instrument(skip(self, message), fields(user = %message.source, is_group = %message.is_group))]
    async fn execute(&self, message: &BotMessage) -> AppResult<String> {
        let conversation_id = message.reply_target();

        // Log incoming message
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
            .add_message(conversation_id, "user", &message.text, Some(&self.build_system_prompt()))
            .await?;

        // Get tool definitions
        let tool_defs = self.tool_executor.registry().get_definitions();
        let tools: Vec<near_ai_client::ToolDefinition> = tool_defs
            .into_iter()
            .map(|d| near_ai_client::ToolDefinition {
                tool_type: d.tool_type,
                function: near_ai_client::FunctionDefinitionApi {
                    name: d.function.name,
                    description: d.function.description,
                    parameters: d.function.parameters,
                },
            })
            .collect();

        // Tool execution loop
        for iteration in 0..self.max_tool_iterations {
            let messages = self.build_messages(conversation_id).await?;

            let response = match self.near_ai.chat_with_tools(
                messages,
                Some(0.7),
                None,
                if tools.is_empty() { None } else { Some(&tools) },
            ).await {
                Ok(r) => r,
                Err(NearAiError::RateLimit) => {
                    return Ok("I'm receiving too many requests. Please wait a moment and try again.".into());
                }
                Err(NearAiError::EmptyResponse) => {
                    error!("NEAR AI returned empty response");
                    return Ok("The AI service returned an empty response. Please try rephrasing your message.".into());
                }
                Err(e) => {
                    error!("NEAR AI error: {}", e);
                    return Ok("Sorry, I encountered an error connecting to the AI service. Please try again.".into());
                }
            };

            // Check if model wants to call tools
            if let Some(tool_calls) = response.tool_calls {
                if tool_calls.is_empty() {
                    // No tool calls, return content
                    return self.finalize_response(conversation_id, response.content).await;
                }

                // Store assistant message with tool calls
                self.conversations
                    .add_assistant_with_tools(conversation_id, response.content.as_deref(), &tool_calls)
                    .await?;

                // Execute each tool
                for tool_call in &tool_calls {
                    // Send progress indicator
                    let progress_msg = format!("🔧 Using {}...", tool_call.function.name);
                    if let Err(e) = self.signal_client.send_to_conversation(conversation_id, &progress_msg).await {
                        warn!("Failed to send progress message: {}", e);
                    }

                    // Execute tool
                    let result = self.tool_executor.execute(&tools::ToolCall {
                        id: tool_call.id.clone(),
                        call_type: tool_call.call_type.clone(),
                        function: tools::FunctionCall {
                            name: tool_call.function.name.clone(),
                            arguments: tool_call.function.arguments.clone(),
                        },
                    }).await;

                    // Store tool result
                    self.conversations
                        .add_tool_result(conversation_id, &tool_call.id, &result.content)
                        .await?;
                }

                // Continue loop - NEAR AI will see tool results
                debug!("Tool iteration {} complete, continuing loop", iteration);
            } else {
                // No tool calls, return final response
                return self.finalize_response(conversation_id, response.content).await;
            }
        }

        // Max iterations reached
        warn!("Max tool iterations ({}) reached", self.max_tool_iterations);
        Ok("I've reached the limit for tool calls. Here's what I found so far - please ask a more specific question if you need more information.".into())
    }
}

impl ChatHandler {
    /// Build system prompt with current timestamp.
    fn build_system_prompt(&self) -> String {
        let now = chrono::Utc::now();
        format!(
            "{}\n\nCurrent date and time: {} UTC",
            self.system_prompt,
            now.format("%A, %B %d, %Y at %H:%M")
        )
    }

    /// Build messages for NEAR AI request.
    async fn build_messages(&self, conversation_id: &str) -> AppResult<Vec<near_ai_client::Message>> {
        let stored_messages = self
            .conversations
            .to_openai_messages(conversation_id, Some(&self.build_system_prompt()))
            .await?;

        let messages: Vec<near_ai_client::Message> = stored_messages
            .into_iter()
            .map(|m| {
                if m.role == "tool" {
                    near_ai_client::Message::tool_result(
                        m.tool_call_id.unwrap_or_default(),
                        m.content.unwrap_or_default(),
                    )
                } else if m.tool_calls.is_some() {
                    near_ai_client::Message::assistant_with_tool_calls(
                        m.content,
                        m.tool_calls.unwrap_or_default().into_iter().map(|tc| {
                            near_ai_client::ToolCall {
                                id: tc.id,
                                call_type: "function".into(),
                                function: near_ai_client::FunctionCall {
                                    name: tc.name,
                                    arguments: tc.arguments,
                                },
                            }
                        }).collect(),
                    )
                } else {
                    near_ai_client::Message {
                        role: match m.role.as_str() {
                            "system" => near_ai_client::Role::System,
                            "assistant" => near_ai_client::Role::Assistant,
                            _ => near_ai_client::Role::User,
                        },
                        content: m.content,
                        tool_calls: None,
                        tool_call_id: None,
                    }
                }
            })
            .collect();

        Ok(messages)
    }

    /// Finalize and store the response.
    async fn finalize_response(&self, conversation_id: &str, content: Option<String>) -> AppResult<String> {
        let response = content.unwrap_or_else(|| "I don't have a response.".into());

        self.conversations
            .add_message(conversation_id, "assistant", &response, None)
            .await?;

        info!(
            "Response to {}: {} chars",
            &conversation_id[..conversation_id.len().min(12)],
            response.len()
        );

        Ok(response)
    }
}
```

**Step 5: Verify it compiles**

Run: `cargo check -p signal-bot`
Expected: Compiles (may need import fixes)

**Step 6: Commit**

```bash
git add crates/signal-bot/
git commit -m "feat(signal-bot): implement tool execution loop in ChatHandler"
```

---

### Task 15: Initialize tool registry in main.rs

**Files:**
- Modify: `crates/signal-bot/src/main.rs`

**Step 1: Create tool registry initialization function**

```rust
use tools::{ToolRegistry, builtin::{CalculatorTool, WeatherTool, WebSearchTool}};

fn create_tool_registry(config: &ToolsConfig) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    if config.enabled {
        // Calculator - always available (no API key needed)
        if config.calculator.enabled {
            registry.register(Arc::new(CalculatorTool::new()));
            info!("Registered calculator tool");
        }

        // Weather - always available (no API key needed)
        if config.weather.enabled {
            registry.register(Arc::new(WeatherTool::new()));
            info!("Registered weather tool");
        }

        // Web search - requires API key
        if config.web_search.enabled {
            if let Some(api_key) = &config.web_search.api_key {
                let tool = WebSearchTool::new(api_key.clone())
                    .with_max_results(config.web_search.max_results);
                registry.register(Arc::new(tool));
                info!("Registered web search tool");
            } else {
                warn!("Web search tool enabled but no API key provided - skipping");
            }
        }
    } else {
        info!("Tools system disabled");
    }

    registry
}
```

**Step 2: Update main() to create registry and pass to ChatHandler**

Find where ChatHandler is created and update it to include the tool registry and signal client.

**Step 3: Verify it compiles**

Run: `cargo check -p signal-bot`
Expected: Compiles

**Step 4: Run full test suite**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add crates/signal-bot/
git commit -m "feat(signal-bot): initialize tool registry in main"
```

---

## Phase 7: Documentation and Final Testing

### Task 16: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md` (if exists)

**Step 1: Add tools section to CLAUDE.md**

Add documentation about:
- Available tools (calculator, weather, web search)
- Configuration environment variables
- Brave Search API key setup instructions
- How to test tools

**Step 2: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: add tool use system documentation"
```

---

### Task 17: Final integration test

**Step 1: Build release**

Run: `cargo build --release`
Expected: Builds successfully

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Manual testing checklist**

- [ ] Calculator: Send "what is 2^10?" - should use calculate tool
- [ ] Weather: Send "weather in Tokyo" - should use get_weather tool
- [ ] Web search (if API key set): Send "latest news about AI" - should use web_search tool
- [ ] Progress messages appear before tool results
- [ ] Multi-tool: Send "what's 100 Fahrenheit in Celsius and what's the weather in Miami?"

**Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete tool use system implementation"
```

---

## Summary

| Phase | Tasks | Key Files |
|-------|-------|-----------|
| 1. Core Types | 1-5 | `crates/tools/src/*.rs` |
| 2. Built-in Tools | 6-8 | `crates/tools/src/builtin/*.rs` |
| 3. NEAR AI Client | 9-10 | `crates/near-ai-client/src/types.rs`, `client.rs` |
| 4. Conversation Store | 11-12 | `crates/conversation-store/src/types.rs` |
| 5. Configuration | 13 | `crates/signal-bot/src/config.rs` |
| 6. Chat Handler | 14-15 | `crates/signal-bot/src/commands/chat.rs`, `main.rs` |
| 7. Documentation | 16-17 | `CLAUDE.md`, final testing |

**Total: 17 tasks, ~85 steps**
