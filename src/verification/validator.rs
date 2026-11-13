use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::tools::path_guard::resolve_writable_file;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Verified,
    Recovered,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactCheck {
    pub path: String,
    pub status: CheckStatus,
    pub reason: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactContractReport {
    pub node_id: String,
    pub checks: Vec<ArtifactCheck>,
}

impl ArtifactContractReport {
    pub fn new(node_id: impl Into<String>, checks: Vec<ArtifactCheck>) -> Self {
        Self {
            node_id: node_id.into(),
            checks,
        }
    }

    pub fn passed(&self) -> bool {
        self.checks
            .iter()
            .all(|c| c.status == CheckStatus::Verified || c.status == CheckStatus::Recovered)
    }

    pub fn paths_with_status(&self, status: CheckStatus) -> Vec<String> {
        self.checks
            .iter()
            .filter(|c| c.status == status)
            .map(|c| c.path.clone())
            .collect()
    }
}

pub fn is_signature_ok(suffix: &str, head: &[u8]) -> bool {
    match suffix {
        ".png" => head.starts_with(b"\x89PNG\r\n\x1a\n"),
        ".jpg" | ".jpeg" => head.starts_with(b"\xff\xd8\xff"),
        ".gif" => head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a"),
        ".webp" => head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP",
        ".mp4" | ".mov" | ".m4v" | ".m4a" => head.len() >= 8 && &head[4..8] == b"ftyp",
        ".webm" | ".mkv" => head.starts_with(b"\x1a\x45\xdf\xa3"),
        ".wav" => head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WAVE",
        ".mp3" => {
            head.starts_with(b"ID3")
                || (head.len() > 1 && head[0] == 0xFF && (head[1] & 0xE0) == 0xE0)
        }
        ".zip" => head.starts_with(b"PK\x03\x04"),
        ".pdf" => head.starts_with(b"%PDF"),
        _ => true,
    }
}

pub fn is_text_suffix(suffix: &str) -> bool {
    matches!(
        suffix,
        ".md"
            | ".txt"
            | ".py"
            | ".ts"
            | ".tsx"
            | ".js"
            | ".mjs"
            | ".gd"
            | ".cs"
            | ".html"
            | ".css"
            | ".yaml"
            | ".yml"
            | ".csv"
            | ".toml"
            | ".srt"
            | ".vtt"
            | ".sql"
            | ".sh"
            | ".tscn"
            | ".tres"
            | ".cfg"
            | ".ini"
            | ".xml"
            | ".svg"
            | ".json"
    )
}

pub fn is_document_suffix(suffix: &str) -> bool {
    matches!(suffix, ".md" | ".txt" | ".json")
}

pub fn render_document(
    relative: &str,
    output: &serde_json::Map<String, serde_json::Value>,
) -> String {
    if relative.to_lowercase().ends_with(".json") {
        return serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".to_string()) + "\n";
    }

    let stem = Path::new(relative)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Deliverable");
    let title = stem
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let mut lines = Vec::new();
    lines.push(format!("# {}", title));
    lines.push(String::new());

    for (k, v) in output {
        if k == "artifact_contract" {
            continue;
        }
        let section_title = k.replace('_', " ");
        lines.push(format!("## {}", section_title));
        lines.push(String::new());
        if v.is_object() || v.is_array() {
            lines.push("```json".to_string());
            lines.push(serde_json::to_string_pretty(v).unwrap_or_default());
            lines.push("```".to_string());
        } else if let Some(s) = v.as_str() {
            lines.push(s.to_string());
        } else {
            lines.push(v.to_string());
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

pub struct ArtifactContractHook;

impl Default for ArtifactContractHook {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactContractHook {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_file(&self, workspace_root: &Path, relative: &str) -> ArtifactCheck {
        if relative.contains('\0') {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Invalid,
                reason: "Path cannot contain null bytes".to_string(),
                size_bytes: 0,
            };
        }
        if relative.trim().is_empty() {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Invalid,
                reason: "Path cannot be empty".to_string(),
                size_bytes: 0,
            };
        }
        if relative.contains('\\') {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Invalid,
                reason: "Backslashes are not allowed in paths".to_string(),
                size_bytes: 0,
            };
        }
        if relative.starts_with('/') {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Invalid,
                reason: "Absolute paths are forbidden".to_string(),
                size_bytes: 0,
            };
        }

        let rel_path = Path::new(relative);
        for comp in rel_path.components() {
            match comp {
                Component::ParentDir => {
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: "Parent traversal '..' is forbidden".to_string(),
                        size_bytes: 0,
                    };
                }
                Component::RootDir | Component::Prefix(_) => {
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: "Root or prefix paths are forbidden".to_string(),
                        size_bytes: 0,
                    };
                }
                _ => {}
            }
        }

        let canonical_ws = match fs::canonicalize(workspace_root) {
            Ok(p) => p,
            Err(e) => {
                return ArtifactCheck {
                    path: relative.to_string(),
                    status: CheckStatus::Invalid,
                    reason: format!("Cannot canonicalize workspace root: {}", e),
                    size_bytes: 0,
                };
            }
        };

        let target_path = canonical_ws.join(rel_path);

        let mut probe = target_path.clone();
        loop {
            match fs::canonicalize(&probe) {
                Ok(real_ancestor) => {
                    if !real_ancestor.starts_with(&canonical_ws) {
                        return ArtifactCheck {
                            path: relative.to_string(),
                            status: CheckStatus::Invalid,
                            reason: "Symlink escapes workspace root".to_string(),
                            size_bytes: 0,
                        };
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
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: format!("Cannot validate path: {}", e),
                        size_bytes: 0,
                    };
                }
            }
        }

        if !target_path.exists() {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Missing,
                reason: "The node finished without writing this file".to_string(),
                size_bytes: 0,
            };
        }

        if target_path.is_dir() {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Invalid,
                reason: "Declared artifact path is a directory".to_string(),
                size_bytes: 0,
            };
        }

        let metadata = match fs::symlink_metadata(&target_path) {
            Ok(m) => m,
            Err(e) => {
                return ArtifactCheck {
                    path: relative.to_string(),
                    status: CheckStatus::Invalid,
                    reason: format!("Cannot read file metadata: {}", e),
                    size_bytes: 0,
                };
            }
        };

        if metadata.file_type().is_symlink() {
            match fs::canonicalize(&target_path) {
                Ok(canonical_target) => {
                    if !canonical_target.starts_with(&canonical_ws) {
                        return ArtifactCheck {
                            path: relative.to_string(),
                            status: CheckStatus::Invalid,
                            reason: "Symlink escapes workspace root".to_string(),
                            size_bytes: 0,
                        };
                    }
                }
                Err(_) => {
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: "Dangling symlink".to_string(),
                        size_bytes: 0,
                    };
                }
            }
        }

        let size = match fs::metadata(&target_path) {
            Ok(m) => m.len(),
            Err(e) => {
                return ArtifactCheck {
                    path: relative.to_string(),
                    status: CheckStatus::Invalid,
                    reason: format!("Cannot read target metadata: {}", e),
                    size_bytes: 0,
                };
            }
        };

        if size == 0 {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Invalid,
                reason: "The file is empty".to_string(),
                size_bytes: 0,
            };
        }

        let suffix = rel_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| format!(".{}", ext.to_lowercase()))
            .unwrap_or_default();

        let mut file = match fs::File::open(&target_path) {
            Ok(f) => f,
            Err(e) => {
                return ArtifactCheck {
                    path: relative.to_string(),
                    status: CheckStatus::Invalid,
                    reason: format!("Cannot open file: {}", e),
                    size_bytes: size,
                };
            }
        };

        let mut head = [0u8; 16];
        use std::io::Read;
        let bytes_read = match file.read(&mut head) {
            Ok(n) => n,
            Err(e) => {
                return ArtifactCheck {
                    path: relative.to_string(),
                    status: CheckStatus::Invalid,
                    reason: format!("Cannot read file header: {}", e),
                    size_bytes: size,
                };
            }
        };

        if !is_signature_ok(&suffix, &head[..bytes_read]) {
            return ArtifactCheck {
                path: relative.to_string(),
                status: CheckStatus::Invalid,
                reason: format!("The file is not a valid {} container", suffix),
                size_bytes: size,
            };
        }

        if suffix == ".json" {
            let bytes = match fs::read(&target_path) {
                Ok(b) => b,
                Err(e) => {
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: format!("Cannot read JSON file: {}", e),
                        size_bytes: size,
                    };
                }
            };
            let text = match std::str::from_utf8(&bytes) {
                Ok(t) => t,
                Err(_) => {
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: "The text file is not UTF-8".to_string(),
                        size_bytes: size,
                    };
                }
            };
            if serde_json::from_str::<serde_json::Value>(text).is_err() {
                return ArtifactCheck {
                    path: relative.to_string(),
                    status: CheckStatus::Invalid,
                    reason: "The file is not valid JSON".to_string(),
                    size_bytes: size,
                };
            }
        } else if is_text_suffix(&suffix) {
            let bytes = match fs::read(&target_path) {
                Ok(b) => b,
                Err(e) => {
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: format!("Cannot read text file: {}", e),
                        size_bytes: size,
                    };
                }
            };
            let text = match std::str::from_utf8(&bytes) {
                Ok(t) => t,
                Err(_) => {
                    return ArtifactCheck {
                        path: relative.to_string(),
                        status: CheckStatus::Invalid,
                        reason: "The text file is not UTF-8".to_string(),
                        size_bytes: size,
                    };
                }
            };
            if text.trim().is_empty() {
                return ArtifactCheck {
                    path: relative.to_string(),
                    status: CheckStatus::Invalid,
                    reason: "The file contains only whitespace".to_string(),
                    size_bytes: size,
                };
            }
        }

        ArtifactCheck {
            path: relative.to_string(),
            status: CheckStatus::Verified,
            reason: String::new(),
            size_bytes: size,
        }
    }

    pub fn validate_node_artifacts(
        &self,
        workspace_root: &Path,
        node_id: &str,
        declared_paths: &[String],
        structured_output: Option<&serde_json::Map<String, serde_json::Value>>,
        sources: Option<&HashMap<String, Vec<u8>>>,
    ) -> ArtifactContractReport {
        let mut checks = Vec::new();

        for rel in declared_paths {
            let safe_target = match resolve_writable_file(workspace_root, rel) {
                Ok((_, target, _)) => target,
                Err(_) => {
                    let reason = if rel.contains("..") {
                        "Parent traversal '..' is forbidden".to_string()
                    } else {
                        "Symlink escapes workspace root".to_string()
                    };
                    checks.push(ArtifactCheck {
                        path: rel.clone(),
                        status: CheckStatus::Invalid,
                        reason,
                        size_bytes: 0,
                    });
                    continue;
                }
            };

            if !safe_target.is_file() {
                if let Some(src_map) = sources {
                    if let Some(data) = src_map.get(rel) {
                        if !data.is_empty() {
                            if let Some(parent) = safe_target.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            let _ = fs::write(&safe_target, data);
                        }
                    }
                }
            }

            let mut check = self.validate_file(workspace_root, rel);

            if check.status != CheckStatus::Verified {
                let suffix = Path::new(rel)
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| format!(".{}", ext.to_lowercase()))
                    .unwrap_or_default();

                if is_document_suffix(&suffix) {
                    if let Some(output) = structured_output {
                        let content = render_document(rel, output);
                        if !content.trim().is_empty() && content.len() <= 512_000 {
                            if let Some(parent) = safe_target.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            if fs::write(&safe_target, content.as_bytes()).is_ok() {
                                let re_check = self.validate_file(workspace_root, rel);
                                if re_check.status == CheckStatus::Verified {
                                    check = ArtifactCheck {
                                        path: rel.clone(),
                                        status: CheckStatus::Recovered,
                                        reason: format!(
                                            "Agent returned structured text; recovered document to {}",
                                            rel
                                        ),
                                        size_bytes: re_check.size_bytes,
                                    };
                                }
                            }
                        }
                    }
                }
            }

            checks.push(check);
        }

        ArtifactContractReport::new(node_id, checks)
    }
}
