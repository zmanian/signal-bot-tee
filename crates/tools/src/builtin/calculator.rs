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
