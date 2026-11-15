use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::domain::{ExecutionScope, ToolCall};
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
use nexus_albaai::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError, ToolRegistry};
use nexus_albaai::workflow::engine::{WorkflowExecutionError, WorkflowExecutor, WorkflowOutcome};
use tempfile::tempdir;
use uuid::Uuid;

pub struct S4MockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
}

impl S4MockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
        }
    }
}

impl ModelClient for S4MockModelClient {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let mut queue = self.responses.lock().unwrap();
        if let Some(resp) = queue.pop_front() {
            Ok(resp)
        } else {
            Ok(ModelResponse {
                content: Some("Default response".to_string()),
                tool_calls: vec![],
                usage: None,
            })
        }
    }
}

pub struct CountingTool {
    call_count: Arc<AtomicUsize>,
}

impl CountingTool {
    pub fn new(call_count: Arc<AtomicUsize>) -> Self {
        Self { call_count }
    }
}

impl Tool for CountingTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "counting_tool".to_string(),
            description: "Counts execution invocations".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "param": { "type": "string" }
                }
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _scope: &'a ExecutionScope,
        _arguments: &'a serde_json::Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok("counted_ok".to_string())
        })
    }
}

fn create_tool_crew_spec() -> WorkflowSpec {
    let agent = AgentSpec {
        id: "agent_tool_runner".to_string(),
        role: "Tool Runner".to_string(),
        goal: "Invoke tool and return result".to_string(),
        backstory: "Precise agent that calls tools".to_string(),
        model: "mock-model".to_string(),
        tool_ids: vec!["counting_tool".to_string()],
        skill_ids: vec![],
        max_iterations: 5,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    };

    let task = TaskSpec {
        id: "task_tool".to_string(),
        agent_id: "agent_tool_runner".to_string(),
        description: "Call counting tool".to_string(),
        expected_output: "Counted result".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Idempotent Tool Workflow".to_string(),
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
                id: "crew_tool".to_string(),
                label: "Crew Tool".to_string(),
                agents: vec![agent],
                tasks: vec![task],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
                verification_command: None,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "crew_tool".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_tool".to_string(),
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
async fn test_tool_journal_idempotency_avoids_reexecution() {
    let call_count = Arc::new(AtomicUsize::new(0));

    let model = Arc::new(S4MockModelClient::new(vec![
        ModelResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_abc_123".to_string(),
                name: "counting_tool".to_string(),
                arguments: serde_json::json!({ "param": "test" }),
            }],
            usage: None,
        },
        ModelResponse {
            content: Some("Final answer after tool".to_string()),
            tool_calls: vec![],
            usage: None,
        },
    ]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("workflow_s4_idempotent.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(CountingTool::new(call_count.clone())));

    let ws_dir = tempdir().unwrap();
    let executor = WorkflowExecutor::new(
        model.clone(),
        store.clone(),
        events.clone(),
        policy,
        registry.clone(),
        ws_dir.path(),
    );

    let spec = create_tool_crew_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    store
        .record_tool_call(
            &scope,
            "crew_tool",
            "call_abc_123",
            "counting_tool",
            &serde_json::json!({ "param": "test" }),
        )
        .await
        .unwrap();

    store
        .complete_tool_call(&scope, "crew_tool", "call_abc_123", "counted_ok")
        .await
        .unwrap();

    let outcome = executor
        .start_workflow(&spec, scope, None)
        .await
        .expect("First run must succeed");

    assert_eq!(call_count.load(Ordering::SeqCst), 0);
    match outcome {
        WorkflowOutcome::Completed(val) => {
            assert_eq!(val["task_tool"], "Final answer after tool");
        }
        _ => panic!("Expected Completed"),
    }

    let status = store
        .get_tool_call_status(&scope, "crew_tool", "call_abc_123")
        .await
        .unwrap()
        .expect("Tool call journal entry must exist");

    assert_eq!(status.0, "consumed");
    assert_eq!(status.1.as_deref(), Some("counted_ok"));
}

#[tokio::test]
async fn test_tool_interrupted_executing_fails_closed_on_resume() {
    let call_count = Arc::new(AtomicUsize::new(0));

    let model = Arc::new(S4MockModelClient::new(vec![ModelResponse {
        content: None,
        tool_calls: vec![ToolCall {
            id: "call_interrupted_456".to_string(),
            name: "counting_tool".to_string(),
            arguments: serde_json::json!({ "param": "crash_test" }),
        }],
        usage: None,
    }]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("workflow_s4_crash.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(CountingTool::new(call_count.clone())));

    let ws_dir = tempdir().unwrap();
    let executor = WorkflowExecutor::new(
        model.clone(),
        store.clone(),
        events.clone(),
        policy,
        registry,
        ws_dir.path(),
    );

    let spec = create_tool_crew_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    store
        .record_tool_call(
            &scope,
            "crew_tool",
            "call_interrupted_456",
            "counting_tool",
            &serde_json::json!({ "param": "crash_test" }),
        )
        .await
        .unwrap();

    let err = executor
        .start_workflow(&spec, scope, None)
        .await
        .unwrap_err();

    match err {
        WorkflowExecutionError::ExecutionFailed(msg) => {
            assert!(msg.contains("was interrupted mid-execution (crashed), failing closed"));
        }
        _ => panic!("Expected ExecutionFailed fail-closed error, got {:?}", err),
    }

    assert_eq!(call_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_sqlite_wal_integrity_after_clean_close() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("clean_close_test.db");

    {
        let store = SqliteCheckpointStore::new(&db_path).unwrap();
        let scope = ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap();

        store
            .record_tool_call(
                &scope,
                "node_1",
                "tc_1",
                "tool_x",
                &serde_json::json!({ "key": "val" }),
            )
            .await
            .unwrap();

        store
            .complete_tool_call(&scope, "node_1", "tc_1", "output_val")
            .await
            .unwrap();
    }

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let integrity: String = conn
        .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");

    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");
}
