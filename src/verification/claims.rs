use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path};

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ClaimError {
    #[error("Claimed file '{path}' does not exist on disk: {detail}")]
    MissingFile { path: String, detail: String },
    #[error("Claimed file '{path}' is empty (0 bytes)")]
    EmptyFile { path: String },
    #[error("Claimed path '{path}' escapes workspace via traversal or symlink")]
    SymlinkEscape { path: String },
    #[error("Claimed path '{path}' is invalid: {reason}")]
    InvalidPath { path: String, reason: String },
}

pub fn looks_like_file_path(token: &str) -> bool {
    let cleaned = token
        .trim()
        .trim_matches(|c: char| {
            c == '`'
                || c == '"'
                || c == '\''
                || c == '('
                || c == ')'
                || c == '['
                || c == ']'
                || c == '{'
                || c == '}'
                || c == '<'
                || c == '>'
                || c == ','
                || c == ';'
                || c == ':'
        })
        .trim_end_matches('.');

    let stripped = cleaned.strip_prefix("./").unwrap_or(cleaned);

    if stripped.is_empty()
        || stripped.contains(char::is_whitespace)
        || stripped.starts_with("http://")
        || stripped.starts_with("https://")
        || stripped.starts_with("ftp://")
        || stripped.starts_with("file://")
        || stripped.contains('\0')
    {
        return false;
    }

    let path = Path::new(stripped);
    let extension = match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => ext,
        None => return false,
    };

    if extension.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    if stripped.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }

    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return false,
    };

    if stem.is_empty() {
        return false;
    }

    let known_extensions = [
        "txt", "md", "json", "rs", "py", "js", "ts", "tsx", "html", "css", "yaml", "yml", "toml",
        "csv", "sh", "pdf", "png", "jpg", "jpeg", "gif", "webp", "mp4", "webm", "wav", "mp3",
        "zip", "tar", "gz", "sql", "bin", "log", "xml", "svg", "ini", "cfg",
    ];

    if known_extensions
        .iter()
        .any(|k| k.eq_ignore_ascii_case(extension))
    {
        return true;
    }

    extension.len() <= 5 && extension.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn clean_path_token(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_matches(|c: char| {
            c == '`'
                || c == '"'
                || c == '\''
                || c == '('
                || c == ')'
                || c == '['
                || c == ']'
                || c == '{'
                || c == '}'
                || c == '<'
                || c == '>'
                || c == ','
                || c == ';'
                || c == ':'
        })
        .trim_end_matches('.');

    trimmed.strip_prefix("./").unwrap_or(trimmed).to_string()
}

pub fn extract_claimed_files(text: &str) -> Vec<String> {
    let mut claims = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
        extract_claims_from_json(&val, &mut claims, &mut seen);
    }

    let claim_verbs = [
        "created file",
        "wrote file",
        "saved to",
        "saved file",
        "generated file",
        "generated",
        "produced",
        "written to",
        "output saved to",
        "file created at",
        "output file:",
        "deliverable:",
        "artifact:",
        "file:",
        "wrote",
        "created",
    ];

    let skip_tokens = ["a", "an", "the", "new", "file", "to", "out", "into"];

    let is_negated = |prefix: &str| {
        let p = prefix.trim();
        p.ends_with("not")
            || p.ends_with("didn't")
            || p.ends_with("did not")
            || p.ends_with("cannot")
            || p.ends_with("can't")
            || p.ends_with("could not")
            || p.ends_with("couldn't")
            || p.ends_with("failed to")
            || p.ends_with("fail to")
            || p.ends_with("unable to")
            || p.ends_with("never")
            || p.ends_with("to")
            || p.starts_with("please")
            || p.contains("instead of")
            || p.contains("without")
    };

    for line in text.lines() {
        let line_lower = line.to_lowercase();

        for verb in &claim_verbs {
            if let Some(pos) = line_lower.find(verb) {
                let prefix = &line_lower[..pos];
                if is_negated(prefix) {
                    continue;
                }
                let remainder = &line[pos + verb.len()..];
                for token in remainder.split_whitespace() {
                    let lower_t = token.to_lowercase();
                    let cleaned_t = clean_path_token(&lower_t);
                    if skip_tokens.contains(&cleaned_t.as_str()) {
                        continue;
                    }
                    let cleaned = clean_path_token(token);
                    if looks_like_file_path(&cleaned) && seen.insert(cleaned.clone()) {
                        claims.push(cleaned);
                        break;
                    }
                }
            }
        }

        let mut in_backtick = false;
        let mut current_token = String::new();
        for ch in line.chars() {
            if ch == '`' {
                if in_backtick {
                    let cleaned = clean_path_token(&current_token);
                    let line_indicates_file = line_lower.contains("creat")
                        || line_lower.contains("writ")
                        || line_lower.contains("save")
                        || line_lower.contains("generat")
                        || line_lower.contains("output")
                        || line_lower.contains("produc")
                        || line_lower.contains("deliver")
                        || line_lower.contains("file");

                    if line_indicates_file
                        && looks_like_file_path(&cleaned)
                        && seen.insert(cleaned.clone())
                    {
                        claims.push(cleaned);
                    }
                    current_token.clear();
                    in_backtick = false;
                } else {
                    in_backtick = true;
                }
            } else if in_backtick {
                current_token.push(ch);
            }
        }
    }

    claims
}

fn extract_claims_from_json(
    val: &serde_json::Value,
    claims: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match val {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let k_lower = k.to_lowercase();
                let is_file_key = k_lower == "file"
                    || k_lower == "path"
                    || k_lower == "output_file"
                    || k_lower == "deliverable"
                    || k_lower == "deliverables"
                    || k_lower == "artifacts"
                    || k_lower == "created_files"
                    || k_lower == "exported_files";

                if is_file_key {
                    match v {
                        serde_json::Value::String(s) => {
                            let cleaned = clean_path_token(s);
                            if looks_like_file_path(&cleaned) && seen.insert(cleaned.clone()) {
                                claims.push(cleaned);
                            }
                        }
                        serde_json::Value::Array(arr) => {
                            for item in arr {
                                if let Some(s) = item.as_str() {
                                    let cleaned = clean_path_token(s);
                                    if looks_like_file_path(&cleaned)
                                        && seen.insert(cleaned.clone())
                                    {
                                        claims.push(cleaned);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    extract_claims_from_json(v, claims, seen);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_claims_from_json(item, claims, seen);
            }
        }
        _ => {}
    }
}

pub struct ClaimsHook;

impl Default for ClaimsHook {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaimsHook {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_claims(&self, text: &str) -> Vec<String> {
        extract_claimed_files(text)
    }

    pub fn verify_claims(
        &self,
        workspace_root: &Path,
        claims: &[String],
    ) -> Result<(), ClaimError> {
        let canonical_ws =
            fs::canonicalize(workspace_root).map_err(|e| ClaimError::InvalidPath {
                path: workspace_root.display().to_string(),
                reason: format!("Cannot canonicalize workspace root: {}", e),
            })?;

        for claimed in claims {
            let p = Path::new(claimed);
            for comp in p.components() {
                match comp {
                    Component::ParentDir => {
                        return Err(ClaimError::SymlinkEscape {
                            path: claimed.clone(),
                        });
                    }
                    Component::RootDir | Component::Prefix(_) => {
                        return Err(ClaimError::InvalidPath {
                            path: claimed.clone(),
                            reason: "Absolute paths are forbidden in claims".to_string(),
                        });
                    }
                    _ => {}
                }
            }

            let full_target = canonical_ws.join(p);

            let mut probe = full_target.clone();
            loop {
                match fs::canonicalize(&probe) {
                    Ok(real) => {
                        if !real.starts_with(&canonical_ws) {
                            return Err(ClaimError::SymlinkEscape {
                                path: claimed.clone(),
                            });
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
                        return Err(ClaimError::InvalidPath {
                            path: claimed.clone(),
                            reason: format!("Failed path validation: {}", e),
                        });
                    }
                }
            }

            if !full_target.exists() {
                return Err(ClaimError::MissingFile {
                    path: claimed.clone(),
                    detail: "File does not exist on disk".to_string(),
                });
            }

            let metadata = fs::metadata(&full_target).map_err(|e| ClaimError::MissingFile {
                path: claimed.clone(),
                detail: e.to_string(),
            })?;

            if metadata.len() == 0 {
                return Err(ClaimError::EmptyFile {
                    path: claimed.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn verify_text_claims(
        &self,
        workspace_root: &Path,
        text: &str,
    ) -> Result<Vec<String>, ClaimError> {
        let claims = self.extract_claims(text);
        self.verify_claims(workspace_root, &claims)?;
        Ok(claims)
    }
}
