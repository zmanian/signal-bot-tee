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
