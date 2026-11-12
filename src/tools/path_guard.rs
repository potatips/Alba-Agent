use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::tools::types::ToolError;

pub const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

pub fn is_sensitive_component(comp_str: &str) -> bool {
    let lower = comp_str.to_lowercase();
    lower == ".env"
        || lower.starts_with(".env.")
        || lower == ".ssh"
        || lower == ".aws"
        || lower == ".gnupg"
        || lower == ".git"
        || lower == ".npmrc"
        || lower == ".pypirc"
        || lower.starts_with("id_rsa")
        || lower.starts_with("id_ed25519")
        || lower.starts_with("id_ecdsa")
        || lower.starts_with("id_dsa")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.ends_with(".keystore")
        || lower == "credentials.json"
        || lower == "secrets.yaml"
        || lower == "secrets.yml"
        || lower == "secrets.json"
}

pub fn normalize_relative_path(
    raw: &str,
    allow_dot_root: bool,
) -> Result<(PathBuf, String), ToolError> {
    if raw.contains('\0') {
        return Err(ToolError::InvalidArguments(
            "Path cannot contain null bytes".to_string(),
        ));
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        if allow_dot_root {
            return Ok((PathBuf::from("."), ".".to_string()));
        }
        return Err(ToolError::InvalidArguments(
            "Path cannot be empty".to_string(),
        ));
    }
    if trimmed.contains('\\') {
        return Err(ToolError::PathTraversal(
            "Backslashes are not allowed in relative paths".to_string(),
        ));
    }
    if trimmed.starts_with('/') {
        return Err(ToolError::PathTraversal(
            "Absolute paths are forbidden".to_string(),
        ));
    }

    if trimmed == "." {
        if allow_dot_root {
            return Ok((PathBuf::from("."), ".".to_string()));
        }
        return Err(ToolError::InvalidArguments(
            "File path cannot be '.'".to_string(),
        ));
    }

    let path = Path::new(trimmed);
    let mut parts = Vec::new();
    for comp in path.components() {
        match comp {
            Component::Normal(c) => {
                let s = c.to_string_lossy();
                if is_sensitive_component(&s) {
                    return Err(ToolError::AccessDenied(format!(
                        "Access to sensitive path '{}' is forbidden",
                        trimmed
                    )));
                }
                parts.push(s.into_owned());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ToolError::PathTraversal(format!(
                    "Parent traversal '..' is forbidden in '{}'",
                    trimmed
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ToolError::PathTraversal(
                    "Root or prefix paths are forbidden".to_string(),
                ));
            }
        }
    }

    if parts.is_empty() {
        if allow_dot_root {
            return Ok((PathBuf::from("."), ".".to_string()));
        }
        return Err(ToolError::InvalidArguments(
            "Path cannot be empty".to_string(),
        ));
    }

    let rel_str = parts.join("/");
    let rel_buf = parts.iter().collect::<PathBuf>();
    Ok((rel_buf, rel_str))
}

pub fn resolve_existing_directory(
    workspace_root: &Path,
    raw: &str,
) -> Result<(PathBuf, PathBuf, String), ToolError> {
    let real_root = fs::canonicalize(workspace_root)
        .map_err(|e| ToolError::ExecutionFailed(format!("Invalid workspace root: {}", e)))?;

    let (rel_buf, rel_str) = normalize_relative_path(raw, true)?;
    let joined = if rel_str == "." {
        real_root.clone()
    } else {
        real_root.join(&rel_buf)
    };

    let real_target = fs::canonicalize(&joined).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            ToolError::ExecutionFailed(format!("Directory '{}' not found", raw))
        }
        _ => ToolError::ExecutionFailed(format!("Cannot access '{}': {}", raw, e)),
    })?;

    if !real_target.starts_with(&real_root) {
        return Err(ToolError::PathTraversal(format!(
            "Resolved directory '{}' escapes workspace root",
            raw
        )));
    }

    if !real_target.is_dir() {
        return Err(ToolError::InvalidArguments(format!(
            "Path '{}' is not a directory",
            raw
        )));
    }

    Ok((real_root, real_target, rel_str))
}

pub fn resolve_writable_file(
    workspace_root: &Path,
    raw: &str,
) -> Result<(PathBuf, PathBuf, String), ToolError> {
    let real_root = fs::canonicalize(workspace_root)
        .map_err(|e| ToolError::ExecutionFailed(format!("Invalid workspace root: {}", e)))?;

    let (rel_buf, rel_str) = normalize_relative_path(raw, false)?;
    let target = real_root.join(&rel_buf);

    let mut probe = target.clone();
    loop {
        match fs::canonicalize(&probe) {
            Ok(real_ancestor) => {
                if !real_ancestor.starts_with(&real_root) {
                    return Err(ToolError::PathTraversal(format!(
                        "Path '{}' resolves outside workspace root via symlink",
                        raw
                    )));
                }
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(parent) = probe.parent() {
                    if parent == probe {
                        break;
                    }
                    probe = parent.to_path_buf();
                } else {
                    break;
                }
            }
            Err(e) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Cannot validate path '{}': {}",
                    raw, e
                )));
            }
        }
    }

    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.is_dir() {
            return Err(ToolError::InvalidArguments(format!(
                "Target path '{}' is a directory",
                raw
            )));
        }
    }

    Ok((real_root, target, rel_str))
}
