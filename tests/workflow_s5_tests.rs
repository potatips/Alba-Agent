use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::ModelClient;
use nexus_albaai::models::types::{ModelError, ModelRequest, ModelResponse};
use nexus_albaai::schema::agent::AgentSpec;
use nexus_albaai::schema::node::{
    ConsensusConfigSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode, ParallelBranchNodeSpec,
    RouteSpec, RouterNodeSpec, StartNodeSpec, WorkflowNode,
};
use nexus_albaai::schema::task::TaskSpec;
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::tools::types::ToolRegistry;
use nexus_albaai::workflow::engine::{WorkflowExecutionError, WorkflowExecutor, WorkflowOutcome};
use tempfile::tempdir;
use uuid::Uuid;

pub struct S5MockModelClient {
    responses: Mutex<VecDeque<ModelResponse>>,
}

impl S5MockModelClient {
    pub fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
        }
    }
}

impl ModelClient for S5MockModelClient {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        let mut queue = self.responses.lock().unwrap();
        if let Some(resp) = queue.pop_front() {
            Ok(resp)
        } else {
            Ok(ModelResponse {
                content: Some("done".to_string()),
                tool_calls: vec![],
                usage: None,
            })
        }
    }
}

fn create_agent(id: &str) -> AgentSpec {
    AgentSpec {
        id: id.to_string(),
        role: "Worker".to_string(),
        goal: "Accomplish task".to_string(),
        backstory: "Faithful worker".to_string(),
        model: "mock".to_string(),
        tool_ids: vec![],
        skill_ids: vec![],
        max_iterations: 3,
        temperature: 0.1,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: None,
    }
}

fn create_task(id: &str, agent_id: &str) -> TaskSpec {
    TaskSpec {
        id: id.to_string(),
        agent_id: agent_id.to_string(),
        description: "Execute unit of work".to_string(),
        expected_output: "Done".to_string(),
        output_schema: "text".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec![],
    }
}

#[tokio::test]
async fn test_router_loopback_bounded_execution() {
    let model = Arc::new(S5MockModelClient::new(vec![
        ModelResponse {
            content: Some("attempt_1".to_string()),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("attempt_2".to_string()),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("success".to_string()),
            tool_calls: vec![],
            usage: None,
        },
    ]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("router_loop.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();
    let ws_dir = tempdir().unwrap();

    let executor = WorkflowExecutor::new(model, store, events, policy, registry, ws_dir.path());

    let spec = WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Loopback Workflow".to_string(),
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
                id: "crew_worker".to_string(),
                label: "Worker".to_string(),
                agents: vec![create_agent("a1")],
                tasks: vec![create_task("t1", "a1")],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
                verification_command: None,
            }),
            WorkflowNode::Router(RouterNodeSpec {
                id: "router".to_string(),
                label: "Check Success".to_string(),
                routes: vec![RouteSpec {
                    name: "exit_route".to_string(),
                    field_path: "t1".to_string(),
                    operator: "equals".to_string(),
                    value: Some(serde_json::Value::String("success".to_string())),
                }],
                default_route: "crew_worker".to_string(),
                artifact_gate: false,
                max_loop_iterations: 3,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "crew_worker".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_worker".to_string(),
                target: "router".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "router".to_string(),
                target: "crew_worker".to_string(),
                route: None,
                loop_back: true,
            },
            WorkflowEdgeSpec {
                source: "router".to_string(),
                target: "end".to_string(),
                route: Some("exit_route".to_string()),
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
        .expect("Loopback workflow must eventually exit and succeed");

    match outcome {
        WorkflowOutcome::Completed(val) => {
            assert_eq!(val["t1"], "success");
        }
        _ => panic!("Expected Completed outcome, got {:?}", outcome),
    }
}

#[tokio::test]
async fn test_router_max_loop_iterations_exceeded_fails() {
    let model = Arc::new(S5MockModelClient::new(vec![
        ModelResponse {
            content: Some("infinite_loop".to_string()),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("infinite_loop".to_string()),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("infinite_loop".to_string()),
            tool_calls: vec![],
            usage: None,
        },
    ]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("router_loop_limit.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();
    let ws_dir = tempdir().unwrap();

    let executor = WorkflowExecutor::new(model, store, events, policy, registry, ws_dir.path());

    let spec = WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Loop Limit Workflow".to_string(),
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
                id: "crew_worker".to_string(),
                label: "Worker".to_string(),
                agents: vec![create_agent("a1")],
                tasks: vec![create_task("t1", "a1")],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
                verification_command: None,
            }),
            WorkflowNode::Router(RouterNodeSpec {
                id: "router".to_string(),
                label: "Loop Router".to_string(),
                routes: vec![],
                default_route: "crew_worker".to_string(),
                artifact_gate: false,
                max_loop_iterations: 2,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "crew_worker".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_worker".to_string(),
                target: "router".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "router".to_string(),
                target: "crew_worker".to_string(),
                route: None,
                loop_back: true,
            },
            WorkflowEdgeSpec {
                source: "router".to_string(),
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
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        executor.start_workflow(&spec, scope, None),
    )
    .await
    .expect("test timed out: router loop limit was not enforced");

    let err = res.unwrap_err();

    match err {
        WorkflowExecutionError::LoopLimitExceeded(router_id, limit) => {
            assert_eq!(router_id, "router");
            assert_eq!(limit, 2);
        }
        _ => panic!("Expected LoopLimitExceeded error, got {:?}", err),
    }
}

#[tokio::test]
async fn test_parallel_branch_execution() {
    let model = Arc::new(S5MockModelClient::new(vec![
        ModelResponse {
            content: Some("branch_1_result".to_string()),
            tool_calls: vec![],
            usage: None,
        },
        ModelResponse {
            content: Some("branch_2_result".to_string()),
            tool_calls: vec![],
            usage: None,
        },
    ]));

    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("parallel_branch.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new());
    let registry = ToolRegistry::new();
    let ws_dir = tempdir().unwrap();

    let executor = WorkflowExecutor::new(model, store, events, policy, registry, ws_dir.path());

    let spec = WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Parallel Branch Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::ParallelBranch(ParallelBranchNodeSpec {
                id: "parallel_node".to_string(),
                label: "Execute Branches".to_string(),
                branch_nodes: vec!["branch_a".to_string(), "branch_b".to_string()],
                join: JoinMode::All,
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "branch_a".to_string(),
                label: "Branch A".to_string(),
                agents: vec![create_agent("agent_a")],
                tasks: vec![create_task("task_a", "agent_a")],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec![],
                requires_visual_evidence: false,
                verification_command: None,
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "branch_b".to_string(),
                label: "Branch B".to_string(),
                agents: vec![create_agent("agent_b")],
                tasks: vec![create_task("task_b", "agent_b")],
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
                target: "parallel_node".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "parallel_node".to_string(),
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
        .expect("Parallel branch workflow must succeed");

    match outcome {
        WorkflowOutcome::Completed(val) => {
            assert_eq!(val["task_a"], "branch_1_result");
            assert_eq!(val["task_b"], "branch_2_result");
        }
        _ => panic!("Expected Completed outcome, got {:?}", outcome),
    }
}
