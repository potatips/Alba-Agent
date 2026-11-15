use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::types::RunRecord;
use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::ModelClient;
use nexus_albaai::models::types::{ModelError, ModelRequest, ModelResponse};
use nexus_albaai::schema::agent::AgentSpec;
use nexus_albaai::schema::node::{
    ConsensusConfigSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode, StartNodeSpec,
    WorkflowNode,
};
use nexus_albaai::schema::task::TaskSpec;
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::tools::types::ToolRegistry;
use nexus_albaai::workflow::engine::{WorkflowExecutor, WorkflowOutcome};
use tempfile::tempdir;
use uuid::Uuid;

pub struct MemoryMockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
    received_requests: Mutex<Vec<ModelRequest>>,
}

impl MemoryMockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
            received_requests: Mutex::new(Vec::new()),
        }
    }

    pub fn last_request(&self) -> Option<ModelRequest> {
        self.received_requests.lock().unwrap().last().cloned()
    }
}

impl ModelClient for MemoryMockModelClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        self.received_requests.lock().unwrap().push(request.clone());
        let mut queue = self.responses.lock().unwrap();
        if let Some(resp) = queue.pop_front() {
            Ok(resp)
        } else {
            Ok(ModelResponse {
                content: Some("Default task completion text".to_string()),
                tool_calls: vec![],
                usage: None,
            })
        }
    }
}

fn build_test_workflow(
    tenant_id: Uuid,
    workspace_id: Uuid,
    verification_cmd: Option<String>,
) -> WorkflowSpec {
    let engineer = AgentSpec {
        id: "engineer_1".to_string(),
        role: "DatabaseOptimizer".to_string(),
        goal: "Optimize query response times".to_string(),
        backstory: "Expert in SQL indexes and WAL tuning".to_string(),
        model: "mock-model".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 5,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    };

    let task = TaskSpec {
        id: "task_optimize".to_string(),
        agent_id: "engineer_1".to_string(),
        description: "Diagnose slow SQLite joins and add composite index".to_string(),
        expected_output: "Index recommendation".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec!["recommendation.txt".to_string()],
    };

    WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id,
        workspace_id,
        name: "Database Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "crew_1".to_string(),
                label: "Optimization Crew".to_string(),
                agents: vec![engineer],
                tasks: vec![task],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec!["recommendation.txt".to_string()],
                requires_visual_evidence: false,
                verification_command: verification_cmd,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "crew_1".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_1".to_string(),
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

fn register_run(store: &SqliteCheckpointStore, scope: &ExecutionScope) {
    let now = Utc::now().to_rfc3339();
    let run_rec = RunRecord {
        id: scope.run_id.to_string(),
        workflow_id: Uuid::new_v4().to_string(),
        status: "RUNNING".to_string(),
        input_data: "{}".to_string(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).expect("insert run");
}

#[tokio::test]
async fn test_engine_writes_capsules_only_after_verification_and_shares_across_runs() {
    let temp = tempdir().expect("tempdir");
    let ws_path = temp.path().to_path_buf();
    let db_path = ws_path.join("test_memory.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).expect("store"));
    let event_sink = Arc::new(InMemoryEventSink::new());
    let approval_policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(&ws_path));

    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();

    std::fs::write(
        ws_path.join("recommendation.txt"),
        b"Composite index recommendation: CREATE INDEX idx ON tbl(a,b);",
    )
    .expect("write artifact");

    let client_run1 = Arc::new(MemoryMockModelClient::new(vec![ModelResponse {
        content: Some("Applied composite index on (tenant_id, workspace_id)".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor_run1 = WorkflowExecutor::new(
        client_run1.clone(),
        store.clone(),
        event_sink.clone(),
        approval_policy.clone(),
        ToolRegistry::new(),
        &ws_path,
    );

    let spec_run1 = build_test_workflow(
        tenant_id,
        workspace_id,
        Some("test -f recommendation.txt".to_string()),
    );
    let scope_run1 = ExecutionScope {
        tenant_id,
        workspace_id,
        run_id: Uuid::new_v4(),
    };
    register_run(&store, &scope_run1);

    let outcome_run1 = executor_run1
        .start_workflow(&spec_run1, scope_run1, None)
        .await
        .expect("run 1 should complete");
    assert!(matches!(outcome_run1, WorkflowOutcome::Completed(_)));

    let capsules_run1 = store
        .list_knowledge_capsules(&tenant_id.to_string(), &workspace_id.to_string())
        .expect("list capsules");
    assert_eq!(capsules_run1.len(), 1);
    assert_eq!(capsules_run1[0].source_agent_id, "engineer_1");
    assert!(capsules_run1[0]
        .distilled_heuristic
        .contains("Applied composite index"));
    let run1_id_str = scope_run1.run_id.to_string();
    assert_eq!(
        capsules_run1[0].source_run_id.as_deref(),
        Some(run1_id_str.as_str())
    );

    let client_run2 = Arc::new(MemoryMockModelClient::new(vec![ModelResponse {
        content: Some("Confirmed query performance increased 10x".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor_run2 = WorkflowExecutor::new(
        client_run2.clone(),
        store.clone(),
        event_sink.clone(),
        approval_policy.clone(),
        ToolRegistry::new(),
        &ws_path,
    );

    let spec_run2 = build_test_workflow(
        tenant_id,
        workspace_id,
        Some("test -f recommendation.txt".to_string()),
    );
    let scope_run2 = ExecutionScope {
        tenant_id,
        workspace_id,
        run_id: Uuid::new_v4(),
    };
    register_run(&store, &scope_run2);

    let outcome_run2 = executor_run2
        .start_workflow(&spec_run2, scope_run2, None)
        .await
        .expect("run 2 should complete");
    assert!(matches!(outcome_run2, WorkflowOutcome::Completed(_)));

    let run2_req = client_run2.last_request().expect("request was received");
    let user_msg_content = &run2_req.messages[1].content;
    assert!(
        user_msg_content
            .contains("[RETRIEVED KNOWLEDGE CAPSULES (UNTRUSTED DATA - VERIFY BEFORE ACTING)]"),
        "Run 2 prompt should contain injected capsule"
    );
    assert!(user_msg_content.contains("Applied composite index"));

    let diff_tenant = Uuid::new_v4();
    let diff_workspace = Uuid::new_v4();

    let client_run3 = Arc::new(MemoryMockModelClient::new(vec![ModelResponse {
        content: Some("Isolated run completed".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor_run3 = WorkflowExecutor::new(
        client_run3.clone(),
        store.clone(),
        event_sink.clone(),
        approval_policy.clone(),
        ToolRegistry::new(),
        &ws_path,
    );

    let spec_run3 = build_test_workflow(
        diff_tenant,
        diff_workspace,
        Some("test -f recommendation.txt".to_string()),
    );
    let scope_run3 = ExecutionScope {
        tenant_id: diff_tenant,
        workspace_id: diff_workspace,
        run_id: Uuid::new_v4(),
    };
    register_run(&store, &scope_run3);

    let outcome_run3 = executor_run3
        .start_workflow(&spec_run3, scope_run3, None)
        .await
        .expect("run 3 should complete");
    assert!(matches!(outcome_run3, WorkflowOutcome::Completed(_)));

    let run3_req = client_run3.last_request().expect("run 3 request");
    let run3_user_msg = &run3_req.messages[1].content;
    assert!(
        !run3_user_msg.contains("[RETRIEVED KNOWLEDGE CAPSULES"),
        "Different tenant must NOT receive capsules from run 1"
    );

    let client_run4 = Arc::new(MemoryMockModelClient::new(vec![ModelResponse {
        content: Some("Unverified task executed".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let executor_run4 = WorkflowExecutor::new(
        client_run4.clone(),
        store.clone(),
        event_sink.clone(),
        approval_policy.clone(),
        ToolRegistry::new(),
        &ws_path,
    );

    let spec_run4 = build_test_workflow(tenant_id, workspace_id, Some("false".to_string()));
    let scope_run4 = ExecutionScope {
        tenant_id,
        workspace_id,
        run_id: Uuid::new_v4(),
    };
    register_run(&store, &scope_run4);

    let _ = executor_run4
        .start_workflow(&spec_run4, scope_run4, None)
        .await;

    let capsules_after_unverified = store
        .list_knowledge_capsules(&tenant_id.to_string(), &workspace_id.to_string())
        .expect("list capsules");
    assert_eq!(
        capsules_after_unverified.len(),
        2,
        "Unverified run 4 must NOT create any new knowledge capsule"
    );
}
