use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use uuid::Uuid;

use crate::checkpoints::sqlite::SqliteCheckpointStore;
use crate::checkpoints::store::CheckpointStore;
use crate::checkpoints::types::AuditLogEntry;
use crate::domain::ExecutionScope;
use crate::events::sink::EventSink;
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::sandbox::{SandboxBackendKind, SandboxPolicy, SandboxRunner, SandboxStreamChunk};
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};
use crate::verification::policy::{ExecutionPolicyEngine, PolicyDecision};

#[derive(Clone)]
pub struct RunCommandTool {
    workspace_root: PathBuf,
    event_sink: Option<Arc<dyn EventSink>>,
    backend: SandboxBackendKind,
    audit_store: Option<Arc<SqliteCheckpointStore>>,
    policy_engine: Option<Arc<ExecutionPolicyEngine>>,
}

impl RunCommandTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            event_sink: None,
            backend: SandboxBackendKind::Auto,
            audit_store: None,
            policy_engine: None,
        }
    }

    pub fn with_event_sink(mut self, event_sink: Arc<dyn EventSink>) -> Self {
        self.event_sink = Some(event_sink);
        self
    }

    pub fn with_backend(mut self, backend: SandboxBackendKind) -> Self {
        self.backend = backend;
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
struct RunCommandArgs {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    allow_network: bool,
    timeout_secs: Option<u64>,
}

impl Tool for RunCommandTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "run_command".to_string(),
            description: "Execute a command inside the OS-native workspace sandbox, streaming stdout/stderr and returning exit status.".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "program": {
                        "type": "string",
                        "description": "Executable name or binary to run"
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Argument list passed without host shell expansion"
                    },
                    "allow_network": {
                        "type": "boolean",
                        "description": "Allow outbound network access (false by default)"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Maximum execution seconds (defaults to 30, capped at 300)"
                    }
                },
                "required": ["program"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        scope: &'a ExecutionScope,
        arguments: &'a serde_json::Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let args: RunCommandArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid run_command arguments: {}", e))
            })?;

            let timeout_secs = args.timeout_secs.unwrap_or(30).clamp(1, 300);

            let full_cmd = if args.args.is_empty() {
                args.program.clone()
            } else {
                format!("{} {}", args.program, args.args.join(" "))
            };

            let engine = self.policy_engine.clone().unwrap_or_else(|| {
                if let Some(ref store) = self.audit_store {
                    if let Ok(rec) = store.get_execution_policy() {
                        return Arc::new(ExecutionPolicyEngine::from_record(&rec));
                    }
                }
                Arc::new(ExecutionPolicyEngine::default())
            });

            let decision = engine.evaluate_command(&full_cmd);
            if decision == PolicyDecision::Deny {
                if let Some(ref store) = self.audit_store {
                    let _ = store
                        .record_audit_log(&AuditLogEntry {
                            id: Uuid::new_v4(),
                            scope: *scope,
                            actor_id: None,
                            action: "sandbox.command_blocked".to_string(),
                            target_id: None,
                            details: serde_json::json!({
                                "program": args.program,
                                "args": args.args,
                                "reason": "Command execution denied by execution policy",
                            }),
                            created_at: chrono::Utc::now(),
                        })
                        .await;
                }
                return Err(ToolError::AccessDenied(
                    "Command execution is blocked by execution policy".to_string(),
                ));
            }

            let is_network_call = args.program == "curl"
                || args.program == "wget"
                || args
                    .args
                    .iter()
                    .any(|a| a.starts_with("http://") || a.starts_with("https://"));

            if is_network_call {
                for arg in &args.args {
                    if (arg.starts_with("http://") || arg.starts_with("https://"))
                        && engine.evaluate_network(arg) == PolicyDecision::Deny
                    {
                        if let Some(ref store) = self.audit_store {
                            let _ = store
                                .record_audit_log(&AuditLogEntry {
                                    id: Uuid::new_v4(),
                                    scope: *scope,
                                    actor_id: None,
                                    action: "sandbox.network_blocked".to_string(),
                                    target_id: None,
                                    details: serde_json::json!({
                                        "program": args.program,
                                        "args": args.args,
                                        "target": arg,
                                        "reason": "Target domain denied by execution policy",
                                    }),
                                    created_at: chrono::Utc::now(),
                                })
                                .await;
                        }
                        return Err(ToolError::AccessDenied(format!(
                            "Network access to '{}' is blocked by execution policy",
                            arg
                        )));
                    }
                }

                if !args.allow_network {
                    if let Some(ref store) = self.audit_store {
                        let _ = store
                            .record_audit_log(&AuditLogEntry {
                                id: Uuid::new_v4(),
                                scope: *scope,
                                actor_id: None,
                                action: "sandbox.network_blocked".to_string(),
                                target_id: None,
                                details: serde_json::json!({
                                    "program": args.program,
                                    "args": args.args,
                                    "reason": "Network access blocked by default sandbox policy",
                                }),
                                created_at: chrono::Utc::now(),
                            })
                            .await;
                    }
                    return Err(ToolError::AccessDenied(
                        "Outbound network access is blocked by sandbox policy".to_string(),
                    ));
                }
            }

            let policy = SandboxPolicy::new(&self.workspace_root)
                .with_network(args.allow_network)
                .with_timeout(Duration::from_secs(timeout_secs))
                .with_backend(self.backend);

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SandboxStreamChunk>();
            let sink_opt = self.event_sink.clone();
            let scope_copy = *scope;
            let program_label = args.program.clone();

            let forwarder = tokio::spawn(async move {
                while let Some(chunk) = rx.recv().await {
                    if let Some(ref sink) = sink_opt {
                        let _ = sink.emit(RuntimeEvent::new(
                            scope_copy,
                            RuntimeEventType::SandboxTerminalOutput,
                            serde_json::json!({
                                "program": program_label,
                                "stream": chunk.stream.as_str(),
                                "chunk": chunk.chunk,
                            }),
                        ));
                    }
                }
            });

            let output = SandboxRunner::execute(&policy, &args.program, &args.args, Some(tx))
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            let _ = forwarder.await;

            if output.exit_code != 0 {
                if let Some(ref store) = self.audit_store {
                    let _ = store
                        .record_audit_log(&AuditLogEntry {
                            id: Uuid::new_v4(),
                            scope: *scope,
                            actor_id: None,
                            action: "sandbox.execution_denied".to_string(),
                            target_id: None,
                            details: serde_json::json!({
                                "program": args.program,
                                "args": args.args,
                                "exit_code": output.exit_code,
                                "stderr": output.stderr,
                            }),
                            created_at: chrono::Utc::now(),
                        })
                        .await;
                }
            }

            serde_json::to_string(&output).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed serializing sandbox output: {}", e))
            })
        })
    }
}
