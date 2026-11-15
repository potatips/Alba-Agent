use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::checkpoints::store::CheckpointStore;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferObservation {
    pub success: bool,
    pub retry_and_error_turns: usize,
    pub human_approval_rejected: bool,
    pub token_count: usize,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferImpactScore {
    pub score: f64,
    pub success_delta: f64,
    pub retry_delta: f64,
    pub rejection_delta: f64,
    pub token_overhead_delta: f64,
    pub latency_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectorConfig {
    pub threshold: f64,
    pub minimum_applications: usize,
    pub success_weight: f64,
    pub retry_weight: f64,
    pub rejection_weight: f64,
    pub token_weight: f64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            threshold: 0.15,
            minimum_applications: 5,
            success_weight: 0.45,
            retry_weight: 0.25,
            rejection_weight: 0.20,
            token_weight: 0.10,
        }
    }
}

impl DetectorConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.threshold <= 0.0 || self.threshold > 1.0 {
            return Err("threshold must be in (0.0, 1.0]".to_string());
        }
        if self.minimum_applications == 0 {
            return Err("minimum_applications must be >= 1".to_string());
        }
        let sum =
            self.success_weight + self.retry_weight + self.rejection_weight + self.token_weight;
        if (sum - 1.0).abs() > 1e-6 {
            return Err("weights must sum to 1.0".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferPairDecision {
    pub capsule_id: String,
    pub target_agent_id: String,
    pub sample_count: usize,
    pub impact: TransferImpactScore,
    pub negative_transfer: bool,
    pub quarantined: bool,
}

fn mean<F: Fn(&TransferObservation) -> f64>(obs: &[TransferObservation], f: F) -> f64 {
    if obs.is_empty() {
        0.0
    } else {
        obs.iter().map(f).sum::<f64>() / obs.len() as f64
    }
}

pub fn score_transfer_pair(
    capsule_id: &str,
    target_agent_id: &str,
    baseline: &[TransferObservation],
    treatment: &[TransferObservation],
    config: &DetectorConfig,
) -> TransferPairDecision {
    let sample_count = treatment.len();
    if baseline.is_empty() || treatment.is_empty() {
        let empty_impact = TransferImpactScore {
            score: 0.0,
            success_delta: 0.0,
            retry_delta: 0.0,
            rejection_delta: 0.0,
            token_overhead_delta: 0.0,
            latency_delta: 0.0,
        };
        return TransferPairDecision {
            capsule_id: capsule_id.to_string(),
            target_agent_id: target_agent_id.to_string(),
            sample_count,
            impact: empty_impact,
            negative_transfer: false,
            quarantined: false,
        };
    }

    let b_success = mean(baseline, |o| if o.success { 1.0 } else { 0.0 });
    let t_success = mean(treatment, |o| if o.success { 1.0 } else { 0.0 });
    let success_delta = t_success - b_success;

    let b_retry = mean(baseline, |o| o.retry_and_error_turns as f64);
    let t_retry = mean(treatment, |o| o.retry_and_error_turns as f64);
    let retry_delta = t_retry - b_retry;

    let b_rejection = mean(
        baseline,
        |o| if o.human_approval_rejected { 1.0 } else { 0.0 },
    );
    let t_rejection = mean(
        treatment,
        |o| if o.human_approval_rejected { 1.0 } else { 0.0 },
    );
    let rejection_delta = t_rejection - b_rejection;

    let b_tokens = mean(baseline, |o| o.token_count as f64);
    let t_tokens = mean(treatment, |o| o.token_count as f64);
    let token_overhead_delta = (t_tokens - b_tokens) / b_tokens.max(1.0);

    let b_latency = mean(baseline, |o| o.latency_ms as f64);
    let t_latency = mean(treatment, |o| o.latency_ms as f64);
    let latency_delta = (t_latency - b_latency) / b_latency.max(1.0);

    let retry_penalty = (retry_delta / (b_retry + 1.0).max(1.0)).clamp(-1.0, 1.0);
    let efficiency_penalty = ((token_overhead_delta + latency_delta) / 2.0).clamp(-1.0, 1.0);

    let raw_score = (config.success_weight * success_delta)
        - (config.retry_weight * retry_penalty)
        - (config.rejection_weight * rejection_delta)
        - (config.token_weight * efficiency_penalty);

    let score = raw_score.clamp(-1.0, 1.0);

    let is_negative = sample_count >= config.minimum_applications && score < -config.threshold;

    TransferPairDecision {
        capsule_id: capsule_id.to_string(),
        target_agent_id: target_agent_id.to_string(),
        sample_count,
        impact: TransferImpactScore {
            score,
            success_delta,
            retry_delta,
            rejection_delta,
            token_overhead_delta,
            latency_delta,
        },
        negative_transfer: is_negative,
        quarantined: is_negative,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceTransition {
    pub baseline_correct: bool,
    pub treatment_correct: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlipRates {
    pub pairs: usize,
    pub wrong_to_correct: usize,
    pub correct_to_wrong: usize,
    pub net_benefit: f64,
}

pub fn compute_flip_rates(transitions: &[TraceTransition]) -> FlipRates {
    if transitions.is_empty() {
        return FlipRates {
            pairs: 0,
            wrong_to_correct: 0,
            correct_to_wrong: 0,
            net_benefit: 0.0,
        };
    }

    let mut w_to_c = 0usize;
    let mut c_to_w = 0usize;

    for t in transitions {
        if !t.baseline_correct && t.treatment_correct {
            w_to_c += 1;
        } else if t.baseline_correct && !t.treatment_correct {
            c_to_w += 1;
        }
    }

    let pairs = transitions.len();
    let net_benefit = (w_to_c as f64 - c_to_w as f64) / pairs as f64;

    FlipRates {
        pairs,
        wrong_to_correct: w_to_c,
        correct_to_wrong: c_to_w,
        net_benefit,
    }
}

pub async fn quarantine_if_negative<S: CheckpointStore>(
    store: &S,
    decision: &TransferPairDecision,
) -> Result<bool, crate::checkpoints::store::CheckpointError> {
    if decision.quarantined {
        let _ = Uuid::parse_str(&decision.capsule_id).map_err(|e| {
            crate::checkpoints::store::CheckpointError::SerializationError(e.to_string())
        })?;
        store
            .set_knowledge_capsule_negative_transfer(&decision.capsule_id, true)
            .await?;
        Ok(true)
    } else {
        Ok(false)
    }
}
