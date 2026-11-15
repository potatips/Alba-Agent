use std::path::Path;

use tokio::sync::mpsc::UnboundedSender;

use crate::sandbox::types::{SandboxError, SandboxOutput, SandboxPolicy, SandboxStreamChunk};

fn escape_sbpl(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

pub fn generate_seatbelt_profile(canonical_ws: &Path, policy: &SandboxPolicy) -> String {
    let ws = escape_sbpl(canonical_ws);
    let network_rule = if policy.allow_network {
        "(allow network-outbound)\n(allow network-inbound)"
    } else {
        "(deny network*)"
    };

    format!(
        "(version 1)\n\
         (deny default)\n\
         (allow process-exec)\n\
         (allow process-fork)\n\
         (allow sysctl-read)\n\
         (allow mach-lookup)\n\
         (allow signal (target self))\n\
         (allow file-read-metadata)\n\
         (allow file-ioctl)\n\
         (allow ipc-posix-shm*)\n\
         (allow file-read*\n\
           (literal \"/\")\n\
           (subpath \"/usr\")\n\
           (subpath \"/bin\")\n\
           (subpath \"/sbin\")\n\
           (subpath \"/System\")\n\
           (subpath \"/Library\")\n\
           (subpath \"/private/var/db\")\n\
           (subpath \"/private/etc\")\n\
           (subpath \"/etc\")\n\
           (subpath \"/private/var\")\n\
           (subpath \"/var\")\n\
           (subpath \"/dev\")\n\
           (subpath \"{ws}\"))\n\
         (deny mach-lookup (global-name \"com.apple.SecurityServer\") (global-name \"com.apple.securityd.xpc\"))\n\
         (allow file-write*\n\
           (subpath \"{ws}\")\n\
           (literal \"/dev/null\")\n\
           (literal \"/dev/zero\")\n\
           (literal \"/dev/tty\")\n\
           (literal \"/dev/dtracehelper\"))\n\
         (deny file-read-data (regex #\"/\\.ssh(/|$)\"))\n\
         (deny file-read-data (regex #\"/\\.aws(/|$)\"))\n\
         (deny file-read-data (regex #\"/\\.gnupg(/|$)\"))\n\
         (deny file-read-data (regex #\"/\\.kube(/|$)\"))\n\
         (deny file-read-data (regex #\"/Library/Keychains(/|$)\"))\n\
         (deny file-read-data (regex #\"/\\.env(\\.[^/]*)?$\"))\n\
         (allow file-read-data (regex #\"/\\.env\\.(example|sample|template)$\"))\n\
         {network_rule}\n",
        ws = ws,
        network_rule = network_rule,
    )
}

#[cfg(target_os = "macos")]
pub async fn execute_macos(
    canonical_ws: &Path,
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
    stream_tx: Option<UnboundedSender<SandboxStreamChunk>>,
) -> Result<SandboxOutput, SandboxError> {
    use std::process::Stdio;
    use tokio::process::Command;

    use crate::sandbox::types::collect_child_output;

    let profile = generate_seatbelt_profile(canonical_ws, policy);
    let mut cmd = Command::new("/usr/bin/sandbox-exec");
    cmd.arg("-p")
        .arg(&profile)
        .arg("--")
        .arg(program)
        .args(args)
        .current_dir(canonical_ws)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", canonical_ws)
        .kill_on_drop(true);

    for (k, v) in &policy.env {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| {
        SandboxError::ExecutionFailed(format!("Failed spawning sandbox-exec process: {}", e))
    })?;

    collect_child_output(
        child,
        "macos:seatbelt",
        policy.timeout,
        policy.max_output_bytes,
        stream_tx,
    )
    .await
}

#[cfg(not(target_os = "macos"))]
pub async fn execute_macos(
    _canonical_ws: &Path,
    _policy: &SandboxPolicy,
    _program: &str,
    _args: &[String],
    _stream_tx: Option<UnboundedSender<SandboxStreamChunk>>,
) -> Result<SandboxOutput, SandboxError> {
    Err(SandboxError::UnsupportedPlatform(
        "Apple Seatbelt / sandbox-exec is only supported on macOS".to_string(),
    ))
}
