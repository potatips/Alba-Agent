use std::path::{Component, Path, PathBuf};

use crate::domain::ExecutionScope;
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};

pub const DEFAULT_MAX_FILE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ReadFileTool {
    workspace_root: PathBuf,
    max_bytes: usize,
}

impl ReadFileTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            max_bytes: DEFAULT_MAX_FILE_BYTES,
        }
    }

    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    fn is_sensitive(path_str: &str) -> bool {
        let normalized = path_str.to_lowercase();
        let sensitive_names = [
            ".git",
            ".ssh",
            ".aws",
            ".gnupg",
            ".kube",
            ".netrc",
            ".pypirc",
            ".npmrc",
            "id_rsa",
            "id_ed25519",
            "keychains",
            "shadow",
        ];

        for part in Path::new(&normalized).components() {
            let s = part.as_os_str().to_string_lossy();
            if sensitive_names.iter().any(|&name| s == name) {
                return true;
            }
            if s == ".env" {
                return true;
            }
            if s.starts_with(".env.")
                && !s.ends_with(".example")
                && !s.ends_with(".sample")
                && !s.ends_with(".template")
            {
                return true;
            }
        }
        false
    }

    pub fn validate_and_resolve(&self, relative_path: &str) -> Result<PathBuf, ToolError> {
        if relative_path.is_empty() {
            return Err(ToolError::InvalidArguments(
                "Path cannot be empty".to_string(),
            ));
        }
        if relative_path.contains('\0') {
            return Err(ToolError::InvalidArguments(
                "Path cannot contain null bytes".to_string(),
            ));
        }
        if relative_path.contains('\\') {
            return Err(ToolError::InvalidArguments(
                "Backslashes are not allowed in paths".to_string(),
            ));
        }
        if relative_path.starts_with('/') {
            return Err(ToolError::PathTraversal(
                "Absolute paths are rejected".to_string(),
            ));
        }

        let input_path = Path::new(relative_path);
        for comp in input_path.components() {
            match comp {
                Component::ParentDir => {
                    return Err(ToolError::PathTraversal(
                        "Parent directory traversal is rejected".to_string(),
                    ));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(ToolError::PathTraversal(
                        "Root or prefix paths are rejected".to_string(),
                    ));
                }
                Component::CurDir | Component::Normal(_) => {}
            }
        }

        if Self::is_sensitive(relative_path) {
            return Err(ToolError::AccessDenied(format!(
                "Access to sensitive file '{}' is blocked",
                relative_path
            )));
        }

        let candidate = self.workspace_root.join(input_path);
        if !candidate.exists() {
            return Err(ToolError::NotFound(format!(
                "File '{}' does not exist",
                relative_path
            )));
        }

        let canonical_root = self
            .workspace_root
            .canonicalize()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let canonical_target = candidate
            .canonicalize()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if !canonical_target.starts_with(&canonical_root) {
            return Err(ToolError::PathTraversal(
                "Resolved path escapes authorized workspace root".to_string(),
            ));
        }

        if canonical_target.is_dir() {
            return Err(ToolError::InvalidArguments(
                "Specified path is a directory, not a file".to_string(),
            ));
        }

        Ok(canonical_target)
    }
}

impl Tool for ReadFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file within the authorized workspace.".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path of the file to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _scope: &'a ExecutionScope,
        arguments: &'a serde_json::Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let path_val = arguments.get("path").ok_or_else(|| {
                ToolError::InvalidArguments("Missing required parameter 'path'".to_string())
            })?;

            let relative_path = path_val.as_str().ok_or_else(|| {
                ToolError::InvalidArguments("Parameter 'path' must be a string".to_string())
            })?;

            let resolved = self.validate_and_resolve(relative_path)?;

            let metadata = tokio::fs::metadata(&resolved)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            let file_size = metadata.len() as usize;
            if file_size > self.max_bytes {
                return Err(ToolError::FileSizeExceeded {
                    size: file_size,
                    limit: self.max_bytes,
                });
            }

            let bytes = tokio::fs::read(&resolved)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            if bytes.len() > self.max_bytes {
                return Err(ToolError::FileSizeExceeded {
                    size: bytes.len(),
                    limit: self.max_bytes,
                });
            }

            String::from_utf8(bytes)
                .map_err(|e| ToolError::ExecutionFailed(format!("File is not valid UTF-8: {}", e)))
        })
    }
}
