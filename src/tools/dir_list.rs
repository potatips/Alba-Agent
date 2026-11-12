use std::path::PathBuf;

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use crate::domain::ExecutionScope;
use crate::tools::path_guard::{is_sensitive_component, resolve_existing_directory};
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};

pub const DEFAULT_LIST_DEPTH: usize = 4;
pub const MAX_LIST_DEPTH: usize = 10;
pub const DEFAULT_MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntryItem {
    pub path: String,
    pub kind: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirScanResult {
    pub root: String,
    pub entries: Vec<DirEntryItem>,
    pub truncated: bool,
}

pub fn scan_directory(
    workspace_root: &std::path::Path,
    rel_dir: &str,
    max_depth: usize,
    max_entries: usize,
    include_hidden: bool,
) -> Result<DirScanResult, ToolError> {
    let clamped_depth = max_depth.clamp(1, MAX_LIST_DEPTH);
    let clamped_entries = max_entries.clamp(1, 2000);
    let (real_root, real_target, clean_rel) = resolve_existing_directory(workspace_root, rel_dir)?;

    let mut builder = WalkBuilder::new(&real_target);
    builder
        .hidden(!include_hidden)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .follow_links(false)
        .max_depth(Some(clamped_depth))
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !is_sensitive_component(&name)
        });

    let mut items = Vec::new();
    let mut truncated = false;

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path == real_target {
            continue;
        }
        if !path.starts_with(&real_root) {
            continue;
        }
        if entry.file_type().is_some_and(|ft| ft.is_symlink()) {
            continue;
        }

        let rel_path = match path.strip_prefix(&real_root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        if rel_path.split('/').any(is_sensitive_component) {
            continue;
        }

        if items.len() >= clamped_entries {
            truncated = true;
            break;
        }

        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
        let size = if is_dir {
            0
        } else {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        };

        items.push(DirEntryItem {
            path: rel_path,
            kind: if is_dir {
                "dir".to_string()
            } else {
                "file".to_string()
            },
            size,
        });
    }

    items.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(DirScanResult {
        root: clean_rel,
        entries: items,
        truncated,
    })
}

pub struct ListDirTool {
    workspace_root: PathBuf,
}

impl ListDirTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[derive(Deserialize)]
struct ListDirArgs {
    path: Option<String>,
    max_depth: Option<usize>,
    max_entries: Option<usize>,
    #[serde(default)]
    include_hidden: bool,
}

impl Tool for ListDirTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_dir".to_string(),
            description: "Recursively list directory entries within workspace root respecting ignore rules and depth limits.".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative directory path inside workspace root (defaults to '.')"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum traversal depth (1..10)"
                    },
                    "max_entries": {
                        "type": "integer",
                        "description": "Maximum returned entries (1..2000)"
                    },
                    "include_hidden": {
                        "type": "boolean",
                        "description": "Include dotfiles except sensitive files and .git"
                    }
                }
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _scope: &'a ExecutionScope,
        arguments: &'a serde_json::Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let args: ListDirArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid list_dir arguments: {}", e))
            })?;

            let target = args.path.as_deref().unwrap_or(".");
            let depth = args.max_depth.unwrap_or(DEFAULT_LIST_DEPTH);
            let cap = args.max_entries.unwrap_or(DEFAULT_MAX_ENTRIES);

            let scan = scan_directory(
                &self.workspace_root,
                target,
                depth,
                cap,
                args.include_hidden,
            )?;

            serde_json::to_string(&scan).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed serializing directory scan: {}", e))
            })
        })
    }
}
