use chrono::Utc;
use nexus_albaai::checkpoints::store::CheckpointError;
use nexus_albaai::checkpoints::types::KnowledgeCapsuleRecord;
use nexus_albaai::checkpoints::SqliteCheckpointStore;
use nexus_albaai::schema::capsule::KnowledgeCapsuleSpec;
use nexus_albaai::schema::workflow::{PropagationScope, SensitivityLevel};
use rusqlite::Connection;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_knowledge_capsule_round_trip_exact_values() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("capsule_roundtrip.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let capsule_id = Uuid::new_v4().to_string();
    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();
    let source_run_id = Some(Uuid::new_v4().to_string());
    let source_workflow_id = Some(Uuid::new_v4().to_string());
    let now = Utc::now().to_rfc3339();

    let record = KnowledgeCapsuleRecord {
        id: capsule_id.clone(),
        tenant_id: tenant_id.clone(),
        workspace_id: workspace_id.clone(),
        source_run_id: source_run_id.clone(),
        source_agent_id: "senior-db-analyst".to_string(),
        source_role: "Database Performance Engineer".to_string(),
        source_domain_tags: vec![
            "sql".to_string(),
            "sqlite".to_string(),
            "indexing".to_string(),
            "wal".to_string(),
        ],
        trigger_preconditions: "table scan observed on large table without covering index"
            .to_string(),
        distilled_heuristic: "create covering index on (tenant_id, workspace_id, created_at)"
            .to_string(),
        prohibited_contexts: vec![
            "in_memory_only".to_string(),
            "temporary_scratch_table".to_string(),
        ],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: source_workflow_id.clone(),
        source_crew_id: Some("crew_perf_01".to_string()),
        confidence: 0.945,
        negative_transfer: false,
        created_at: now.clone(),
    };

    store.insert_knowledge_capsule(&record).unwrap();

    let read_opt = store.get_knowledge_capsule(&capsule_id).unwrap();
    assert!(read_opt.is_some());
    let read = read_opt.unwrap();

    assert_eq!(read.id, record.id);
    assert_eq!(read.tenant_id, record.tenant_id);
    assert_eq!(read.workspace_id, record.workspace_id);
    assert_eq!(read.source_run_id, record.source_run_id);
    assert_eq!(read.source_agent_id, record.source_agent_id);
    assert_eq!(read.source_role, record.source_role);
    assert_eq!(read.source_domain_tags, record.source_domain_tags);
    assert_eq!(read.trigger_preconditions, record.trigger_preconditions);
    assert_eq!(read.distilled_heuristic, record.distilled_heuristic);
    assert_eq!(read.prohibited_contexts, record.prohibited_contexts);
    assert_eq!(read.propagation_scope, record.propagation_scope);
    assert_eq!(read.sensitivity_level, record.sensitivity_level);
    assert_eq!(read.source_workflow_id, record.source_workflow_id);
    assert_eq!(read.source_crew_id, record.source_crew_id);
    assert!((read.confidence - record.confidence).abs() < 1e-6);
    assert_eq!(read.negative_transfer, record.negative_transfer);
    assert_eq!(read.created_at, record.created_at);
}

#[tokio::test]
async fn test_knowledge_capsule_from_spec_conversion() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("capsule_spec_conv.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let capsule_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let workflow_id = Uuid::new_v4();
    let run_id = Uuid::new_v4().to_string();

    let spec = KnowledgeCapsuleSpec {
        capsule_id,
        tenant_id,
        workspace_id,
        source_agent_id: "agent_optimizer".to_string(),
        source_role: "Optimizer".to_string(),
        source_domain_tags: vec!["caching".to_string(), "redis".to_string()],
        trigger_preconditions: "cache miss rate exceeds 20 percent".to_string(),
        distilled_heuristic: "use write-through caching pattern with 5m TTL".to_string(),
        prohibited_contexts: vec!["write_heavy_burst".to_string()],
        propagation_scope: PropagationScope::WorkflowScoped,
        sensitivity_level: SensitivityLevel::ConfidentialPiiStripped,
        source_workflow_id: Some(workflow_id),
        source_crew_id: Some("crew_cache".to_string()),
        confidence: 0.88,
    };

    let record = KnowledgeCapsuleRecord::from_spec(&spec, Some(run_id.clone()));
    assert_eq!(record.id, capsule_id.to_string());
    assert_eq!(record.tenant_id, tenant_id.to_string());
    assert_eq!(record.workspace_id, workspace_id.to_string());
    assert_eq!(record.source_run_id, Some(run_id));
    assert_eq!(record.propagation_scope, "WORKFLOW_SCOPED");
    assert_eq!(record.sensitivity_level, "CONFIDENTIAL_PII_STRIPPED");

    store.insert_knowledge_capsule(&record).unwrap();
    let loaded = store
        .get_knowledge_capsule(&capsule_id.to_string())
        .unwrap()
        .unwrap();

    assert_eq!(loaded.id, capsule_id.to_string());
    assert_eq!(loaded.distilled_heuristic, spec.distilled_heuristic);
    assert!(!loaded.negative_transfer);
}

#[tokio::test]
async fn test_sqlite_migration_legacy_capsules_table() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("legacy_capsules.db");

    {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );
            INSERT INTO schema_migrations (version, applied_at) VALUES (5, datetime('now'));

            CREATE TABLE capsules (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                trigger_pattern TEXT NOT NULL,
                advice TEXT NOT NULL,
                scope TEXT NOT NULL,
                sensitivity TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            INSERT INTO capsules (id, title, trigger_pattern, advice, scope, sensitivity, created_at)
            VALUES ('old-1', 'Legacy Rule', 'pattern', 'advice', 'workspace', 'internal', datetime('now'));
            ",
        )
        .unwrap();
    }

    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let legacy = store.list_capsules().unwrap();
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy[0].id, "old-1");

    let new_capsule_id = Uuid::new_v4().to_string();
    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();

    let new_record = KnowledgeCapsuleRecord {
        id: new_capsule_id.clone(),
        tenant_id: tenant_id.clone(),
        workspace_id: workspace_id.clone(),
        source_run_id: None,
        source_agent_id: "agent_migration".to_string(),
        source_role: "Migrator".to_string(),
        source_domain_tags: vec!["migration".to_string()],
        trigger_preconditions: "schema version upgraded".to_string(),
        distilled_heuristic: "use schema migration 10".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 1.0,
        negative_transfer: false,
        created_at: Utc::now().to_rfc3339(),
    };

    store.insert_knowledge_capsule(&new_record).unwrap();
    let read_back = store
        .get_knowledge_capsule(&new_capsule_id)
        .unwrap()
        .unwrap();
    assert_eq!(read_back.id, new_capsule_id);
    assert_eq!(read_back.tenant_id, tenant_id);
    assert_eq!(read_back.workspace_id, workspace_id);
}

#[tokio::test]
async fn test_negative_transfer_flag_update() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("neg_transfer.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let capsule_id = Uuid::new_v4().to_string();
    let tenant_id = Uuid::new_v4().to_string();
    let workspace_id = Uuid::new_v4().to_string();

    let record = KnowledgeCapsuleRecord {
        id: capsule_id.clone(),
        tenant_id: tenant_id.clone(),
        workspace_id: workspace_id.clone(),
        source_run_id: None,
        source_agent_id: "agent_test".to_string(),
        source_role: "Tester".to_string(),
        source_domain_tags: vec!["test".to_string()],
        trigger_preconditions: "test precondition".to_string(),
        distilled_heuristic: "test heuristic".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "CREW_LOCAL".to_string(),
        sensitivity_level: "PUBLIC".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.5,
        negative_transfer: false,
        created_at: Utc::now().to_rfc3339(),
    };

    store.insert_knowledge_capsule(&record).unwrap();
    let initial = store.get_knowledge_capsule(&capsule_id).unwrap().unwrap();
    assert!(!initial.negative_transfer);

    store
        .set_knowledge_capsule_negative_transfer(&capsule_id, true)
        .unwrap();
    let updated = store.get_knowledge_capsule(&capsule_id).unwrap().unwrap();
    assert!(updated.negative_transfer);

    let nonexistent_id = Uuid::new_v4().to_string();
    let err = store
        .set_knowledge_capsule_negative_transfer(&nonexistent_id, true)
        .unwrap_err();
    assert!(matches!(err, CheckpointError::NotFound(_)));
}

#[tokio::test]
async fn test_list_knowledge_capsules_by_tenant_and_workspace() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("list_capsules.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let tenant_a = Uuid::new_v4().to_string();
    let workspace_a = Uuid::new_v4().to_string();

    let tenant_b = Uuid::new_v4().to_string();
    let workspace_b = Uuid::new_v4().to_string();

    let mut cap_a1 = KnowledgeCapsuleRecord {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_a.clone(),
        workspace_id: workspace_a.clone(),
        source_run_id: None,
        source_agent_id: "agent_a".to_string(),
        source_role: "RoleA".to_string(),
        source_domain_tags: vec!["tagA1".to_string()],
        trigger_preconditions: "prec A1".to_string(),
        distilled_heuristic: "heur A1".to_string(),
        prohibited_contexts: vec![],
        propagation_scope: "WORKSPACE_VERIFIED".to_string(),
        sensitivity_level: "INTERNAL".to_string(),
        source_workflow_id: None,
        source_crew_id: None,
        confidence: 0.9,
        negative_transfer: false,
        created_at: "2026-11-15T10:00:00Z".to_string(),
    };
    store.insert_knowledge_capsule(&cap_a1).unwrap();

    cap_a1.id = Uuid::new_v4().to_string();
    cap_a1.created_at = "2026-11-15T11:00:00Z".to_string();
    store.insert_knowledge_capsule(&cap_a1).unwrap();

    let mut cap_b1 = cap_a1.clone();
    cap_b1.id = Uuid::new_v4().to_string();
    cap_b1.tenant_id = tenant_b.clone();
    cap_b1.workspace_id = workspace_b.clone();
    cap_b1.created_at = "2026-11-15T12:00:00Z".to_string();
    store.insert_knowledge_capsule(&cap_b1).unwrap();

    let list_a = store
        .list_knowledge_capsules(&tenant_a, &workspace_a)
        .unwrap();
    assert_eq!(list_a.len(), 2);

    let list_b = store
        .list_knowledge_capsules(&tenant_b, &workspace_b)
        .unwrap();
    assert_eq!(list_b.len(), 1);

    let other_tenant = Uuid::new_v4().to_string();
    let other_workspace = Uuid::new_v4().to_string();
    let list_other = store
        .list_knowledge_capsules(&other_tenant, &other_workspace)
        .unwrap();
    assert_eq!(list_other.len(), 0);
}
