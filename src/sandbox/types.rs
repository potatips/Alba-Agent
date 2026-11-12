use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxBackendKind {
    Auto,
    Bwrap,
    Unshare,
    Seatbelt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStreamKind {
    Stdout,
    Stderr,
}

impl SandboxStreamKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxStreamChunk {
    pub stream: SandboxStreamKind,
    pub chunk: String,
}

#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub workspace_root: PathBuf,
    pub allow_network: bool,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub env: BTreeMap<String, String>,
    pub backend: SandboxBackendKind,
}

impl SandboxPolicy {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            allow_network: false,
            timeout: Duration::from_secs(30),
            max_output_bytes: 256 * 1024,
            env: BTreeMap::new(),
            backend: SandboxBackendKind::Auto,
        }
    }

    pub fn with_network(mut self, allow_network: bool) -> Self {
        self.allow_network = allow_network;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }

    pub fn with_backend(mut self, backend: SandboxBackendKind) -> Self {
        self.backend = backend;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub timed_out: bool,
    pub backend: String,
    pub duration_ms: u64,
}

#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("Invalid workspace root: {0}")]
    InvalidWorkspace(String),
    #[error("Sandbox backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("Sandbox backend unsupported on this platform: {0}")]
    UnsupportedPlatform(String),
    #[error("Invalid sandbox command: {0}")]
    InvalidCommand(String),
    #[error("Sandbox execution failed: {0}")]
    ExecutionFailed(String),
}

async fn read_limited_stream<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    kind: SandboxStreamKind,
    limit: usize,
    tx: Option<UnboundedSender<SandboxStreamChunk>>,
) -> (String, bool) {
    let mut buf_reader = BufReader::new(reader);
    let mut out = String::new();
    let mut truncated = false;
    let mut line = String::new();

    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                if let Some(ref sender) = tx {
                    let _ = sender.send(SandboxStreamChunk {
                        stream: kind,
                        chunk: line.clone(),
                    });
                }
                if out.len() + line.len() <= limit {
                    out.push_str(&line);
                } else {
                    if out.len() < limit {
                        let remaining = limit - out.len();
                        let mut end = remaining;
                        while !line.is_char_boundary(end) && end > 0 {
                            end -= 1;
                        }
                        out.push_str(&line[..end]);
                    }
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }

    (out, truncated)
}

pub(crate) async fn collect_child_output(
    mut child: Child,
    backend_name: &str,
    timeout: Duration,
    max_output_bytes: usize,
    stream_tx: Option<UnboundedSender<SandboxStreamChunk>>,
) -> Result<SandboxOutput, SandboxError> {
    let start = std::time::Instant::now();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SandboxError::ExecutionFailed("Failed to capture child stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| SandboxError::ExecutionFailed("Failed to capture child stderr".into()))?;

    let tx_out = stream_tx.clone();
    let tx_err = stream_tx;
    let out_task = tokio::spawn(read_limited_stream(
        stdout,
        SandboxStreamKind::Stdout,
        max_output_bytes,
        tx_out,
    ));
    let err_task = tokio::spawn(read_limited_stream(
        stderr,
        SandboxStreamKind::Stderr,
        max_output_bytes,
        tx_err,
    ));

    let mut timed_out = false;
    let exit_code = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => status.code().unwrap_or(-1),
        Ok(Err(e)) => {
            return Err(SandboxError::ExecutionFailed(format!(
                "Failed waiting on sandboxed process: {}",
                e
            )));
        }
        Err(_) => {
            timed_out = true;
            let _ = child.kill().await;
            let _ = child.wait().await;
            -1
        }
    };

    let (stdout_str, out_trunc) = out_task.await.unwrap_or_default();
    let (stderr_str, err_trunc) = err_task.await.unwrap_or_default();

    Ok(SandboxOutput {
        exit_code,
        stdout: stdout_str,
        stderr: stderr_str,
        truncated: out_trunc || err_trunc,
        timed_out,
        backend: backend_name.to_string(),
        duration_ms: start.elapsed().as_millis() as u64,
    })
}
