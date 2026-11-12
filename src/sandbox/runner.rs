use tokio::sync::mpsc::UnboundedSender;

#[cfg(target_os = "linux")]
use crate::sandbox::linux::execute_linux;
#[cfg(target_os = "macos")]
use crate::sandbox::macos::execute_macos;
#[cfg(not(target_os = "linux"))]
use crate::sandbox::types::SandboxBackendKind;
use crate::sandbox::types::{SandboxError, SandboxOutput, SandboxPolicy, SandboxStreamChunk};

pub struct SandboxRunner;

impl SandboxRunner {
    pub async fn execute(
        policy: &SandboxPolicy,
        program: &str,
        args: &[String],
        stream_tx: Option<UnboundedSender<SandboxStreamChunk>>,
    ) -> Result<SandboxOutput, SandboxError> {
        let trimmed_prog = program.trim();
        if trimmed_prog.is_empty() || trimmed_prog.contains('\0') {
            return Err(SandboxError::InvalidCommand(
                "Program name cannot be empty or contain null bytes".to_string(),
            ));
        }

        let canonical_ws = std::fs::canonicalize(&policy.workspace_root).map_err(|e| {
            SandboxError::InvalidWorkspace(format!(
                "Cannot resolve workspace root '{}': {}",
                policy.workspace_root.display(),
                e
            ))
        })?;

        if !canonical_ws.is_dir() {
            return Err(SandboxError::InvalidWorkspace(format!(
                "Workspace root '{}' is not a directory",
                canonical_ws.display()
            )));
        }

        #[cfg(target_os = "linux")]
        {
            execute_linux(&canonical_ws, policy, trimmed_prog, args, stream_tx).await
        }

        #[cfg(target_os = "macos")]
        {
            if !matches!(
                policy.backend,
                SandboxBackendKind::Auto | SandboxBackendKind::Seatbelt
            ) {
                return Err(SandboxError::UnsupportedPlatform(
                    "Linux sandbox backends are not available on macOS".to_string(),
                ));
            }
            execute_macos(&canonical_ws, policy, trimmed_prog, args, stream_tx).await
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (&canonical_ws, args, stream_tx, SandboxBackendKind::Auto);
            Err(SandboxError::UnsupportedPlatform(
                "OS-native sandbox is only supported on Linux and macOS".to_string(),
            ))
        }
    }
}
