use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::domain::{ApprovalDecision, ExecutionScope};
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::ModelClient;
use nexus_albaai::models::types::{ModelError, ModelRequest, ModelResponse};
use nexus_albaai::schema::agent::AgentSpec;
use nexus_albaai::schema::node::{
    ApprovalGateNodeSpec, ConsensusConfigSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode,
    StartNodeSpec, WorkflowNode,
};
use nexus_albaai::schema::task::TaskSpec;
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::tools::types::ToolRegistry;
use nexus_albaai::workflow::engine::{WorkflowExecutor, WorkflowOutcome};
use tempfile::tempdir;
use uuid::Uuid;

pub struct S7MockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
}

impl S7MockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
        }
    }
}

impl ModelClient for S7MockModelClient {
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

fn create_fork_workflow_spec() -> WorkflowSpec {
    let agent = AgentSpec {
        id: "agent_worker".to_string(),
        role: "Worker".to_string(),
        goal: "Execute deployment task".to_string(),
        backstory: "Task worker".to_string(),
        model: "mock".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 3,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    };

    let task = TaskSpec {
        id: "task_1".to_string(),
        agent_id: "agent_worker".to_string(),
        description: "Deploy artifact".to_string(),
        expected_output: "Success".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Forkable Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::Approval(ApprovalGateNodeSpec {
                id: "gate".to_string(),
                label: "Review Gate".to_string(),
                action_description: "Approve deployment".to_string(),
                required_role: "admin".to_string(),
                timeout_seconds: 3600,
                boundary_rules: vec![],
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "crew_worker".to_string(),
                label: "Worker Crew".to_string(),
                agents: vec![agent],
                tasks: vec![task],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "gate".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "gate".to_string(),
                target: "crew_worker".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_worker".to_string(),
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
async fn test_workflow_fork_preserves_parent_immutability_and_diverges() {
    let model = Arc::new(S7MockModelClient::new(vec![ModelResponse {
        content: Some("Deployment succeeded on original run".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("fork_test.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();
    let ws_dir = tempdir().unwrap();

    let executor = WorkflowExecutor::new(
        model,
        store.clone(),
        events,
        policy,
        registry,
        ws_dir.path(),
    )
    .with_author_id("dev_author");

    let spec = create_fork_workflow_spec();
    let parent_scope =
        ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let outcome = executor
        .start_workflow(&spec, parent_scope, None)
        .await
        .expect("Start workflow must pause at gate");

    let approval_id = match outcome {
        WorkflowOutcome::AwaitingApproval { approval_id, .. } => approval_id,
        _ => panic!("Expected AwaitingApproval, got {:?}", outcome),
    };

    let parent_steps_before_fork = store.list_workflow_steps(&parent_scope).await.unwrap();
    assert_eq!(parent_steps_before_fork.len(), 3);

    let parent_seq1_snapshot = parent_steps_before_fork[0].clone();
    let parent_seq2_snapshot = parent_steps_before_fork[1].clone();
    let parent_seq3_snapshot = parent_steps_before_fork[2].clone();

    let forked_scope = executor
        .fork_workflow(&parent_scope, 3, "experiment_branch")
        .await
        .expect("Fork workflow at sequence 3 must succeed");

    assert_ne!(forked_scope.run_id, parent_scope.run_id);

    store
        .resolve_approval(
            &parent_scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "lead_approver",
        )
        .await
        .unwrap();

    let parent_final_outcome = executor
        .resume_workflow(&spec, parent_scope)
        .await
        .expect("Parent resume must succeed");

    assert!(matches!(
        parent_final_outcome,
        WorkflowOutcome::Completed(_)
    ));

    let parent_steps_final = store.list_workflow_steps(&parent_scope).await.unwrap();
    assert_eq!(parent_steps_final[0].id, parent_seq1_snapshot.id);
    assert_eq!(
        parent_steps_final[0].sequence,
        parent_seq1_snapshot.sequence
    );
    assert_eq!(parent_steps_final[0].node_id, parent_seq1_snapshot.node_id);
    assert_eq!(
        parent_steps_final[0].state_json,
        parent_seq1_snapshot.state_json
    );

    assert_eq!(parent_steps_final[1].id, parent_seq2_snapshot.id);
    assert_eq!(
        parent_steps_final[1].sequence,
        parent_seq2_snapshot.sequence
    );
    assert_eq!(parent_steps_final[1].node_id, parent_seq2_snapshot.node_id);
    assert_eq!(
        parent_steps_final[1].state_json,
        parent_seq2_snapshot.state_json
    );

    assert_eq!(parent_steps_final[2].id, parent_seq3_snapshot.id);
    assert_eq!(
        parent_steps_final[2].sequence,
        parent_seq3_snapshot.sequence
    );
    assert_eq!(parent_steps_final[2].node_id, parent_seq3_snapshot.node_id);
    assert_eq!(
        parent_steps_final[2].state_json,
        parent_seq3_snapshot.state_json
    );

    let forked_steps = store.list_workflow_steps(&forked_scope).await.unwrap();
    assert_eq!(forked_steps.len(), 3);
    assert_ne!(forked_steps[0].id, parent_seq1_snapshot.id);
    assert_ne!(forked_steps[1].id, parent_seq2_snapshot.id);
    assert_ne!(forked_steps[2].id, parent_seq3_snapshot.id);
    assert_eq!(forked_steps[0].scope, forked_scope);
    assert_eq!(forked_steps[1].scope, forked_scope);
    assert_eq!(forked_steps[2].scope, forked_scope);

    let forked_latest = store
        .get_latest_workflow_step(&forked_scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(forked_latest.sequence, 3);
    assert_eq!(forked_latest.node_id, "gate");

    let branches = store.list_branches(&parent_scope).await.unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].parent_run_id, parent_scope.run_id);
    assert_eq!(branches[0].forked_run_id, forked_scope.run_id);
    assert_eq!(branches[0].fork_sequence, 3);
    assert_eq!(branches[0].branch_label, "experiment_branch");
}
