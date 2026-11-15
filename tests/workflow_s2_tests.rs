use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::events::types::RuntimeEventType;
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

pub struct MockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
    received_requests: Mutex<Vec<ModelRequest>>,
}

impl MockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
            received_requests: Mutex::new(Vec::new()),
        }
    }
}

impl ModelClient for MockModelClient {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        self.received_requests.lock().unwrap().push(request.clone());
        let mut queue = self.responses.lock().unwrap();
        if let Some(resp) = queue.pop_front() {
            Ok(resp)
        } else {
            Ok(ModelResponse {
                content: Some("Default mock response".to_string()),
                tool_calls: vec![],
                usage: None,
            })
        }
    }
}

fn create_two_agent_crew_spec() -> WorkflowSpec {
    let researcher = AgentSpec {
        id: "agent_researcher".to_string(),
        role: "Researcher".to_string(),
        goal: "Gather telemetry data".to_string(),
        backstory: "Curious and thorough data investigator".to_string(),
        model: "mock-researcher".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 5,
        temperature: 0.1,
        guardrails: vec!["Do not fabricate facts".to_string()],
        knowledge_capsule_ids: vec![],
        tier: None,
    };

    let writer = AgentSpec {
        id: "agent_writer".to_string(),
        role: "Writer".to_string(),
        goal: "Synthesize insights into clean report".to_string(),
        backstory: "Concise and factual technical writer".to_string(),
        model: "mock-writer".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 5,
        temperature: 0.2,
        guardrails: vec!["Keep report under 100 words".to_string()],
        knowledge_capsule_ids: vec![],
        tier: None,
    };

    let task_research = TaskSpec {
        id: "task_research".to_string(),
        agent_id: "agent_researcher".to_string(),
        description: "Analyze system metric logs".to_string(),
        expected_output: "Key observations identified".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    let task_synthesis = TaskSpec {
        id: "task_synthesis".to_string(),
        agent_id: "agent_writer".to_string(),
        description: "Draft final synthesis summary".to_string(),
        expected_output: "Structured findings summary".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec!["task_research".to_string()],
        expected_artifact_paths: vec![],
    };

    WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Two Agent Linear Crew".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start Execution".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "crew_node".to_string(),
                label: "Analysis Crew".to_string(),
                agents: vec![researcher, writer],
                tasks: vec![task_research, task_synthesis],
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
                label: "End Execution".to_string(),
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
async fn test_linear_crew_two_agents_end_to_end() {
    let mock_responses = vec![
        ModelResponse {
            content: Some("Metric anomalies: memory peaked at 42MB".to_string()),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("Final executive summary: RAM anomaly resolved at 42MB".to_string()),
            tool_calls: vec![],
            usage: None,
        },
    ];

    let model = Arc::new(MockModelClient::new(mock_responses));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("workflow_s2.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();
    let ws_dir = tempdir().unwrap();

    let executor = WorkflowExecutor::new(
        model.clone(),
        store.clone(),
        events.clone(),
        policy,
        registry,
        ws_dir.path(),
    );

    let spec = create_two_agent_crew_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let outcome = executor
        .start_workflow(&spec, scope, None)
        .await
        .expect("Workflow execution must succeed");

    match outcome {
        WorkflowOutcome::Completed(val) => {
            assert_eq!(
                val["task_research"],
                "Metric anomalies: memory peaked at 42MB"
            );
            assert_eq!(
                val["task_synthesis"],
                "Final executive summary: RAM anomaly resolved at 42MB"
            );
        }
        _ => panic!("Expected Completed outcome, got {:?}", outcome),
    }

    let steps = store
        .list_workflow_steps(&scope)
        .await
        .expect("Must list checkpoints");

    assert!(
        steps.len() >= 4,
        "Expected at least 4 step checkpoints, got {}",
        steps.len()
    );

    assert_eq!(steps[0].sequence, 1);
    assert_eq!(steps[0].node_id, "start");

    let final_step = steps.last().unwrap();
    assert_eq!(final_step.node_id, "end");

    let emitted = events.events();
    let state_changes: Vec<_> = emitted
        .iter()
        .filter(|e| e.event_type == RuntimeEventType::FlowStateChanged)
        .collect();
    assert!(
        state_changes.len() >= 2,
        "Expected FlowStateChanged events, got {}",
        state_changes.len()
    );

    let completed_events: Vec<_> = emitted
        .iter()
        .filter(|e| e.event_type == RuntimeEventType::RunCompleted)
        .collect();
    assert_eq!(completed_events.len(), 1);

    let requests = model.received_requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1]
        .messages
        .iter()
        .any(|m| m.content.contains("memory peaked at 42MB")));
}
