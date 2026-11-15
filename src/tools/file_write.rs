use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use similar::TextDiff;
use uuid::Uuid;

use crate::checkpoints::sqlite::SqliteCheckpointStore;
use crate::checkpoints::store::CheckpointStore;
use crate::checkpoints::types::AuditLogEntry;
use crate::domain::ExecutionScope;
use crate::events::sink::EventSink;
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::tools::path_guard::{resolve_writable_file, MAX_TEXT_BYTES};
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};
use crate::verification::policy::{ExecutionPolicyEngine, PolicyDecision};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteDiffPreview {
    pub path: String,
    pub created: bool,
    pub previous_content: String,
    pub unified_diff: String,
}

pub fn compute_unified_diff(rel_path: &str, previous: &str, updated: &str) -> String {
    TextDiff::from_lines(previous, updated)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", rel_path), &format!("b/{}", rel_path))
        .to_string()
}

pub fn preview_write_diff(
    workspace_root: &std::path::Path,
    rel_path: &str,
    new_content: &str,
) -> Result<WriteDiffPreview, ToolError> {
    if new_content.len() > MAX_TEXT_BYTES {
        return Err(ToolError::FileSizeExceeded {
            size: new_content.len(),
            limit: MAX_TEXT_BYTES,
        });
    }

    let (_root, target, clean_rel) = resolve_writable_file(workspace_root, rel_path)?;
    let (previous_content, created) = if target.exists() {
        let bytes = fs::read(&target).map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed reading existing file '{}': {}",
                clean_rel, e
            ))
        })?;
        let text = String::from_utf8(bytes).map_err(|_| {
            ToolError::InvalidArguments(format!(
                "Existing file '{}' is binary and cannot be diff-edited",
                clean_rel
            ))
        })?;
        (text, false)
    } else {
        (String::new(), true)
    };

    let unified_diff = compute_unified_diff(&clean_rel, &previous_content, new_content);
    Ok(WriteDiffPreview {
        path: clean_rel,
        created,
        previous_content,
        unified_diff,
    })
}

#[derive(Clone)]
pub struct WriteFileTool {
    workspace_root: PathBuf,
    event_sink: Option<Arc<dyn EventSink>>,
    audit_store: Option<Arc<SqliteCheckpointStore>>,
    policy_engine: Option<Arc<ExecutionPolicyEngine>>,
}

impl WriteFileTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            event_sink: None,
            audit_store: None,
            policy_engine: None,
        }
    }

    pub fn with_event_sink(mut self, event_sink: Arc<dyn EventSink>) -> Self {
        self.event_sink = Some(event_sink);
        self
    }

    pub fn with_audit_store(mut self, audit_store: Arc<SqliteCheckpointStore>) -> Self {
        self.audit_store = Some(audit_store);
        self
    }

    pub fn with_policy_engine(mut self, policy_engine: Arc<ExecutionPolicyEngine>) -> Self {
        self.policy_engine = Some(policy_engine);
        self
    }
}

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
    #[serde(default = "default_true")]
    create_dirs: bool,
}

fn default_true() -> bool {
    true
}

impl Tool for WriteFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_string(),
            description:
                "Atomically compute a unified git diff and write UTF-8 content to a workspace file."
                    .to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path within workspace root to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Complete UTF-8 file content to write atomically"
                    },
                    "create_dirs": {
                        "type": "boolean",
                        "description": "Whether to create missing parent directories within workspace"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn check_policy(&self, arguments: &serde_json::Value) -> Option<PolicyDecision> {
        let p = arguments.get("path")?.as_str()?;
        let engine = self.policy_engine.clone().unwrap_or_else(|| {
            if let Some(ref store) = self.audit_store {
                if let Ok(rec) = store.get_execution_policy() {
                    return Arc::new(ExecutionPolicyEngine::from_record(&rec));
                }
            }
            Arc::new(ExecutionPolicyEngine::default())
        });
        Some(engine.evaluate_path(p))
    }

    fn execute<'a>(
        &'a self,
        scope: &'a ExecutionScope,
        arguments: &'a serde_json::Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let args: WriteFileArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid write_file arguments: {}", e))
            })?;

            if args.content.len() > MAX_TEXT_BYTES {
                return Err(ToolError::FileSizeExceeded {
                    size: args.content.len(),
                    limit: MAX_TEXT_BYTES,
                });
            }

            let engine = self.policy_engine.clone().unwrap_or_else(|| {
                if let Some(ref store) = self.audit_store {
                    if let Ok(rec) = store.get_execution_policy() {
                        return Arc::new(ExecutionPolicyEngine::from_record(&rec));
                    }
                }
                Arc::new(ExecutionPolicyEngine::default())
            });

            if engine.evaluate_path(&args.path) == PolicyDecision::Deny {
                if let Some(ref store) = self.audit_store {
                    let _ = store
                        .record_audit_log(&AuditLogEntry {
                            id: Uuid::new_v4(),
                            scope: *scope,
                            actor_id: None,
                            action: "sandbox.write_blocked".to_string(),
                            target_id: None,
                            details: serde_json::json!({
                                "path": args.path,
                                "error": "Access denied by execution policy",
                            }),
                            created_at: chrono::Utc::now(),
                        })
                        .await;
                }
                return Err(ToolError::AccessDenied(format!(
                    "Writing to path '{}' is blocked by execution policy",
                    args.path
                )));
            }

            let resolve_res = resolve_writable_file(&self.workspace_root, &args.path);
            let (_real_root, target, clean_rel) = match resolve_res {
                Ok(tuple) => tuple,
                Err(err) => {
                    if let Some(ref store) = self.audit_store {
                        let _ = store
                            .record_audit_log(&AuditLogEntry {
                                id: Uuid::new_v4(),
                                scope: *scope,
                                actor_id: None,
                                action: "sandbox.write_blocked".to_string(),
                                target_id: None,
                                details: serde_json::json!({
                                    "path": args.path,
                                    "error": err.to_string(),
                                }),
                                created_at: chrono::Utc::now(),
                            })
                            .await;
                    }
                    return Err(err);
                }
            };

            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    if !args.create_dirs {
                        return Err(ToolError::ExecutionFailed(format!(
                            "Parent directory for '{}' does not exist",
                            clean_rel
                        )));
                    }
                    fs::create_dir_all(parent).map_err(|e| {
                        ToolError::ExecutionFailed(format!(
                            "Failed creating parent directory for '{}': {}",
                            clean_rel, e
                        ))
                    })?;
                }
            }

            let (_, target_reverified, _) =
                resolve_writable_file(&self.workspace_root, &args.path)?;

            let preview = preview_write_diff(&self.workspace_root, &args.path, &args.content)?;

            let parent_dir = target_reverified.parent().ok_or_else(|| {
                ToolError::ExecutionFailed("Target file missing parent directory".to_string())
            })?;
            let tmp_name = format!(".alba-write-{}.tmp", Uuid::new_v4());
            let tmp_path = parent_dir.join(tmp_name);

            let mut opts = OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            opts.mode(0o644);

            let write_res = (|| -> Result<(), std::io::Error> {
                let mut f = opts.open(&tmp_path)?;
                f.write_all(args.content.as_bytes())?;
                f.sync_all()?;
                fs::rename(&tmp_path, &target_reverified)?;
                Ok(())
            })();

            if let Err(e) = write_res {
                let _ = fs::remove_file(&tmp_path);
                return Err(ToolError::ExecutionFailed(format!(
                    "Atomic write failed for '{}': {}",
                    clean_rel, e
                )));
            }

            let mut hasher = Sha256::new();
            hasher.update(args.content.as_bytes());
            let sha256_hex = format!("{:x}", hasher.finalize());

            if let Some(ref sink) = self.event_sink {
                let _ = sink.emit(RuntimeEvent::new(
                    *scope,
                    RuntimeEventType::SandboxFileDiff,
                    serde_json::json!({
                        "path": preview.path,
                        "created": preview.created,
                        "bytes_written": args.content.len(),
                        "original": preview.previous_content,
                        "modified": args.content,
                        "diff": preview.unified_diff,
                        "sha256": sha256_hex,
                    }),
                ));
            }

            let out = serde_json::json!({
                "path": preview.path,
                "created": preview.created,
                "bytes_written": args.content.len(),
                "sha256": sha256_hex,
                "diff": preview.unified_diff,
            });
            Ok(out.to_string())
        })
    }
}
