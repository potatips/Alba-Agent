use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::ModelClient;
use nexus_albaai::models::types::{ModelError, ModelRequest, ModelResponse};
use nexus_albaai::schema::agent::{AgentSpec, AgentTier};
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

pub struct S6MockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
    captured_requests: Mutex<Vec<ModelRequest>>,
}

impl S6MockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
            captured_requests: Mutex::new(Vec::new()),
        }
    }
}

impl ModelClient for S6MockModelClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        self.captured_requests.lock().unwrap().push(request.clone());
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

#[tokio::test]
async fn test_three_agent_hierarchical_crew_execution() {
    let responses = vec![
        ModelResponse {
            content: Some(
                "Manager Plan: Frontend builds telemetry charts; Backend exposes /api/v1/telemetry"
                    .to_string(),
            ),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("Frontend Dashboard UI implementation completed".to_string()),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("Backend Telemetry Axum API implementation completed".to_string()),
            tool_calls: vec![],
            usage: None,
        },
    ];

    let model = Arc::new(S6MockModelClient::new(responses));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("hierarchical_crew.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();
    let ws_dir = tempdir().unwrap();

    let executor = WorkflowExecutor::new(
        model.clone(),
        store,
        events,
        policy,
        registry,
        ws_dir.path(),
    );

    let manager = AgentSpec {
        id: "agent_lead".to_string(),
        role: "Engineering Lead".to_string(),
        goal: "Coordinate full-stack delivery".to_string(),
        backstory: "Experienced tech lead".to_string(),
        model: "lead-model".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 3,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: Some(AgentTier::Lead),
    };

    let frontend = AgentSpec {
        id: "agent_fe".to_string(),
        role: "Frontend Specialist".to_string(),
        goal: "Build Next.js UI components".to_string(),
        backstory: "React and Tailwind expert".to_string(),
        model: "fe-model".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 3,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: Some(AgentTier::Specialist),
    };

    let backend = AgentSpec {
        id: "agent_be".to_string(),
        role: "Backend Specialist".to_string(),
        goal: "Build Axum Rust services".to_string(),
        backstory: "Systems engineer".to_string(),
        model: "be-model".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 3,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: Some(AgentTier::Specialist),
    };

    let task_fe = TaskSpec {
        id: "task_fe".to_string(),
        agent_id: "agent_fe".to_string(),
        description: "Develop Dashboard telemetry charts".to_string(),
        expected_output: "Component tsx code".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    let task_be = TaskSpec {
        id: "task_be".to_string(),
        agent_id: "agent_be".to_string(),
        description: "Develop Axum telemetry route handler".to_string(),
        expected_output: "Rust handler code".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    let spec = WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Hierarchical Fullstack Team".to_string(),
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
                id: "crew_fullstack".to_string(),
                label: "Fullstack Team".to_string(),
                agents: vec![manager, frontend, backend],
                tasks: vec![task_fe, task_be],
                process: CrewProcess::Hierarchical,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: Some("agent_lead".to_string()),
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
                target: "crew_fullstack".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_fullstack".to_string(),
                target: "end".to_string(),
                route: None,
                loop_back: false,
            },
        ],
        learning_edges: vec![],
        layout: BTreeMap::new(),
        modes: WorkflowModesSpec::default(),
    };

    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();
    let outcome = executor
        .start_workflow(&spec, scope, None)
        .await
        .expect("Hierarchical crew must succeed");

    match outcome {
        WorkflowOutcome::Completed(val) => {
            assert_eq!(
                val["task_fe"],
                "Frontend Dashboard UI implementation completed"
            );
            assert_eq!(
                val["task_be"],
                "Backend Telemetry Axum API implementation completed"
            );
        }
        _ => panic!("Expected Completed outcome, got {:?}", outcome),
    }

    let reqs = model.captured_requests.lock().unwrap();
    assert_eq!(reqs.len(), 3);

    assert_eq!(reqs[0].model_override.as_deref(), Some("lead-model"));
    assert_eq!(reqs[1].model_override.as_deref(), Some("fe-model"));
    assert_eq!(reqs[2].model_override.as_deref(), Some("be-model"));

    assert!(reqs[1]
        .messages
        .iter()
        .any(|m| m.content.contains("Manager Directives from agent_lead")));
    assert!(reqs[2]
        .messages
        .iter()
        .any(|m| m.content.contains("Manager Directives from agent_lead")));
}
