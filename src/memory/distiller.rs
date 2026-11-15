use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::capsule::KnowledgeCapsuleSpec;
use crate::schema::workflow::{PropagationScope, SensitivityLevel};

pub const MAX_PRECONDITIONS_LEN: usize = 4000;
pub const MAX_HEURISTIC_LEN: usize = 8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistillationTrigger {
    DebugRecovery,
    ApprovalCorrection,
    HighConfidenceTask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistillationCandidate {
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub source_agent_id: String,
    pub source_role: String,
    pub trigger: DistillationTrigger,
    pub heuristic: String,
    pub preconditions: String,
    #[serde(default)]
    pub domain_tags: Vec<String>,
    #[serde(default)]
    pub debug_turns: usize,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer_feedback: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew_id: Option<String>,
    #[serde(default)]
    pub propagation_scope: PropagationScope,
}

#[derive(Debug, Clone)]
pub struct EpisodicKnowledgeDistiller {
    pub high_confidence_threshold: f64,
}

impl Default for EpisodicKnowledgeDistiller {
    fn default() -> Self {
        Self {
            high_confidence_threshold: 0.9,
        }
    }
}

impl EpisodicKnowledgeDistiller {
    pub fn new(high_confidence_threshold: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&high_confidence_threshold),
            "high_confidence_threshold must be between 0.0 and 1.0"
        );
        Self {
            high_confidence_threshold,
        }
    }

    pub fn distill(&self, candidate: &DistillationCandidate) -> Option<KnowledgeCapsuleSpec> {
        match candidate.trigger {
            DistillationTrigger::DebugRecovery => {
                if candidate.debug_turns < 2 {
                    return None;
                }
            }
            DistillationTrigger::ApprovalCorrection => {
                let feedback = candidate.reviewer_feedback.as_deref().unwrap_or("").trim();
                if feedback.is_empty() {
                    return None;
                }
            }
            DistillationTrigger::HighConfidenceTask => {
                if candidate.confidence < self.high_confidence_threshold {
                    return None;
                }
            }
        }

        let heuristic = match candidate.trigger {
            DistillationTrigger::ApprovalCorrection => {
                candidate.reviewer_feedback.as_deref().unwrap_or("").trim()
            }
            DistillationTrigger::DebugRecovery | DistillationTrigger::HighConfidenceTask => {
                candidate.heuristic.trim()
            }
        };

        let preconditions = candidate.preconditions.trim();

        if heuristic.is_empty() || preconditions.is_empty() {
            return None;
        }

        let bounded_preconditions = if preconditions.len() > MAX_PRECONDITIONS_LEN {
            &preconditions[..MAX_PRECONDITIONS_LEN]
        } else {
            preconditions
        };

        let bounded_heuristic = if heuristic.len() > MAX_HEURISTIC_LEN {
            &heuristic[..MAX_HEURISTIC_LEN]
        } else {
            heuristic
        };

        let mut cleaned_tags = Vec::new();
        for tag in &candidate.domain_tags {
            let t = tag.trim().to_lowercase();
            if !t.is_empty() && !cleaned_tags.contains(&t) {
                cleaned_tags.push(t);
            }
        }

        let sensitivity = match candidate.trigger {
            DistillationTrigger::ApprovalCorrection => SensitivityLevel::ConfidentialPiiStripped,
            DistillationTrigger::DebugRecovery | DistillationTrigger::HighConfidenceTask => {
                SensitivityLevel::Internal
            }
        };

        let confidence = match candidate.trigger {
            DistillationTrigger::HighConfidenceTask => candidate.confidence,
            DistillationTrigger::DebugRecovery | DistillationTrigger::ApprovalCorrection => 0.75,
        };

        Some(KnowledgeCapsuleSpec {
            capsule_id: Uuid::new_v4(),
            tenant_id: candidate.tenant_id,
            workspace_id: candidate.workspace_id,
            source_agent_id: candidate.source_agent_id.clone(),
            source_role: candidate.source_role.clone(),
            source_domain_tags: cleaned_tags,
            trigger_preconditions: bounded_preconditions.to_string(),
            distilled_heuristic: bounded_heuristic.to_string(),
            prohibited_contexts: Vec::new(),
            propagation_scope: candidate.propagation_scope,
            sensitivity_level: sensitivity,
            source_workflow_id: candidate.workflow_id,
            source_crew_id: candidate.crew_id.clone(),
            confidence,
        })
    }
}
