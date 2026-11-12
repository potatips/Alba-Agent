use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use crate::sandbox::types::{
    collect_child_output, SandboxBackendKind, SandboxError, SandboxOutput, SandboxPolicy,
    SandboxStreamChunk,
};

const DEFAULT_PATH_ENV: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

pub fn find_binary(name: &str) -> Option<String> {
    let candidate = Path::new("/usr/bin").join(name);
    if candidate.is_file() {
        return Some(candidate.to_string_lossy().into_owned());
    }
    let candidate_bin = Path::new("/bin").join(name);
    if candidate_bin.is_file() {
        return Some(candidate_bin.to_string_lossy().into_owned());
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let p = Path::new(dir).join(name);
            if p.is_file() {
                return Some(p.to_string_lossy().into_owned());
            }
        }
    }
    None
}

pub fn build_bwrap_args(
    canonical_ws: &Path,
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
) -> Vec<String> {
    let ws_str = canonical_ws.to_string_lossy().into_owned();
    let mut bwrap_args = vec![
        "--ro-bind".to_string(),
        "/".to_string(),
        "/".to_string(),
        "--tmpfs".to_string(),
        "/tmp".to_string(),
        "--tmpfs".to_string(),
        "/home".to_string(),
        "--tmpfs".to_string(),
        "/root".to_string(),
        "--dev".to_string(),
        "/dev".to_string(),
        "--proc".to_string(),
        "/proc".to_string(),
        "--unshare-pid".to_string(),
        "--die-with-parent".to_string(),
    ];

    if !policy.allow_network {
        bwrap_args.push("--unshare-net".to_string());
    }

    bwrap_args.push("--bind".to_string());
    bwrap_args.push(ws_str.clone());
    bwrap_args.push(ws_str.clone());
    bwrap_args.push("--chdir".to_string());
    bwrap_args.push(ws_str.clone());

    bwrap_args.push("--clearenv".to_string());
    let path_val = std::env::var("PATH").unwrap_or_else(|_| DEFAULT_PATH_ENV.to_string());
    bwrap_args.extend(["--setenv".to_string(), "PATH".to_string(), path_val]);
    bwrap_args.extend(["--setenv".to_string(), "HOME".to_string(), ws_str]);
    bwrap_args.extend([
        "--setenv".to_string(),
        "LANG".to_string(),
        "C.UTF-8".to_string(),
    ]);

    for (k, v) in &policy.env {
        bwrap_args.extend(["--setenv".to_string(), k.clone(), v.clone()]);
    }

    bwrap_args.push("--".to_string());
    bwrap_args.push(program.to_string());
    bwrap_args.extend(args.iter().cloned());

    bwrap_args
}

pub fn build_unshare_invocation(
    canonical_ws: &Path,
    scratch_root: &Path,
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
) -> Vec<String> {
    let mut u_args = vec![
        "--user".to_string(),
        "--map-root-user".to_string(),
        "--mount".to_string(),
        "--pid".to_string(),
        "--fork".to_string(),
    ];
    if !policy.allow_network {
        u_args.push("--net".to_string());
    }

    let script = r#"
set -eu
ROOT="$1"
WS="$2"
PATH_VAL="$3"
shift 3
mount -t tmpfs tmpfs "$ROOT"
for d in bin usr lib lib64 sbin etc; do
  if [ -L "/$d" ]; then
    ln -s "$(readlink "/$d")" "$ROOT/$d"
  elif [ -d "/$d" ]; then
    mkdir -p "$ROOT/$d"
    mount --rbind "/$d" "$ROOT/$d"
    mount -o remount,ro,bind "$ROOT/$d" || true
  fi
done
mkdir -p "$ROOT/proc" "$ROOT/dev" "$ROOT/tmp" "$ROOT/home" "$ROOT/root"
mount -t proc proc "$ROOT/proc" || true
for devnode in null zero full random urandom tty; do
  if [ -e "/dev/$devnode" ]; then
    touch "$ROOT/dev/$devnode"
    mount --bind "/dev/$devnode" "$ROOT/dev/$devnode" || true
  fi
done
mount -t tmpfs tmpfs "$ROOT/tmp"
mount -t tmpfs tmpfs "$ROOT/home"
mount -t tmpfs tmpfs "$ROOT/root"
mkdir -p "$ROOT$WS"
mount --rbind "$WS" "$ROOT$WS"
CHROOT_BIN=$(command -v chroot || echo /usr/sbin/chroot)
exec "$CHROOT_BIN" "$ROOT" /usr/bin/env -i PATH="$PATH_VAL" HOME="$WS" LANG="C.UTF-8" /bin/sh -c 'cd "$1" && shift && exec "$@"' _ "$WS" "$@"
"#;

    let path_val = std::env::var("PATH").unwrap_or_else(|_| DEFAULT_PATH_ENV.to_string());
    u_args.push("/bin/sh".to_string());
    u_args.push("-c".to_string());
    u_args.push(script.to_string());
    u_args.push("_".to_string());
    u_args.push(scratch_root.to_string_lossy().into_owned());
    u_args.push(canonical_ws.to_string_lossy().into_owned());
    u_args.push(path_val);
    u_args.push(program.to_string());
    u_args.extend(args.iter().cloned());

    u_args
}

struct ScratchDir {
    path: std::path::PathBuf,
}

impl ScratchDir {
    fn create() -> Result<Self, SandboxError> {
        let dir = std::env::temp_dir().join(format!("alba-unshare-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).map_err(|e| {
            SandboxError::ExecutionFailed(format!("Failed creating scratch mount root: {}", e))
        })?;
        Ok(Self { path: dir })
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub async fn execute_linux(
    canonical_ws: &Path,
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
    stream_tx: Option<UnboundedSender<SandboxStreamChunk>>,
) -> Result<SandboxOutput, SandboxError> {
    let selected = match policy.backend {
        SandboxBackendKind::Seatbelt => {
            return Err(SandboxError::UnsupportedPlatform(
                "Seatbelt is macOS-only; use Auto, Bwrap, or Unshare on Linux".to_string(),
            ));
        }
        SandboxBackendKind::Bwrap => "bwrap",
        SandboxBackendKind::Unshare => "unshare",
        SandboxBackendKind::Auto => {
            if find_binary("bwrap").is_some() {
                "bwrap"
            } else if find_binary("unshare").is_some() {
                "unshare"
            } else {
                return Err(SandboxError::BackendUnavailable(
                    "Neither 'bwrap' nor 'unshare' is available on PATH".to_string(),
                ));
            }
        }
    };

    if selected == "bwrap" {
        let bin = find_binary("bwrap").ok_or_else(|| {
            SandboxError::BackendUnavailable("Bubblewrap ('bwrap') binary not found".to_string())
        })?;
        let bwrap_args = build_bwrap_args(canonical_ws, policy, program, args);
        let child = Command::new(bin)
            .args(&bwrap_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                SandboxError::ExecutionFailed(format!("Failed spawning bwrap process: {}", e))
            })?;

        return collect_child_output(
            child,
            "linux:bwrap",
            policy.timeout,
            policy.max_output_bytes,
            stream_tx,
        )
        .await;
    }

    let bin = find_binary("unshare").ok_or_else(|| {
        SandboxError::BackendUnavailable("'unshare' binary not found".to_string())
    })?;
    let scratch = ScratchDir::create()?;
    let unshare_args = build_unshare_invocation(canonical_ws, &scratch.path, policy, program, args);
    let mut cmd = Command::new(bin);
    let outer_path = format!(
        "{}:{}",
        DEFAULT_PATH_ENV,
        std::env::var("PATH").unwrap_or_default()
    );
    cmd.args(&unshare_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("PATH", outer_path)
        .kill_on_drop(true);

    let child = cmd.spawn().map_err(|e| {
        SandboxError::ExecutionFailed(format!("Failed spawning unshare process: {}", e))
    })?;

    collect_child_output(
        child,
        "linux:unshare",
        policy.timeout,
        policy.max_output_bytes,
        stream_tx,
    )
    .await
}
