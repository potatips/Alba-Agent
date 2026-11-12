use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use crate::domain::ExecutionScope;
use crate::events::sink::EventSink;
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::sandbox::{SandboxBackendKind, SandboxPolicy, SandboxRunner, SandboxStreamChunk};
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};

#[derive(Clone)]
pub struct RunCommandTool {
    workspace_root: PathBuf,
    event_sink: Option<Arc<dyn EventSink>>,
    backend: SandboxBackendKind,
}

impl RunCommandTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            event_sink: None,
            backend: SandboxBackendKind::Auto,
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

            serde_json::to_string(&output).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed serializing sandbox output: {}", e))
            })
        })
    }
}
