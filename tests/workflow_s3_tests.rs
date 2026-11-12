use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::{CheckpointError, CheckpointStore};
use nexus_albaai::domain::{ApprovalDecision, ApprovalStatus, ExecutionScope};
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

pub struct S3MockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
}

impl S3MockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
        }
    }
}

impl ModelClient for S3MockModelClient {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let mut queue = self.responses.lock().unwrap();
        if let Some(resp) = queue.pop_front() {
            Ok(resp)
        } else {
            Ok(ModelResponse {
                content: Some("Agent output post approval".to_string()),
                tool_calls: vec![],
                usage: None,
            })
        }
    }
}

fn create_approval_gate_workflow_spec() -> WorkflowSpec {
    let deployer = AgentSpec {
        id: "agent_deployer".to_string(),
        role: "Deployer".to_string(),
        goal: "Deploy production release".to_string(),
        backstory: "Reliable production deployer".to_string(),
        model: "mock-deployer".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 3,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    };

    let task_deploy = TaskSpec {
        id: "task_deploy".to_string(),
        agent_id: "agent_deployer".to_string(),
        description: "Execute deployment scripts".to_string(),
        expected_output: "Release deployed successfully".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    };

    WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Approval Gate Release Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start Execution".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::Approval(ApprovalGateNodeSpec {
                id: "gate_prod".to_string(),
                label: "Production Gate".to_string(),
                action_description: "Approve deployment to production".to_string(),
                required_role: "release_manager".to_string(),
                timeout_seconds: 3600,
                boundary_rules: vec![],
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "crew_deploy".to_string(),
                label: "Deploy Crew".to_string(),
                agents: vec![deployer],
                tasks: vec![task_deploy],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End Execution".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "gate_prod".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "gate_prod".to_string(),
                target: "crew_deploy".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_deploy".to_string(),
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
async fn test_approval_gate_pause_and_allow_once_resume() {
    let model = Arc::new(S3MockModelClient::new(vec![ModelResponse {
        content: Some("Deployment to production complete".to_string()),
        tool_calls: vec![],
        usage: None,
    }]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("workflow_s3_allow.db");
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
    )
    .with_author_id("engineer_alice");

    let spec = create_approval_gate_workflow_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let initial_outcome = executor
        .start_workflow(&spec, scope, None)
        .await
        .expect("Start workflow must succeed");

    let approval_id = match initial_outcome {
        WorkflowOutcome::AwaitingApproval {
            node_id,
            approval_id,
            action_description,
        } => {
            assert_eq!(node_id, "gate_prod");
            assert_eq!(action_description, "Approve deployment to production");
            approval_id
        }
        _ => panic!(
            "Expected AwaitingApproval outcome, got {:?}",
            initial_outcome
        ),
    };

    let approval = store
        .get_approval(&scope, approval_id)
        .await
        .unwrap()
        .expect("Approval must exist");
    assert_eq!(approval.status, ApprovalStatus::Pending);
    assert_eq!(approval.author_id.as_deref(), Some("engineer_alice"));

    let self_approve_err = store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "engineer_alice",
        )
        .await
        .unwrap_err();

    match self_approve_err {
        CheckpointError::InvalidState(msg) => {
            assert!(msg.contains("Separation of duties violation"));
        }
        _ => panic!(
            "Expected separation of duties violation, got {:?}",
            self_approve_err
        ),
    }

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::AllowOnce,
            "manager_bob",
        )
        .await
        .expect("Different approver must be allowed to approve");

    let approved = store
        .get_approval(&scope, approval_id)
        .await
        .unwrap()
        .expect("Approval must exist");
    assert_eq!(approved.status, ApprovalStatus::AllowedOnce);
    assert_eq!(approved.decided_by.as_deref(), Some("manager_bob"));

    let resume_outcome = executor
        .resume_workflow(&spec, scope)
        .await
        .expect("Resume workflow must succeed");

    match resume_outcome {
        WorkflowOutcome::Completed(val) => {
            assert_eq!(val["task_deploy"], "Deployment to production complete");
        }
        _ => panic!("Expected Completed outcome, got {:?}", resume_outcome),
    }

    let final_approval = store
        .get_approval(&scope, approval_id)
        .await
        .unwrap()
        .expect("Approval must exist");
    assert_eq!(final_approval.status, ApprovalStatus::Consumed);

    let audit_logs = store.list_audit_logs(&scope).await.unwrap();
    assert!(audit_logs.iter().any(|log| {
        log.action == "approval.resolved" && log.actor_id.as_deref() == Some("manager_bob")
    }));
}

#[tokio::test]
async fn test_approval_gate_deny() {
    let model = Arc::new(S3MockModelClient::new(vec![]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("workflow_s3_deny.db");
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
    )
    .with_author_id("engineer_alice");

    let spec = create_approval_gate_workflow_spec();
    let scope = ExecutionScope::new(spec.tenant_id, spec.workspace_id, Uuid::new_v4()).unwrap();

    let initial_outcome = executor
        .start_workflow(&spec, scope, None)
        .await
        .expect("Start workflow must succeed");

    let approval_id = match initial_outcome {
        WorkflowOutcome::AwaitingApproval { approval_id, .. } => approval_id,
        _ => panic!("Expected AwaitingApproval outcome"),
    };

    store
        .resolve_approval(
            &scope,
            approval_id,
            ApprovalDecision::Deny,
            "security_officer_charlie",
        )
        .await
        .expect("Denial must succeed");

    let resume_outcome = executor
        .resume_workflow(&spec, scope)
        .await
        .expect("Resume after denial must return Failed outcome");

    match resume_outcome {
        WorkflowOutcome::Failed(msg) => {
            assert!(msg.contains("denied by operator"));
        }
        _ => panic!("Expected Failed outcome, got {:?}", resume_outcome),
    }

    let latest_step = store
        .get_latest_workflow_step(&scope)
        .await
        .unwrap()
        .expect("Latest step must exist");
    assert_eq!(latest_step.status, nexus_albaai::domain::RunStatus::Failed);
}
