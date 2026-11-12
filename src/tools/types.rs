use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::ExecutionScope;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),
    #[error("Invalid tool arguments: {0}")]
    InvalidArguments(String),
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("Path traversal attempt rejected: {0}")]
    PathTraversal(String),
    #[error("File size {size} bytes exceeds limit of {limit} bytes")]
    FileSizeExceeded { size: usize, limit: usize },
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}

pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn execute<'a>(
        &'a self,
        scope: &'a ExecutionScope,
        arguments: &'a serde_json::Value,
    ) -> BoxFuture<'a, Result<String, ToolError>>;
}

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.definition().name;
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<_> = self.tools.values().map(|t| t.definition()).collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    pub async fn execute(
        &self,
        scope: &ExecutionScope,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(scope, arguments).await
    }
}
