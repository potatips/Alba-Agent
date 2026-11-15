use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use crate::checkpoints::store::CheckpointStore;
use crate::checkpoints::types::{AuditLogEntry, KnowledgeCapsuleRecord};
use crate::domain::ExecutionScope;
use crate::schema::workflow::PropagationScope;

#[derive(Debug, Clone)]
pub struct RedactionEngine {
    secret_patterns: Vec<Regex>,
    path_pattern: Regex,
}

impl Default for RedactionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionEngine {
    pub fn new() -> Self {
        let raw_patterns = vec![
            r"(?i)\b(?:sk|nx-sk)-[A-Za-z0-9_-]{12,}\b",
            r"(?i)\bsk-ant-[A-Za-z0-9_-]{20,}\b",
            r"(?i)\b(?:sk|rk)_(?:live|test)_[0-9a-zA-Z]{24,}\b",
            r"\bAIza[0-9A-Za-z-_]{35}\b",
            r"\bAKIA[0-9A-Z]{16}\b",
            r"(?i)\baws_secret_access_key\s*[:=]\s*[A-Za-z0-9/+=]{40}\b",
            r"\bghp_[A-Za-z0-9]{36}\b|\bgithub_pat_[A-Za-z0-9_]{30,}\b",
            r"\bxox[baprs]-[0-9a-zA-Z-]{10,}\b",
            r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----",
            r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b",
            r"\bBearer\s+[A-Za-z0-9._~+/-]{20,}\b",
            r"(?i)\bapi[_ -]?key\s*[:=]\s*[^\s,;]+",
            r"(?i)\baccess[_ -]?token\s*[:=]\s*[^\s,;]+",
            r"(?i)\bpassword\s*[:=]\s*[^\s,;]+",
            r"(?i)\bsecret\s*[:=]\s*[^\s,;]+",
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
            r"\b(?:\+?\d[\d .()/-]{7,}\d)\b",
            r"\b\d{3}-\d{2}-\d{4}\b|(?i)\bssn\s*[:=]\s*[^\s,;]+",
            r"\b(?:\d{4}[ -]?){3}\d{4}\b",
            r"(?i)\b(?:customer|client|contact|person)\s+(?:name\s+)?(?:is\s+)?[A-Z][a-z]+(?:\s+[A-Z][a-z]+){1,2}\b",
            r"(?i)\baccount(?:_id|[_ ]number)\s*[:=]\s*[0-9A-Za-z_-]+",
            r"(?:postgres|postgresql|mysql|mongodb(?:\+srv)?):\/\/[^:]+:[^@]+@",
        ];

        let secret_patterns = raw_patterns
            .into_iter()
            .map(|pat| Regex::new(pat).expect("valid regex"))
            .collect();

        let path_pattern = Regex::new(r"/(?:Users|home)/[a-zA-Z0-9._-]+/").expect("valid regex");

        Self {
            secret_patterns,
            path_pattern,
        }
    }

    pub fn redact(&self, input: &str, sensitive_values: &[String]) -> String {
        let mut out = input.to_string();
        for val in sensitive_values {
            if val.len() >= 3 {
                let escaped = regex::escape(val);
                if let Ok(re) = Regex::new(&format!("(?i){}", escaped)) {
                    out = re.replace_all(&out, "[REDACTED]").to_string();
                }
            }
        }
        for pat in &self.secret_patterns {
            out = pat.replace_all(&out, "[REDACTED]").to_string();
        }
        out = self
            .path_pattern
            .replace_all(&out, "/[REDACTED_USER]/")
            .to_string();
        out
    }

    pub fn sanitize_capsule(
        &self,
        capsule: &KnowledgeCapsuleRecord,
        sensitive_values: &[String],
    ) -> KnowledgeCapsuleRecord {
        let mut clean = capsule.clone();
        clean.trigger_preconditions = self.redact(&clean.trigger_preconditions, sensitive_values);
        clean.distilled_heuristic = self.redact(&clean.distilled_heuristic, sensitive_values);
        clean.source_domain_tags = clean
            .source_domain_tags
            .iter()
            .map(|tag| self.redact(tag, sensitive_values))
            .collect();
        clean.prohibited_contexts = clean
            .prohibited_contexts
            .iter()
            .map(|ctx| self.redact(ctx, sensitive_values))
            .collect();
        clean
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLearningContext {
    pub agent_id: String,
    pub domain_tags: Vec<String>,
    pub task_description: String,
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearningPolicy {
    pub source_agent_id: String,
    pub target_agent_id: String,
    pub scope: PropagationScope,
    #[serde(default = "default_true")]
    pub allow_heuristics: bool,
    #[serde(default)]
    pub allow_code_patterns: bool,
    #[serde(default)]
    pub require_human_approval: bool,
    #[serde(default = "default_true")]
    pub allowed: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirewallDecision {
    pub allowed: bool,
    pub reason: String,
    pub capsule: Option<KnowledgeCapsuleRecord>,
    pub compatibility: f64,
    pub requires_approval: bool,
}

#[derive(Debug, Clone)]
pub struct ScopeFirewall {
    pub top_k: usize,
    pub minimum_compatibility: f64,
    pub redaction: RedactionEngine,
}

impl Default for ScopeFirewall {
    fn default() -> Self {
        Self {
            top_k: 3,
            minimum_compatibility: 0.08,
            redaction: RedactionEngine::new(),
        }
    }
}

impl ScopeFirewall {
    pub fn new(top_k: usize, minimum_compatibility: f64) -> Self {
        assert!(top_k >= 1, "top_k must be >= 1");
        assert!(
            (0.0..=1.0).contains(&minimum_compatibility),
            "minimum_compatibility must be in [0.0, 1.0]"
        );
        Self {
            top_k,
            minimum_compatibility,
            redaction: RedactionEngine::new(),
        }
    }

    pub fn compute_compatibility(
        capsule: &KnowledgeCapsuleRecord,
        target: &AgentLearningContext,
    ) -> f64 {
        let source_tags: HashSet<String> = capsule
            .source_domain_tags
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        let target_tags: HashSet<String> = target
            .domain_tags
            .iter()
            .map(|t| t.to_lowercase())
            .collect();

        let tag_overlap = if source_tags.is_empty() && target_tags.is_empty() {
            1.0
        } else {
            let intersect = source_tags.intersection(&target_tags).count();
            let union = source_tags.union(&target_tags).count();
            if union == 0 {
                0.0
            } else {
                intersect as f64 / union as f64
            }
        };

        let tokenize = |text: &str| -> HashSet<String> {
            text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_lowercase())
                .collect()
        };

        let task_tokens = tokenize(&target.task_description);
        let precondition_tokens = tokenize(&capsule.trigger_preconditions);

        let task_overlap = if task_tokens.is_empty() && precondition_tokens.is_empty() {
            1.0
        } else {
            let intersect = task_tokens.intersection(&precondition_tokens).count();
            let union = task_tokens.union(&precondition_tokens).count();
            if union == 0 {
                0.0
            } else {
                intersect as f64 / union as f64
            }
        };

        let combined = (tag_overlap * 0.8) + (task_overlap * 0.2);
        combined.clamp(0.0, 1.0)
    }

    pub fn authorize(
        &self,
        capsule: &KnowledgeCapsuleRecord,
        target: &AgentLearningContext,
        policy: Option<&LearningPolicy>,
        approved: bool,
        sensitive_values: &[String],
    ) -> FirewallDecision {
        if capsule.tenant_id != target.tenant_id.to_string()
            || capsule.workspace_id != target.workspace_id.to_string()
        {
            return FirewallDecision {
                allowed: false,
                reason: "tenant or workspace boundary mismatch".to_string(),
                capsule: None,
                compatibility: 0.0,
                requires_approval: false,
            };
        }

        if capsule.prohibited_contexts.contains(&target.agent_id) {
            return FirewallDecision {
                allowed: false,
                reason: "target is explicitly prohibited".to_string(),
                capsule: None,
                compatibility: 0.0,
                requires_approval: false,
            };
        }

        let target_tags: HashSet<String> = target
            .domain_tags
            .iter()
            .map(|t| t.to_lowercase())
            .collect();
        for prohibited in &capsule.prohibited_contexts {
            if target_tags.contains(&prohibited.to_lowercase()) {
                return FirewallDecision {
                    allowed: false,
                    reason: "target domain is explicitly prohibited".to_string(),
                    capsule: None,
                    compatibility: 0.0,
                    requires_approval: false,
                };
            }
        }

        let prop_scope = capsule.propagation_scope.as_str();
        match prop_scope {
            "AGENT_PRIVATE" => {
                if target.agent_id != capsule.source_agent_id {
                    return FirewallDecision {
                        allowed: false,
                        reason: "capsule is agent-private".to_string(),
                        capsule: None,
                        compatibility: 0.0,
                        requires_approval: false,
                    };
                }
            }
            "CREW_LOCAL" => {
                let matches_crew = target.crew_id.is_some()
                    && target.crew_id.as_deref() == capsule.source_crew_id.as_deref();
                if !matches_crew {
                    return FirewallDecision {
                        allowed: false,
                        reason: "capsule is restricted to its source crew".to_string(),
                        capsule: None,
                        compatibility: 0.0,
                        requires_approval: false,
                    };
                }
            }
            "WORKFLOW_SCOPED" => {
                let target_wf = target.workflow_id.map(|w| w.to_string());
                let matches_wf = target_wf.is_some()
                    && target_wf.as_deref() == capsule.source_workflow_id.as_deref();
                if !matches_wf {
                    return FirewallDecision {
                        allowed: false,
                        reason: "capsule is restricted to its source workflow".to_string(),
                        capsule: None,
                        compatibility: 0.0,
                        requires_approval: false,
                    };
                }
            }
            "WORKSPACE_QUARANTINE" => {
                if !approved {
                    return FirewallDecision {
                        allowed: false,
                        reason: "quarantined workspace capsules require explicit approval"
                            .to_string(),
                        capsule: None,
                        compatibility: 0.0,
                        requires_approval: true,
                    };
                }
            }
            "WORKSPACE_VERIFIED" => {}
            _ => {
                return FirewallDecision {
                    allowed: false,
                    reason: format!("unknown propagation scope: {}", prop_scope),
                    capsule: None,
                    compatibility: 0.0,
                    requires_approval: false,
                };
            }
        }

        if let Some(pol) = policy {
            if !pol.allowed {
                return FirewallDecision {
                    allowed: false,
                    reason: "learning policy disallows transfer".to_string(),
                    capsule: None,
                    compatibility: 0.0,
                    requires_approval: false,
                };
            }
            if pol.source_agent_id != capsule.source_agent_id
                || pol.target_agent_id != target.agent_id
            {
                return FirewallDecision {
                    allowed: false,
                    reason: "policy source/target agent mismatch".to_string(),
                    capsule: None,
                    compatibility: 0.0,
                    requires_approval: false,
                };
            }
            if pol.require_human_approval && !approved {
                return FirewallDecision {
                    allowed: false,
                    reason: "human approval is required by policy".to_string(),
                    capsule: None,
                    compatibility: 0.0,
                    requires_approval: true,
                };
            }

            let has_code_pattern = capsule.distilled_heuristic.contains("```")
                || capsule.distilled_heuristic.lines().any(|l| {
                    let trim = l.trim_start();
                    trim.starts_with("def ")
                        || trim.starts_with("fn ")
                        || trim.starts_with("class ")
                        || trim.starts_with("import ")
                        || trim.starts_with("from ")
                        || trim.starts_with("#!/")
                        || trim.starts_with("$ ")
                });

            if has_code_pattern && !pol.allow_code_patterns {
                return FirewallDecision {
                    allowed: false,
                    reason: "policy disallows code patterns".to_string(),
                    capsule: None,
                    compatibility: 0.0,
                    requires_approval: false,
                };
            }
            if !has_code_pattern && !pol.allow_heuristics {
                return FirewallDecision {
                    allowed: false,
                    reason: "policy disallows heuristics".to_string(),
                    capsule: None,
                    compatibility: 0.0,
                    requires_approval: false,
                };
            }
        }

        let score = Self::compute_compatibility(capsule, target);
        if score < self.minimum_compatibility {
            return FirewallDecision {
                allowed: false,
                reason: format!(
                    "target task is not domain-compatible ({:.3} < {:.3})",
                    score, self.minimum_compatibility
                ),
                capsule: None,
                compatibility: score,
                requires_approval: false,
            };
        }

        let out_capsule = if prop_scope == "AGENT_PRIVATE" {
            capsule.clone()
        } else {
            self.redaction.sanitize_capsule(capsule, sensitive_values)
        };

        FirewallDecision {
            allowed: true,
            reason: "propagation allowed".to_string(),
            capsule: Some(out_capsule),
            compatibility: score,
            requires_approval: false,
        }
    }

    pub async fn authorize_and_audit<S: CheckpointStore>(
        &self,
        store: &S,
        capsule: &KnowledgeCapsuleRecord,
        target: &AgentLearningContext,
        policy: Option<&LearningPolicy>,
        approved: bool,
        sensitive_values: &[String],
    ) -> Result<FirewallDecision, crate::checkpoints::store::CheckpointError> {
        let decision = self.authorize(capsule, target, policy, approved, sensitive_values);
        if !decision.allowed {
            let scope = ExecutionScope {
                tenant_id: target.tenant_id,
                workspace_id: target.workspace_id,
                run_id: Uuid::nil(),
            };
            let audit = AuditLogEntry {
                id: Uuid::new_v4(),
                scope,
                actor_id: Some(target.agent_id.clone()),
                action: "memory.cross_scope_denied".to_string(),
                target_id: Uuid::parse_str(&capsule.id).ok(),
                details: serde_json::json!({
                    "capsule_id": capsule.id,
                    "target_agent_id": target.agent_id,
                    "capsule_tenant": capsule.tenant_id,
                    "target_tenant": target.tenant_id.to_string(),
                    "capsule_workspace": capsule.workspace_id,
                    "target_workspace": target.workspace_id.to_string(),
                    "propagation_scope": capsule.propagation_scope,
                    "reason": decision.reason,
                }),
                created_at: Utc::now(),
            };
            store.record_audit_log(&audit).await?;
        }
        Ok(decision)
    }
}
