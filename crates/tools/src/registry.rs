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
