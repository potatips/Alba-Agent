use nexus_albaai::checkpoints::types::KnowledgeCapsuleRecord;
use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::memory::distiller::{
    DistillationCandidate, DistillationTrigger, EpisodicKnowledgeDistiller,
};
use nexus_albaai::memory::vector::{
    bytes_to_vector, cosine_similarity, vector_to_bytes, DeterministicEmbedder, VectorEmbedder,
};
use nexus_albaai::schema::workflow::{PropagationScope, SensitivityLevel};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_distiller_conditions_and_refusal_when_below_threshold() {
    let distiller = EpisodicKnowledgeDistiller::new(0.9);
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();

    let base_candidate = DistillationCandidate {
        tenant_id,
        workspace_id,
        source_agent_id: "agent_coder".to_string(),
        source_role: "engineer".to_string(),
        trigger: DistillationTrigger::DebugRecovery,
        heuristic: "run tests in isolated sandbox".to_string(),
        preconditions: "test suite failure".to_string(),
        domain_tags: vec!["rust".to_string(), "testing".to_string()],
        debug_turns: 1,
        confidence: 0.8,
        reviewer_feedback: None,
        workflow_id: None,
        crew_id: None,
        propagation_scope: PropagationScope::WorkspaceVerified,
    };

    assert!(distiller.distill(&base_candidate).is_none());

    let mut debug_recovery_valid = base_candidate.clone();
    debug_recovery_valid.debug_turns = 2;
    let cap_debug = distiller.distill(&debug_recovery_valid);
    assert!(cap_debug.is_some());
    let cap_debug = cap_debug.unwrap();
    assert_eq!(cap_debug.confidence, 0.75);
    assert_eq!(cap_debug.sensitivity_level, SensitivityLevel::Internal);
    assert_eq!(
        cap_debug.distilled_heuristic,
        "run tests in isolated sandbox"
    );

    let approval_correction_empty = DistillationCandidate {
        trigger: DistillationTrigger::ApprovalCorrection,
        reviewer_feedback: Some("   ".to_string()),
        ..base_candidate.clone()
    };
    assert!(distiller.distill(&approval_correction_empty).is_none());

    let approval_correction_valid = DistillationCandidate {
        trigger: DistillationTrigger::ApprovalCorrection,
        reviewer_feedback: Some("always sanitize input before parsing".to_string()),
        ..base_candidate.clone()
    };
    let cap_appr = distiller.distill(&approval_correction_valid);
    assert!(cap_appr.is_some());
    let cap_appr = cap_appr.unwrap();
    assert_eq!(
        cap_appr.distilled_heuristic,
        "always sanitize input before parsing"
    );
    assert_eq!(
        cap_appr.sensitivity_level,
        SensitivityLevel::ConfidentialPiiStripped
    );
    assert_eq!(cap_appr.confidence, 0.75);

    let high_conf_below = DistillationCandidate {
        trigger: DistillationTrigger::HighConfidenceTask,
        confidence: 0.89,
        ..base_candidate.clone()
    };
    assert!(distiller.distill(&high_conf_below).is_none());

    let high_conf_valid = DistillationCandidate {
        trigger: DistillationTrigger::HighConfidenceTask,
        confidence: 0.95,
        ..base_candidate.clone()
    };
    let cap_conf = distiller.distill(&high_conf_valid);
    assert!(cap_conf.is_some());
    let cap_conf = cap_conf.unwrap();
    assert_eq!(cap_conf.confidence, 0.95);
    assert_eq!(cap_conf.sensitivity_level, SensitivityLevel::Internal);

    let empty_preconditions = DistillationCandidate {
        preconditions: "   ".to_string(),
        ..debug_recovery_valid.clone()
    };
    assert!(distiller.distill(&empty_preconditions).is_none());

    let empty_heuristic = DistillationCandidate {
        heuristic: "".to_string(),
        ..debug_recovery_valid.clone()
    };
    assert!(distiller.distill(&empty_heuristic).is_none());
}

#[test]
fn test_vector_embedder_deterministic_and_cosine_similarity() {
    let embedder = DeterministicEmbedder::default();
    assert_eq!(embedder.dimensions(), 64);

    let text_a = "Rust compiler borrow checker lifetime analysis";
    let text_b = "Rust memory safety and borrow checker rules";
    let text_c = "Delicious apple pie pastry baking recipe";

    let vec_a1 = embedder.embed(text_a);
    let vec_a2 = embedder.embed(text_a);
    assert_eq!(vec_a1, vec_a2);
    assert_eq!(vec_a1.len(), 64);

    let norm_a: f32 = vec_a1.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!((norm_a - 1.0).abs() < 1e-5);

    let vec_b = embedder.embed(text_b);
    let vec_c = embedder.embed(text_c);

    let sim_ab = cosine_similarity(&vec_a1, &vec_b);
    let sim_ac = cosine_similarity(&vec_a1, &vec_c);

    assert!(
        sim_ab > sim_ac,
        "Expected sim_ab ({:.3}) > sim_ac ({:.3})",
        sim_ab,
        sim_ac
    );

    let bytes = vector_to_bytes(&vec_a1);
    assert_eq!(bytes.len(), 64 * 4);
    let restored = bytes_to_vector(&bytes).expect("restored");
    assert_eq!(vec_a1, restored);
}

#[tokio::test]
async fn test_embedded_vector_retrieval_and_tenant_workspace_isolation() {
    let temp_file = NamedTempFile::new().expect("temp file");
    let store = SqliteCheckpointStore::new(temp_file.path()).expect("store");

    let tenant_1 = Uuid::new_v4();
    let tenant_2 = Uuid::new_v4();
    let workspace_1 = Uuid::new_v4();
    let workspace_2 = Uuid::new_v4();

    let embedder = DeterministicEmbedder::default();

    let cap_rust = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_1.to_string(),
        workspace_id: workspace_1.to_string(),
        source_run_id: None,
        source_agent_id: "agent_rust".to_string(),
        source_role: "rust_expert".to_string(),
        source_domain_tags: vec!["rust".to_string(), "concurrency".to_string()],
        trigger_preconditions: "tokio async race condition".to_string(),
        distilled_heuristic: "use Mutex or channels to serialize actor state".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.95,
        negative_transfer: false,
        created_at: "2026-11-15T22:00:00Z".to_string(),
    };

    let cap_python = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_1.to_string(),
        workspace_id: workspace_1.to_string(),
        source_run_id: None,
        source_agent_id: "agent_py".to_string(),
        source_role: "python_dev".to_string(),
        source_domain_tags: vec!["python".to_string(), "asyncio".to_string()],
        trigger_preconditions: "event loop blocked".to_string(),
        distilled_heuristic: "use asyncio.to_thread for synchronous IO".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.9,
        negative_transfer: false,
        created_at: "2026-11-15T22:01:00Z".to_string(),
    };

    let cap_quarantined = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_1.to_string(),
        workspace_id: workspace_1.to_string(),
        source_run_id: None,
        source_agent_id: "agent_bad".to_string(),
        source_role: "bad_expert".to_string(),
        source_domain_tags: vec!["rust".to_string(), "concurrency".to_string()],
        trigger_preconditions: "tokio async deadlock".to_string(),
        distilled_heuristic: "use unsafe static mut pointers".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.99,
        negative_transfer: true,
        created_at: "2026-11-15T22:02:00Z".to_string(),
    };

    store
        .insert_knowledge_capsule(&cap_rust)
        .expect("insert rust");
    store
        .insert_knowledge_capsule(&cap_python)
        .expect("insert py");
    store
        .insert_knowledge_capsule(&cap_quarantined)
        .expect("insert bad");

    let vec_rust = embedder.embed("tokio async concurrency channels Mutex actor");
    let vec_py = embedder.embed("python asyncio event loop blocking");
    let vec_bad = embedder.embed("tokio async concurrency deadlock unsafe static");

    store
        .insert_capsule_vector(
            &cap_rust.id,
            &cap_rust.tenant_id,
            &cap_rust.workspace_id,
            &vec_rust,
        )
        .expect("insert vec rust");
    store
        .insert_capsule_vector(
            &cap_python.id,
            &cap_python.tenant_id,
            &cap_python.workspace_id,
            &vec_py,
        )
        .expect("insert vec py");
    store
        .insert_capsule_vector(
            &cap_quarantined.id,
            &cap_quarantined.tenant_id,
            &cap_quarantined.workspace_id,
            &vec_bad,
        )
        .expect("insert vec bad");

    let query_rust = embedder.embed("how to resolve tokio async race with channels");
    let results_tenant1 = store
        .search_capsules_vector(
            &tenant_1.to_string(),
            &workspace_1.to_string(),
            &query_rust,
            5,
        )
        .expect("search");

    assert!(!results_tenant1.is_empty());
    assert_eq!(results_tenant1[0].0.id, cap_rust.id);
    assert!(results_tenant1[0].1 > 0.0);

    for (c, _) in &results_tenant1 {
        assert_ne!(
            c.id, cap_quarantined.id,
            "Quarantined capsule must be excluded from retrieval"
        );
        assert!(!c.negative_transfer);
    }

    let results_tenant2 = store
        .search_capsules_vector(
            &tenant_2.to_string(),
            &workspace_1.to_string(),
            &query_rust,
            5,
        )
        .expect("search tenant2");
    assert!(
        results_tenant2.is_empty(),
        "Cross-tenant search must return nothing"
    );

    let results_workspace2 = store
        .search_capsules_vector(
            &tenant_1.to_string(),
            &workspace_2.to_string(),
            &query_rust,
            5,
        )
        .expect("search ws2");
    assert!(
        results_workspace2.is_empty(),
        "Cross-workspace search must return nothing"
    );

    let top1 = store
        .search_capsules_vector(
            &tenant_1.to_string(),
            &workspace_1.to_string(),
            &query_rust,
            1,
        )
        .expect("search top1");
    assert_eq!(top1.len(), 1);
    assert_eq!(top1[0].0.id, cap_rust.id);
}
