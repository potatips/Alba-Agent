use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::checkpoints::types::KnowledgeCapsuleRecord;
use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::memory::firewall::{
    AgentLearningContext, LearningPolicy, RedactionEngine, ScopeFirewall,
};
use nexus_albaai::schema::workflow::PropagationScope;
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_redaction_engine_catches_20_distinct_secret_pii_patterns() {
    let engine = RedactionEngine::new();

    let secret_samples = vec![
        ("sk-1234567890abcdef1234567890", "openai_key"),
        ("nx-sk-abcdef1234567890", "nexus_key"),
        ("sk-ant-1234567890abcdef1234567890", "anthropic_key"),
        ("sk_live_123456789012345678901234", "stripe_live_key"),
        ("AIzaSyB1234567890abcdef1234567890abcdef", "google_api_key"),
        ("AKIAIOSFODNN7EXAMPLE", "aws_access_key"),
        ("aws_secret_access_key: wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY", "aws_secret_key"),
        ("ghp_1234567890abcdefghijklmnopqrstuvwxyz", "github_pat"),
        ("xoxb-1234567890-123456789012-abcdef123456", "slack_bot_token"),
        ("-----BEGIN RSA PRIVATE KEY-----", "rsa_private_key"),
        ("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c", "jwt_token"),
        ("Bearer 1234567890abcdef1234567890abcdef", "bearer_token"),
        ("api_key: secret_super_key_999", "generic_api_key"),
        ("access_token: token_xyz_998877", "generic_access_token"),
        ("password: SuperSecretPassword123!", "generic_password"),
        ("secret: top_secret_data_55", "generic_secret"),
        ("alice.smith@example.corporate.org", "email_address"),
        ("+1 415 555 2671", "phone_number"),
        ("123-45-6789", "ssn"),
        ("4111 2222 3333 4444", "credit_card"),
        ("customer name Alice Wonderland", "customer_name"),
        ("account_id: ACC-99887766", "account_id"),
        ("postgresql://admin:super_secret_pw@db.internal:5432/main", "db_uri"),
        ("/Users/john_doe/documents/plan.txt", "macos_user_path"),
        ("/home/alice_dev/project/code.rs", "linux_user_path"),
    ];

    assert!(secret_samples.len() >= 20);

    for (sample, kind) in secret_samples {
        let redacted = engine.redact(sample, &[]);
        assert!(
            redacted.contains("[REDACTED") || redacted.contains("[REDACTED_USER]"),
            "Pattern for {} failed to redact: '{}' -> '{}'",
            kind,
            sample,
            redacted
        );
    }
}

#[test]
fn test_redaction_engine_no_false_positives_on_10_benign_samples() {
    let engine = RedactionEngine::new();

    let benign_samples = vec![
        "fn calculate_tax(amount: f64) -> f64 { amount * 0.1 }",
        "public key cryptography uses asymmetric key pairs",
        "https://example.com/api/v1/documentation",
        "let token_count = 42;",
        "v1.2.3-alpha.4",
        "2026-11-15T22:30:00Z",
        "src/workflow/engine.rs",
        "Connection refused on port 8080",
        "retry_weight = 0.25; success_rate = 0.95",
        "SELECT id, name FROM users WHERE active = true",
    ];

    assert_eq!(benign_samples.len(), 10);

    for sample in benign_samples {
        let redacted = engine.redact(sample, &[]);
        assert_eq!(
            redacted, sample,
            "Benign sample falsely redacted: '{}' -> '{}'",
            sample, redacted
        );
    }
}

#[tokio::test]
async fn test_agent_private_capsule_never_returned_to_another_agent() {
    let temp_file = NamedTempFile::new().expect("temp file");
    let store = SqliteCheckpointStore::new(temp_file.path()).expect("store");

    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();

    let capsule = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        source_run_id: None,
        source_agent_id: "agent_alpha".to_string(),
        source_role: "researcher".to_string(),
        source_domain_tags: vec!["analysis".to_string()],
        trigger_preconditions: "needs deep research".to_string(),
        distilled_heuristic: "use primary sources first".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "AGENT_PRIVATE".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.95,
        negative_transfer: false,
        created_at: "2026-11-15T22:00:00Z".to_string(),
    };

    store.insert_knowledge_capsule(&capsule).expect("insert");

    let firewall = ScopeFirewall::default();

    let author_context = AgentLearningContext {
        agent_id: "agent_alpha".to_string(),
        domain_tags: vec!["analysis".to_string()],
        task_description: "deep research on datasets".to_string(),
        tenant_id,
        workspace_id,
        crew_id: None,
        workflow_id: None,
    };

    let author_decision = firewall.authorize(&capsule, &author_context, None, false, &[]);
    assert!(author_decision.allowed);
    assert_eq!(author_decision.capsule.unwrap().id, capsule.id);

    let other_agent_context = AgentLearningContext {
        agent_id: "agent_beta".to_string(),
        domain_tags: vec!["analysis".to_string()],
        task_description: "deep research on datasets".to_string(),
        tenant_id,
        workspace_id,
        crew_id: None,
        workflow_id: None,
    };

    let denied_decision = firewall
        .authorize_and_audit(&store, &capsule, &other_agent_context, None, false, &[])
        .await
        .expect("authorize_and_audit");

    assert!(!denied_decision.allowed);
    assert_eq!(denied_decision.reason, "capsule is agent-private");
    assert!(denied_decision.capsule.is_none());

    let audit_scope = ExecutionScope {
        tenant_id,
        workspace_id,
        run_id: Uuid::nil(),
    };
    let audits = store.list_audit_logs(&audit_scope).await.expect("audits");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "memory.cross_scope_denied");
    assert_eq!(audits[0].actor_id.as_deref(), Some("agent_beta"));
}

#[tokio::test]
async fn test_cross_tenant_and_cross_workspace_query_returns_nothing() {
    let temp_file = NamedTempFile::new().expect("temp file");
    let store = SqliteCheckpointStore::new(temp_file.path()).expect("store");

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let workspace_a = Uuid::new_v4();
    let workspace_b = Uuid::new_v4();

    let capsule = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_a.to_string(),
        workspace_id: workspace_a.to_string(),
        source_run_id: None,
        source_agent_id: "worker_1".to_string(),
        source_role: "coder".to_string(),
        source_domain_tags: vec!["rust".to_string()],
        trigger_preconditions: "compile error".to_string(),
        distilled_heuristic: "use cargo check".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.9,
        negative_transfer: false,
        created_at: "2026-11-15T22:00:00Z".to_string(),
    };

    store.insert_knowledge_capsule(&capsule).expect("insert");

    let list_b = store
        .list_knowledge_capsules(&tenant_b.to_string(), &workspace_b.to_string())
        .expect("list");
    assert!(list_b.is_empty());

    let firewall = ScopeFirewall::default();
    let target_b = AgentLearningContext {
        agent_id: "worker_2".to_string(),
        domain_tags: vec!["rust".to_string()],
        task_description: "compile rust code".to_string(),
        tenant_id: tenant_b,
        workspace_id: workspace_b,
        crew_id: None,
        workflow_id: None,
    };

    let decision = firewall
        .authorize_and_audit(&store, &capsule, &target_b, None, false, &[])
        .await
        .expect("authorize");
    assert!(!decision.allowed);
    assert_eq!(decision.reason, "tenant or workspace boundary mismatch");

    let audit_scope = ExecutionScope {
        tenant_id: tenant_b,
        workspace_id: workspace_b,
        run_id: Uuid::nil(),
    };
    let audits = store.list_audit_logs(&audit_scope).await.expect("audits");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].action, "memory.cross_scope_denied");
}

#[test]
fn test_propagation_scopes_crew_workflow_quarantine_verified() {
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let wf_id = Uuid::new_v4();
    let firewall = ScopeFirewall::default();

    let mut capsule = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        source_run_id: None,
        source_agent_id: "agent_1".to_string(),
        source_role: "coder".to_string(),
        source_domain_tags: vec!["rust".to_string()],
        trigger_preconditions: "compile error".to_string(),
        distilled_heuristic: "run cargo check with token sk-1234567890abcdef".to_string(),
        prohibited_contexts: vec!["hostile_agent".to_string()],
        propagation_scope: "CREW_LOCAL".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: Some(wf_id.to_string()),
        source_crew_id: Some("crew_alpha".to_string()),
        confidence: 0.9,
        negative_transfer: false,
        created_at: "2026-11-15T22:00:00Z".to_string(),
    };

    let target_diff_crew = AgentLearningContext {
        agent_id: "agent_2".to_string(),
        domain_tags: vec!["rust".to_string()],
        task_description: "compile rust".to_string(),
        tenant_id,
        workspace_id,
        crew_id: Some("crew_beta".to_string()),
        workflow_id: Some(wf_id),
    };
    let d1 = firewall.authorize(&capsule, &target_diff_crew, None, false, &[]);
    assert!(!d1.allowed);
    assert_eq!(d1.reason, "capsule is restricted to its source crew");

    let target_same_crew = AgentLearningContext {
        crew_id: Some("crew_alpha".to_string()),
        ..target_diff_crew.clone()
    };
    let d2 = firewall.authorize(&capsule, &target_same_crew, None, false, &[]);
    assert!(d2.allowed);
    let sanitized_heuristic = d2.capsule.unwrap().distilled_heuristic;
    assert!(!sanitized_heuristic.contains("sk-1234567890abcdef"));
    assert!(sanitized_heuristic.contains("[REDACTED]"));

    capsule.propagation_scope = "WORKFLOW_SCOPED".to_string();
    let target_diff_wf = AgentLearningContext {
        workflow_id: Some(Uuid::new_v4()),
        ..target_same_crew.clone()
    };
    let d3 = firewall.authorize(&capsule, &target_diff_wf, None, false, &[]);
    assert!(!d3.allowed);
    assert_eq!(d3.reason, "capsule is restricted to its source workflow");

    capsule.propagation_scope = "WORKSPACE_QUARANTINE".to_string();
    let d4_unapproved = firewall.authorize(&capsule, &target_same_crew, None, false, &[]);
    assert!(!d4_unapproved.allowed);
    assert!(d4_unapproved.requires_approval);

    let d4_approved = firewall.authorize(&capsule, &target_same_crew, None, true, &[]);
    assert!(d4_approved.allowed);

    let hostile_target = AgentLearningContext {
        agent_id: "hostile_agent".to_string(),
        ..target_same_crew.clone()
    };
    let d5 = firewall.authorize(&capsule, &hostile_target, None, false, &[]);
    assert!(!d5.allowed);
    assert_eq!(d5.reason, "target is explicitly prohibited");
}

#[test]
fn test_policy_filtering_code_patterns_and_heuristics() {
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let firewall = ScopeFirewall::default();

    let capsule_code = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.to_string(),
        workspace_id: workspace_id.to_string(),
        source_run_id: None,
        source_agent_id: "coder_1".to_string(),
        source_role: "coder".to_string(),
        source_domain_tags: vec!["python".to_string()],
        trigger_preconditions: "need loop".to_string(),
        distilled_heuristic: "```python\nfor x in range(10):\n    print(x)\n```".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.9,
        negative_transfer: false,
        created_at: "2026-11-15T22:00:00Z".to_string(),
    };

    let target = AgentLearningContext {
        agent_id: "coder_2".to_string(),
        domain_tags: vec!["python".to_string()],
        task_description: "write a python loop".to_string(),
        tenant_id,
        workspace_id,
        crew_id: None,
        workflow_id: None,
    };

    let disallow_code_policy = LearningPolicy {
        source_agent_id: "coder_1".to_string(),
        target_agent_id: "coder_2".to_string(),
        scope: PropagationScope::WorkspaceVerified,
        allow_heuristics: true,
        allow_code_patterns: false,
        require_human_approval: false,
        allowed: true,
    };

    let d_code_denied = firewall.authorize(
        &capsule_code,
        &target,
        Some(&disallow_code_policy),
        false,
        &[],
    );
    assert!(!d_code_denied.allowed);
    assert_eq!(d_code_denied.reason, "policy disallows code patterns");

    let allow_code_policy = LearningPolicy {
        allow_code_patterns: true,
        ..disallow_code_policy.clone()
    };
    let d_code_allowed =
        firewall.authorize(&capsule_code, &target, Some(&allow_code_policy), false, &[]);
    assert!(d_code_allowed.allowed);
}
