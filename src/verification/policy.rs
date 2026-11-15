use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::checkpoints::types::ExecutionPolicyRecord;
use crate::tools::path_guard::is_sensitive_component;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyDecision {
    Deny,
    Ask,
    Allow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicyConfig {
    pub strict_mode: bool,
    pub allowed_domains: Vec<String>,
    pub denied_domains: Vec<String>,
}

impl Default for ExecutionPolicyConfig {
    fn default() -> Self {
        Self {
            strict_mode: false,
            allowed_domains: vec![
                "api.anthropic.com".to_string(),
                "generativelanguage.googleapis.com".to_string(),
            ],
            denied_domains: vec![
                "169.254.169.254".to_string(),
                "metadata.google.internal".to_string(),
            ],
        }
    }
}

impl From<&ExecutionPolicyRecord> for ExecutionPolicyConfig {
    fn from(rec: &ExecutionPolicyRecord) -> Self {
        Self {
            strict_mode: rec.default_mode.eq_ignore_ascii_case("strict"),
            allowed_domains: rec.allowed_domains.clone(),
            denied_domains: rec.denied_domains.clone(),
        }
    }
}

pub struct ExecutionPolicyEngine {
    config: ExecutionPolicyConfig,
}

impl Default for ExecutionPolicyEngine {
    fn default() -> Self {
        Self::new(ExecutionPolicyConfig::default())
    }
}

impl ExecutionPolicyEngine {
    pub fn new(config: ExecutionPolicyConfig) -> Self {
        Self { config }
    }

    pub fn from_record(record: &ExecutionPolicyRecord) -> Self {
        Self::new(ExecutionPolicyConfig::from(record))
    }

    pub fn set_strict_mode(&mut self, strict: bool) {
        self.config.strict_mode = strict;
    }

    pub fn is_strict_mode(&self) -> bool {
        self.config.strict_mode
    }

    pub fn evaluate_path(&self, raw_path: &str) -> PolicyDecision {
        let trimmed = raw_path
            .trim()
            .trim_matches('`')
            .trim_matches('"')
            .trim_matches('\'');
        if trimmed.is_empty() || trimmed.contains('\0') {
            return PolicyDecision::Deny;
        }

        if trimmed.starts_with('/') || trimmed.contains('\\') {
            return PolicyDecision::Deny;
        }

        let path = Path::new(trimmed);
        for comp in path.components() {
            match comp {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return PolicyDecision::Deny;
                }
                Component::Normal(c) => {
                    let s = c.to_string_lossy();
                    if is_sensitive_component(&s) {
                        return PolicyDecision::Deny;
                    }
                }
                Component::CurDir => {}
            }
        }

        PolicyDecision::Allow
    }

    pub fn evaluate_network(&self, target: &str) -> PolicyDecision {
        let clean = target
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("")
            .to_lowercase();

        if clean.is_empty() {
            return PolicyDecision::Deny;
        }

        for denied in &self.config.denied_domains {
            if clean == denied.to_lowercase()
                || clean.ends_with(&format!(".{}", denied.to_lowercase()))
            {
                return PolicyDecision::Deny;
            }
        }

        if clean == "169.254.169.254"
            || clean == "127.0.0.1"
            || clean == "localhost"
            || clean.starts_with("10.")
            || clean.starts_with("192.168.")
            || (clean.starts_with("172.") && is_private_172(&clean))
        {
            return PolicyDecision::Deny;
        }

        for allowed in &self.config.allowed_domains {
            if clean == allowed.to_lowercase()
                || clean.ends_with(&format!(".{}", allowed.to_lowercase()))
            {
                return PolicyDecision::Allow;
            }
        }

        if self.config.strict_mode {
            PolicyDecision::Deny
        } else {
            PolicyDecision::Ask
        }
    }

    pub fn evaluate_command(&self, cmd: &str) -> PolicyDecision {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return PolicyDecision::Allow;
        }

        let cmd_lower = trimmed.to_lowercase();

        let deny_keywords = [
            "sudo",
            "su -",
            "su root",
            "doas",
            "mkfs",
            ":(){ :|:& };:",
            "shutdown",
            "reboot",
            "init 0",
            "init 6",
        ];

        for kw in &deny_keywords {
            if cmd_lower == *kw
                || cmd_lower.starts_with(&format!("{} ", kw))
                || cmd_lower.starts_with(&format!("{}.", kw))
                || cmd_lower.contains(&format!("; {}", kw))
                || cmd_lower.contains(&format!("&& {}", kw))
                || cmd_lower.contains(&format!("|| {}", kw))
                || cmd_lower.contains(&format!("| {}", kw))
                || (*kw == "mkfs" && cmd_lower.contains("mkfs"))
            {
                return PolicyDecision::Deny;
            }
        }

        if (cmd_lower.contains("curl") || cmd_lower.contains("wget"))
            && (cmd_lower.contains("| sh")
                || cmd_lower.contains("| bash")
                || cmd_lower.contains("|sh")
                || cmd_lower.contains("|bash")
                || cmd_lower.contains("| zsh")
                || cmd_lower.contains("|zsh"))
        {
            return PolicyDecision::Deny;
        }

        if cmd_lower.contains("rm ")
            && (cmd_lower.contains("-rf /")
                || cmd_lower.contains("-fr /")
                || cmd_lower.contains("-r /")
                || cmd_lower.contains("-rf ~")
                || cmd_lower.contains("-rf ..")
                || cmd_lower.contains("rm -rf /*")
                || cmd_lower.contains("rm -rf /..")
                || cmd_lower.contains("rm -rf ../.."))
        {
            return PolicyDecision::Deny;
        }

        for token in trimmed.split(|c: char| c.is_whitespace() || c == ';' || c == '|' || c == '&')
        {
            let cleaned = token.trim_matches(|c: char| {
                c == '`' || c == '"' || c == '\'' || c == '(' || c == ')' || c == '[' || c == ']'
            });
            if !cleaned.is_empty() {
                let p = Path::new(cleaned);
                for comp in p.components() {
                    if let Component::Normal(c) = comp {
                        if is_sensitive_component(&c.to_string_lossy()) {
                            return PolicyDecision::Deny;
                        }
                    }
                }
            }
        }

        if cmd_lower.contains("/etc/shadow")
            || cmd_lower.contains("/etc/passwd")
            || cmd_lower.contains("/etc/sudoers")
        {
            return PolicyDecision::Deny;
        }

        for denied in &self.config.denied_domains {
            if cmd_lower.contains(denied) {
                return PolicyDecision::Deny;
            }
        }

        let ask_keywords = [
            "npm install -g",
            "pip install",
            "apt-get",
            "apt install",
            "brew install",
            "cargo install",
            "git push",
            "pkill",
            "kill -9",
            "killall",
            "crontab",
            "cron",
            "core.hookspath",
            "hookspath",
        ];

        for kw in &ask_keywords {
            if cmd_lower.contains(kw) {
                return if self.config.strict_mode {
                    PolicyDecision::Deny
                } else {
                    PolicyDecision::Ask
                };
            }
        }

        if cmd_lower.starts_with("curl ")
            || cmd_lower.starts_with("wget ")
            || cmd_lower.starts_with("ssh ")
            || cmd_lower.starts_with("scp ")
            || cmd_lower.starts_with("nc ")
        {
            let mut has_allowed_domain = false;
            for allowed in &self.config.allowed_domains {
                if cmd_lower.contains(allowed) {
                    has_allowed_domain = true;
                    break;
                }
            }

            if !has_allowed_domain {
                return if self.config.strict_mode {
                    PolicyDecision::Deny
                } else {
                    PolicyDecision::Ask
                };
            }
        }

        let safe_prefixes = [
            "cargo test",
            "cargo check",
            "cargo build",
            "git status",
            "git diff",
            "pwd",
        ];
        for prefix in &safe_prefixes {
            if cmd_lower == *prefix || cmd_lower.starts_with(&format!("{} ", prefix)) {
                return PolicyDecision::Allow;
            }
        }
        if cmd_lower == "ls" || cmd_lower.starts_with("ls ") {
            return PolicyDecision::Allow;
        }

        if self.config.strict_mode {
            PolicyDecision::Ask
        } else {
            PolicyDecision::Allow
        }
    }

    pub fn evaluate(&self, kind: PolicyTarget<'_>) -> PolicyDecision {
        match kind {
            PolicyTarget::Command(c) => self.evaluate_command(c),
            PolicyTarget::Path(p) => self.evaluate_path(p),
            PolicyTarget::Network(n) => self.evaluate_network(n),
        }
    }
}

pub enum PolicyTarget<'a> {
    Command(&'a str),
    Path(&'a str),
    Network(&'a str),
}

fn is_private_172(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 {
        if let Ok(second) = parts[1].parse::<u8>() {
            return (16..=31).contains(&second);
        }
    }
    false
}
