use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::sync::{Arc, Mutex};

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::domain::{ExecutionScope, RunStatus};
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::ModelClient;
use nexus_albaai::models::types::{ModelError, ModelRequest, ModelResponse};
use nexus_albaai::schema::{
    AgentSpec, ConsensusConfigSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode,
    StartNodeSpec, TaskSpec, WorkflowEdgeSpec, WorkflowModesSpec, WorkflowNode, WorkflowSpec,
};
use nexus_albaai::tools::types::ToolRegistry;
use nexus_albaai::verification::claims::{ClaimError, ClaimsHook};
use nexus_albaai::workflow::engine::{WorkflowExecutionError, WorkflowExecutor, WorkflowOutcome};
use tempfile::tempdir;
use uuid::Uuid;

pub struct MockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
}

impl MockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
        }
    }
}

impl ModelClient for MockModelClient {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let mut queue = self.responses.lock().unwrap();
        if let Some(resp) = queue.pop_front() {
            Ok(resp)
        } else {
            Ok(ModelResponse {
                content: Some("Default completion text".to_string()),
                tool_calls: vec![],
                usage: None,
            })
        }
    }
}

fn create_single_agent_workflow_spec() -> WorkflowSpec {
    let agent = AgentSpec {
        id: "writer_agent".to_string(),
        role: "Technical Writer".to_string(),
        goal: "Generate reports and code".to_string(),
        backstory: "Experienced technical writer producing deliverables".to_string(),
        model: "mock-writer".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 5,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    };

    let task = TaskSpec {
        id: "task_write_report".to_string(),
        description: "Write deliverable report".to_string(),
        expected_output: "Confirmed written report".to_string(),
        agent_id: "writer_agent".to_string(),
        context_task_ids: vec![],
        output_schema: "text".to_string(),
        output_schema_json: None,
        expected_artifact_paths: vec![],
    };

    let crew = CrewNodeSpec {
        id: "crew_node".to_string(),
        label: "Report Crew".to_string(),
        agents: vec![agent],
        tasks: vec![task],
        process: CrewProcess::Sequential,
        consensus_config: ConsensusConfigSpec::default(),
        manager_agent_id: None,
        join: JoinMode::Any,
        expected_artifact_paths: vec![],
        requires_visual_evidence: false,
    };

    WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Claims Test Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::Crew(crew),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "crew_node".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_node".to_string(),
                target: "end".to_string(),
                route: None,
                loop_back: false,
            },
        ],
        learning_edges: vec![],
        layout: BTreeMap::new(),
        modes: WorkflowModesSpec::default(),
    }
}

#[tokio::test]
async fn test_claimed_missing_file_fails_node_and_run() {
    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("claims_missing.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();

    let model = Arc::new(MockModelClient::new(vec![ModelResponse {
        content: Some(
            "I have finished the analysis and created file: outputs/analysis.csv with the summary."
                .to_string(),
        ),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor = WorkflowExecutor::new(
        model,
        store.clone(),
        events,
        policy,
        registry,
        ws_dir.path(),
    );

    let spec = create_single_agent_workflow_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let result = executor.start_workflow(&spec, scope, None).await;

    match result {
        Err(WorkflowExecutionError::ExecutionFailed(msg)) => {
            assert!(
                msg.contains("outputs/analysis.csv") || msg.contains("does not exist on disk"),
                "Error message was: {}",
                msg
            );
        }
        _ => panic!(
            "Expected ExecutionFailed for missing claimed file, got: {:?}",
            result
        ),
    }

    let latest = store
        .get_latest_workflow_step(&scope)
        .await
        .unwrap()
        .expect("Latest step must exist in SQLite");

    assert_eq!(latest.status, RunStatus::Failed);
    assert_eq!(latest.node_id, "crew_node");

    let state_json = &latest.state_json;
    let node_state = state_json["node_states"]["crew_node"].as_str().unwrap();
    assert_eq!(node_state, "failed");

    let err_str = state_json["error"].as_str().unwrap();
    assert!(err_str.contains("outputs/analysis.csv"));
}

#[tokio::test]
async fn test_claimed_present_file_passes_run() {
    let ws_dir = tempdir().unwrap();
    let outputs_dir = ws_dir.path().join("outputs");
    fs::create_dir_all(&outputs_dir).unwrap();
    fs::write(outputs_dir.join("valid.json"), r#"{"status": "ok"}"#).unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("claims_present.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();

    let model = Arc::new(MockModelClient::new(vec![ModelResponse {
        content: Some("Created file: outputs/valid.json with complete details.".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor = WorkflowExecutor::new(
        model,
        store.clone(),
        events,
        policy,
        registry,
        ws_dir.path(),
    );

    let spec = create_single_agent_workflow_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let outcome = executor
        .start_workflow(&spec, scope, None)
        .await
        .expect("Workflow must succeed when claimed file exists");

    match outcome {
        WorkflowOutcome::Completed(_) => {}
        _ => panic!("Expected Completed outcome, got: {:?}", outcome),
    }

    let latest = store
        .get_latest_workflow_step(&scope)
        .await
        .unwrap()
        .expect("Latest step must exist in SQLite");

    assert_eq!(latest.status, RunStatus::Completed);
    assert_eq!(latest.node_id, "end");
}

#[tokio::test]
async fn test_claimed_empty_file_fails_node() {
    let ws_dir = tempdir().unwrap();
    let outputs_dir = ws_dir.path().join("outputs");
    fs::create_dir_all(&outputs_dir).unwrap();
    fs::write(outputs_dir.join("empty.txt"), "").unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("claims_empty.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();

    let model = Arc::new(MockModelClient::new(vec![ModelResponse {
        content: Some("Wrote file outputs/empty.txt with summary.".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor = WorkflowExecutor::new(
        model,
        store.clone(),
        events,
        policy,
        registry,
        ws_dir.path(),
    );

    let spec = create_single_agent_workflow_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let result = executor.start_workflow(&spec, scope, None).await;

    match result {
        Err(WorkflowExecutionError::ExecutionFailed(msg)) => {
            assert!(
                msg.contains("empty (0 bytes)"),
                "Expected empty file error, got: {}",
                msg
            );
        }
        _ => panic!(
            "Expected failure for 0-byte claimed file, got: {:?}",
            result
        ),
    }

    let latest = store
        .get_latest_workflow_step(&scope)
        .await
        .unwrap()
        .expect("Latest step");

    assert_eq!(latest.status, RunStatus::Failed);
}

#[tokio::test]
async fn test_claimed_symlink_escape_fails_node() {
    let ws_dir = tempdir().unwrap();
    let outside_dir = tempdir().unwrap();
    let outside_secret = outside_dir.path().join("secret.txt");
    fs::write(&outside_secret, "sensitive outside data").unwrap();

    let link_path = ws_dir.path().join("escaped_link.txt");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_secret, &link_path).unwrap();

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("claims_symlink.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();

    let model = Arc::new(MockModelClient::new(vec![ModelResponse {
        content: Some("Produced escaped_link.txt with exported metrics.".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor = WorkflowExecutor::new(
        model,
        store.clone(),
        events,
        policy,
        registry,
        ws_dir.path(),
    );

    let spec = create_single_agent_workflow_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let result = executor.start_workflow(&spec, scope, None).await;

    match result {
        Err(WorkflowExecutionError::ExecutionFailed(msg)) => {
            assert!(
                msg.contains("escapes workspace"),
                "Expected symlink escape error, got: {}",
                msg
            );
        }
        _ => panic!("Expected failure for symlink escape, got: {:?}", result),
    }

    let latest = store
        .get_latest_workflow_step(&scope)
        .await
        .unwrap()
        .expect("Latest step");

    assert_eq!(latest.status, RunStatus::Failed);
}

#[tokio::test]
async fn test_json_structured_claims_fail_when_missing() {
    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("claims_json.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();

    let json_output = r#"{
        "summary": "Completed code generation",
        "deliverables": [
            "src/generated.rs",
            "docs/spec.md"
        ]
    }"#;

    let model = Arc::new(MockModelClient::new(vec![ModelResponse {
        content: Some(json_output.to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor = WorkflowExecutor::new(
        model,
        store.clone(),
        events,
        policy,
        registry,
        ws_dir.path(),
    );

    let spec = create_single_agent_workflow_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let result = executor.start_workflow(&spec, scope, None).await;
    assert!(result.is_err());

    let latest = store
        .get_latest_workflow_step(&scope)
        .await
        .unwrap()
        .expect("Latest step");

    assert_eq!(latest.status, RunStatus::Failed);
}

#[test]
fn test_claims_hook_standalone_extraction_and_verification() {
    let temp = tempdir().unwrap();
    let ws = temp.path();

    let hook = ClaimsHook::new();

    let text = "I wrote file `src/calculator.rs` and generated report at `docs/calc.md`";
    let extracted = hook.extract_claims(text);
    assert_eq!(extracted.len(), 2);
    assert!(extracted.contains(&"src/calculator.rs".to_string()));
    assert!(extracted.contains(&"docs/calc.md".to_string()));

    let missing_result = hook.verify_claims(ws, &extracted);
    assert!(matches!(
        missing_result,
        Err(ClaimError::MissingFile { .. })
    ));

    let src_dir = ws.join("src");
    let docs_dir = ws.join("docs");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&docs_dir).unwrap();

    fs::write(src_dir.join("calculator.rs"), "pub fn add() {}").unwrap();
    fs::write(docs_dir.join("calc.md"), "# Docs").unwrap();

    let ok_result = hook.verify_claims(ws, &extracted);
    assert!(ok_result.is_ok());

    let empty_file = ws.join("empty.txt");
    fs::write(&empty_file, "").unwrap();
    let empty_result = hook.verify_claims(ws, &["empty.txt".to_string()]);
    assert!(matches!(empty_result, Err(ClaimError::EmptyFile { .. })));

    let traversal_result = hook.verify_claims(ws, &["../outside.txt".to_string()]);
    assert!(matches!(
        traversal_result,
        Err(ClaimError::SymlinkEscape { .. })
    ));

    let no_claim_text = "The system version is 2.1.0 and everything is running properly.";
    let empty_claims = hook.extract_claims(no_claim_text);
    assert!(empty_claims.is_empty());
}
