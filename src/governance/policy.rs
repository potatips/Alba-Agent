use std::path::PathBuf;

use crate::domain::{ActionIntent, ActionIntentKind};
use crate::tools::file_write::preview_write_diff;

pub trait ApprovalPolicy: Send + Sync {
    fn check_approval(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Option<ActionIntent>;
}

#[derive(Debug, Clone, Default)]
pub struct DefaultApprovalPolicy {
    require_all: bool,
    required_tools: Vec<String>,
    sensitive_path_prefixes: Vec<String>,
    workspace_root: Option<PathBuf>,
}

impl DefaultApprovalPolicy {
    pub fn new() -> Self {
        Self {
            require_all: false,
            required_tools: Vec::new(),
            sensitive_path_prefixes: Vec::new(),
            workspace_root: None,
        }
    }

    pub fn with_workspace_root(mut self, workspace_root: impl Into<PathBuf>) -> Self {
        self.workspace_root = Some(workspace_root.into());
        self
    }

    pub fn with_require_all(mut self, require_all: bool) -> Self {
        self.require_all = require_all;
        self
    }

    pub fn with_required_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.required_tools.push(tool_name.into());
        self
    }

    pub fn with_sensitive_path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.sensitive_path_prefixes.push(prefix.into());
        self
    }
}

impl ApprovalPolicy for DefaultApprovalPolicy {
    fn check_approval(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Option<ActionIntent> {
        if self.require_all {
            return Some(ActionIntent {
                kind: ActionIntentKind::ExternalAction,
                description: format!("Execute tool '{}' with arguments {}", tool_name, arguments),
            });
        }

        if self.required_tools.iter().any(|t| t == tool_name) {
            return Some(ActionIntent {
                kind: ActionIntentKind::ExternalAction,
                description: format!("Execute tool '{}' with arguments {}", tool_name, arguments),
            });
        }

        if tool_name == "write_file" {
            let path = arguments
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            let content = arguments
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(ref ws) = self.workspace_root {
                if let Ok(preview) = preview_write_diff(ws, path, content) {
                    return Some(ActionIntent {
                        kind: ActionIntentKind::FileAccess,
                        description: format!(
                            "Write file '{}'\n{}",
                            preview.path, preview.unified_diff
                        ),
                    });
                }
            }
            return Some(ActionIntent {
                kind: ActionIntentKind::FileAccess,
                description: format!("Write file '{}' ({} bytes)", path, content.len()),
            });
        }

        if tool_name == "run_command" {
            let program = arguments
                .get("program")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            let args: Vec<&str> = arguments
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            return Some(ActionIntent {
                kind: ActionIntentKind::DestructiveCommand,
                description: if args.is_empty() {
                    format!("Execute sandboxed command: {}", program)
                } else {
                    format!("Execute sandboxed command: {} {}", program, args.join(" "))
                },
            });
        }

        if tool_name == "read_file" {
            if let Some(path_val) = arguments.get("path").and_then(|v| v.as_str()) {
                for prefix in &self.sensitive_path_prefixes {
                    if path_val.starts_with(prefix) {
                        return Some(ActionIntent {
                            kind: ActionIntentKind::FileAccess,
                            description: format!("Read sensitive file '{}'", path_val),
                        });
                    }
                }
            }
        }

        None
    }
}
