use nexus_albaai::checkpoints::types::KnowledgeCapsuleRecord;
use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::memory::firewall::{AgentLearningContext, ScopeFirewall};
use nexus_albaai::memory::transfer::{
    compute_flip_rates, quarantine_if_negative, score_transfer_pair, DetectorConfig,
    TraceTransition, TransferObservation,
};
use nexus_albaai::memory::vector::{DeterministicEmbedder, VectorEmbedder};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_detector_config_validation_and_scoring_formula() {
    let mut config = DetectorConfig::default();
    assert!(config.validate().is_ok());

    config.threshold = 0.0;
    assert!(config.validate().is_err());
    config.threshold = 0.15;

    config.success_weight = 0.5;
    assert!(config.validate().is_err());
    config.success_weight = 0.45;

    let baseline = vec![
        TransferObservation {
            success: true,
            retry_and_error_turns: 1,
            human_approval_rejected: false,
            token_count: 1000,
            latency_ms: 500,
        },
        TransferObservation {
            success: false,
            retry_and_error_turns: 3,
            human_approval_rejected: false,
            token_count: 1200,
            latency_ms: 600,
        },
    ];

    let positive_treatment = vec![
        TransferObservation {
            success: true,
            retry_and_error_turns: 0,
            human_approval_rejected: false,
            token_count: 900,
            latency_ms: 400,
        },
        TransferObservation {
            success: true,
            retry_and_error_turns: 0,
            human_approval_rejected: false,
            token_count: 850,
            latency_ms: 380,
        },
    ];

    let positive_decision = score_transfer_pair(
        "cap-positive",
        "agent-1",
        &baseline,
        &positive_treatment,
        &config,
    );
    assert!(positive_decision.impact.score > 0.0);
    assert!(positive_decision.impact.success_delta > 0.0);
    assert!(positive_decision.impact.retry_delta < 0.0);
    assert!(!positive_decision.negative_transfer);
    assert!(!positive_decision.quarantined);

    let harmful_observations = vec![
        TransferObservation {
            success: false,
            retry_and_error_turns: 4,
            human_approval_rejected: true,
            token_count: 3000,
            latency_ms: 2000,
        };
        5
    ];

    let harmful_decision_under_min = score_transfer_pair(
        "cap-harmful-small",
        "agent-1",
        &baseline,
        &harmful_observations[..3],
        &config,
    );
    assert!(harmful_decision_under_min.impact.score < -config.threshold);
    assert_eq!(harmful_decision_under_min.sample_count, 3);
    assert!(!harmful_decision_under_min.quarantined);

    let harmful_decision = score_transfer_pair(
        "cap-harmful",
        "agent-1",
        &baseline,
        &harmful_observations,
        &config,
    );
    assert!(harmful_decision.impact.score < -config.threshold);
    assert!(harmful_decision.negative_transfer);
    assert!(harmful_decision.quarantined);
}

#[test]
fn test_trace_flip_rates_calculation() {
    let transitions = vec![
        TraceTransition {
            baseline_correct: false,
            treatment_correct: true,
        },
        TraceTransition {
            baseline_correct: true,
            treatment_correct: false,
        },
        TraceTransition {
            baseline_correct: true,
            treatment_correct: false,
        },
        TraceTransition {
            baseline_correct: true,
            treatment_correct: true,
        },
    ];

    let rates = compute_flip_rates(&transitions);
    assert_eq!(rates.pairs, 4);
    assert_eq!(rates.wrong_to_correct, 1);
    assert_eq!(rates.correct_to_wrong, 2);
    assert!((rates.net_benefit - (-0.25)).abs() < 1e-6);
}

#[tokio::test]
async fn test_negative_transfer_quarantine_persists_and_excludes_from_retrieval() {
    let temp_file = NamedTempFile::new().expect("temp file");
    let store = SqliteCheckpointStore::new(temp_file.path()).expect("store");

    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let capsule_id = Uuid::new_v4();

    let capsule = KnowledgeCapsuleRecord {
        id: capsule_id.to_string(),
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        source_run_id: None,
        source_agent_id: "agent_tester".to_string(),
        source_role: "tester".to_string(),
        source_domain_tags: vec!["testing".to_string()],
        trigger_preconditions: "flaky test assertion".to_string(),
        distilled_heuristic: "use sleep instead of waiting".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.85,
        negative_transfer: false,
        created_at: "2026-11-15T22:00:00Z".to_string(),
    };

    store
        .insert_knowledge_capsule(&capsule)
        .expect("insert capsule");

    let embedder = DeterministicEmbedder::default();
    let vec = embedder.embed("flaky test assertion sleep waiting");
    store
        .insert_capsule_vector(&capsule.id, &capsule.tenant_id, &capsule.workspace_id, &vec)
        .expect("insert vector");

    let query = embedder.embed("flaky test assertion waiting");
    let initial_results = store
        .search_capsules_vector(&tenant_id.to_string(), &workspace_id.to_string(), &query, 5)
        .expect("search");
    assert_eq!(initial_results.len(), 1);
    assert_eq!(initial_results[0].0.id, capsule.id);

    let baseline = vec![
        TransferObservation {
            success: true,
            retry_and_error_turns: 0,
            human_approval_rejected: false,
            token_count: 500,
            latency_ms: 200,
        };
        5
    ];
    let treatment = vec![
        TransferObservation {
            success: false,
            retry_and_error_turns: 5,
            human_approval_rejected: true,
            token_count: 2500,
            latency_ms: 1200,
        };
        5
    ];

    let decision = score_transfer_pair(
        &capsule.id,
        "agent_tester",
        &baseline,
        &treatment,
        &DetectorConfig::default(),
    );
    assert!(decision.quarantined);

    let quarantined = quarantine_if_negative(&store, &decision)
        .await
        .expect("quarantine");
    assert!(quarantined);

    let updated_capsule = store
        .get_knowledge_capsule(&capsule.id)
        .expect("get")
        .expect("exists");
    assert!(updated_capsule.negative_transfer);

    let results_after = store
        .search_capsules_vector(&tenant_id.to_string(), &workspace_id.to_string(), &query, 5)
        .expect("search after");
    assert!(
        results_after.is_empty(),
        "Quarantined capsule must be completely excluded from vector retrieval"
    );

    let firewall = ScopeFirewall::default();
    let target = AgentLearningContext {
        agent_id: "agent_consumer".to_string(),
        domain_tags: vec!["testing".to_string()],
        task_description: "flaky test assertion".to_string(),
        tenant_id,
        workspace_id,
        crew_id: None,
        workflow_id: None,
    };

    let mut quarantined_rec = updated_capsule.clone();
    quarantined_rec.propagation_scope = "WORKSPACE_QUARANTINE".to_string();
    let unapproved = firewall.authorize(&quarantined_rec, &target, None, false, &[]);
    assert!(!unapproved.allowed);
    assert!(unapproved.requires_approval);
}
