use std::collections::BTreeMap;
use uuid::Uuid;

use nexus_albaai::schema::agent::AgentSpec;
use nexus_albaai::schema::node::{
    ApprovalGateNodeSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode,
    NexusGenSandboxNodeSpec, ParallelBranchNodeSpec, RAGNodeSpec, RouterNodeSpec, StartNodeSpec,
    WorkflowNode,
};
use nexus_albaai::schema::task::TaskSpec;
use nexus_albaai::schema::workflow::{WorkflowEdgeSpec, WorkflowModesSpec, WorkflowSpec};
use nexus_albaai::workflow::validation::{validate_workflow_spec, WorkflowValidationError};

fn create_base_workflow() -> WorkflowSpec {
    WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        name: "Test Workflow".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: serde_json::Value::Null,
                schedule: None,
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![WorkflowEdgeSpec {
            source: "start".to_string(),
            target: "end".to_string(),
            route: None,
            loop_back: false,
        }],
        learning_edges: vec![],
        layout: BTreeMap::new(),
        modes: WorkflowModesSpec::default(),
    }
}

#[test]
fn test_valid_minimal_workflow() {
    let spec = create_base_workflow();
    assert!(validate_workflow_spec(&spec).is_ok());
}

#[test]
fn test_missing_start_node() {
    let mut spec = create_base_workflow();
    spec.nodes.retain(|n| !matches!(n, WorkflowNode::Start(_)));
    spec.edges.clear();
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert_eq!(err, WorkflowValidationError::MissingStartNode);
}

#[test]
fn test_multiple_start_nodes() {
    let mut spec = create_base_workflow();
    spec.nodes.push(WorkflowNode::Start(StartNodeSpec {
        id: "start_2".to_string(),
        label: "Start 2".to_string(),
        trigger_type: "manual".to_string(),
        input_schema: serde_json::Value::Null,
        schedule: None,
    }));
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert_eq!(err, WorkflowValidationError::MultipleStartNodes);
}

#[test]
fn test_unreachable_end() {
    let mut spec = create_base_workflow();
    spec.edges.clear();
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert_eq!(err, WorkflowValidationError::UnreachableEnd);
}

#[test]
fn test_edge_missing_source() {
    let mut spec = create_base_workflow();
    spec.edges.push(WorkflowEdgeSpec {
        source: "non_existent".to_string(),
        target: "end".to_string(),
        route: None,
        loop_back: false,
    });
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::EdgeMissingNode { .. }
    ));
}

#[test]
fn test_edge_missing_target() {
    let mut spec = create_base_workflow();
    spec.edges.push(WorkflowEdgeSpec {
        source: "start".to_string(),
        target: "non_existent".to_string(),
        route: None,
        loop_back: false,
    });
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::EdgeMissingNode { .. }
    ));
}

#[test]
fn test_cycle_detected_without_loop_back() {
    let mut spec = create_base_workflow();
    spec.nodes
        .push(WorkflowNode::Approval(ApprovalGateNodeSpec {
            id: "gate".to_string(),
            label: "Gate".to_string(),
            action_description: "Approve action".to_string(),
            required_role: "admin".to_string(),
            timeout_seconds: 3600,
            boundary_rules: vec![],
        }));
    spec.edges = vec![
        WorkflowEdgeSpec {
            source: "start".to_string(),
            target: "gate".to_string(),
            route: None,
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "gate".to_string(),
            target: "start".to_string(),
            route: None,
            loop_back: false,
        },
    ];
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert!(matches!(err, WorkflowValidationError::CycleDetected(_)));
}

#[test]
fn test_cycle_allowed_with_loop_back_true() {
    let mut spec = create_base_workflow();
    spec.nodes.push(WorkflowNode::Router(RouterNodeSpec {
        id: "router".to_string(),
        label: "Router".to_string(),
        routes: vec![],
        default_route: "end".to_string(),
        artifact_gate: false,
        max_loop_iterations: 3,
    }));
    spec.edges = vec![
        WorkflowEdgeSpec {
            source: "start".to_string(),
            target: "router".to_string(),
            route: None,
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "router".to_string(),
            target: "end".to_string(),
            route: None,
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "router".to_string(),
            target: "start".to_string(),
            route: None,
            loop_back: true,
        },
    ];
    assert!(validate_workflow_spec(&spec).is_ok());
}

#[test]
fn test_router_missing_default_route() {
    let mut spec = create_base_workflow();
    spec.nodes.push(WorkflowNode::Router(RouterNodeSpec {
        id: "router".to_string(),
        label: "Router".to_string(),
        routes: vec![],
        default_route: "missing_target".to_string(),
        artifact_gate: false,
        max_loop_iterations: 3,
    }));
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::RouterDefaultTargetMissing { .. }
    ));
}

#[test]
fn test_router_loopback_requires_max_iterations() {
    let mut spec = create_base_workflow();
    spec.nodes.push(WorkflowNode::Router(RouterNodeSpec {
        id: "router".to_string(),
        label: "Router".to_string(),
        routes: vec![],
        default_route: "end".to_string(),
        artifact_gate: false,
        max_loop_iterations: 0,
    }));
    spec.edges = vec![
        WorkflowEdgeSpec {
            source: "start".to_string(),
            target: "router".to_string(),
            route: None,
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "router".to_string(),
            target: "start".to_string(),
            route: None,
            loop_back: true,
        },
        WorkflowEdgeSpec {
            source: "router".to_string(),
            target: "end".to_string(),
            route: None,
            loop_back: false,
        },
    ];
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::InvalidLoopBackRouter { .. }
    ));
}

#[test]
fn test_crew_agent_not_found() {
    let mut spec = create_base_workflow();
    spec.nodes.push(WorkflowNode::Crew(CrewNodeSpec {
        id: "crew".to_string(),
        label: "Crew".to_string(),
        agents: vec![AgentSpec {
            id: "agent_1".to_string(),
            role: "Role 1".to_string(),
            goal: "Goal 1".to_string(),
            backstory: "Backstory 1".to_string(),
            model: "model".to_string(),
            tool_ids: vec![],
            skill_ids: vec![],
            max_iterations: 10,
            temperature: 0.2,
            guardrails: vec![],
            knowledge_capsule_ids: vec![],
            tier: None,
        }],
        tasks: vec![TaskSpec {
            id: "task_1".to_string(),
            agent_id: "agent_unknown".to_string(),
            description: "Do something".to_string(),
            expected_output: "Done".to_string(),
            output_schema: "text".to_string(),
            output_schema_json: None,
            context_task_ids: vec![],
            expected_artifact_paths: vec![],
        }],
        process: CrewProcess::Sequential,
        consensus_config: Default::default(),
        manager_agent_id: None,
        join: JoinMode::Any,
        expected_artifact_paths: vec![],
        requires_visual_evidence: false,
        verification_command: None,
    }));
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert!(matches!(err, WorkflowValidationError::AgentNotFound { .. }));
}

#[test]
fn test_hierarchical_crew_manager_not_found() {
    let mut spec = create_base_workflow();
    spec.nodes.push(WorkflowNode::Crew(CrewNodeSpec {
        id: "crew".to_string(),
        label: "Crew".to_string(),
        agents: vec![AgentSpec {
            id: "agent_1".to_string(),
            role: "Role 1".to_string(),
            goal: "Goal 1".to_string(),
            backstory: "Backstory 1".to_string(),
            model: "model".to_string(),
            tool_ids: vec![],
            skill_ids: vec![],
            max_iterations: 10,
            temperature: 0.2,
            guardrails: vec![],
            knowledge_capsule_ids: vec![],
            tier: None,
        }],
        tasks: vec![TaskSpec {
            id: "task_1".to_string(),
            agent_id: "agent_1".to_string(),
            description: "Do something".to_string(),
            expected_output: "Done".to_string(),
            output_schema: "text".to_string(),
            output_schema_json: None,
            context_task_ids: vec![],
            expected_artifact_paths: vec![],
        }],
        process: CrewProcess::Hierarchical,
        consensus_config: Default::default(),
        manager_agent_id: Some("unknown_manager".to_string()),
        join: JoinMode::Any,
        expected_artifact_paths: vec![],
        requires_visual_evidence: false,
        verification_command: None,
    }));
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert!(matches!(
        err,
        WorkflowValidationError::ManagerAgentNotFound { .. }
    ));
}

#[test]
fn test_unsupported_rag_node_explicit_rejection() {
    let mut spec = create_base_workflow();
    spec.nodes.push(WorkflowNode::Rag(RAGNodeSpec {
        id: "rag_node".to_string(),
        label: "Knowledge Retrieval".to_string(),
        knowledge_base_id: Uuid::new_v4(),
        query_template: "find docs".to_string(),
        top_k: 5,
        mcp_server_ids: vec![],
    }));
    let err = validate_workflow_spec(&spec).unwrap_err();
    match err {
        WorkflowValidationError::UnsupportedNodeKind {
            node_id,
            kind,
            reason,
        } => {
            assert_eq!(node_id, "rag_node");
            assert_eq!(kind, "rag");
            assert!(reason.contains("RAGNode is not supported in v1"));
        }
        _ => panic!("Expected UnsupportedNodeKind error"),
    }
}

#[test]
fn test_unsupported_sandbox_node_explicit_rejection() {
    let mut spec = create_base_workflow();
    spec.nodes
        .push(WorkflowNode::Sandbox(Box::new(NexusGenSandboxNodeSpec {
            id: "sandbox_node".to_string(),
            label: "Coder Reviewer Sandbox".to_string(),
            coder_model: "gpt-4".to_string(),
            reviewer_model: "gpt-4".to_string(),
            sandbox_provider: "docker".to_string(),
            coder_instructions: "write".to_string(),
            reviewer_instructions: "review".to_string(),
            tool_ids: vec![],
            skill_ids: vec![],
            computer_id: None,
            max_rounds: 3,
            timeout_seconds: 60,
            verification_command: "test".to_string(),
            on_unverified: "fail".to_string(),
            on_failure: "fail".to_string(),
            min_added_tests: 1,
            mount_artifacts_from: vec![],
            consensus_config: Default::default(),
            join: JoinMode::Any,
            expected_artifact_paths: vec![],
            requires_visual_evidence: false,
            clean_code_review: false,
        })));
    let err = validate_workflow_spec(&spec).unwrap_err();
    match err {
        WorkflowValidationError::UnsupportedNodeKind {
            node_id,
            kind,
            reason,
        } => {
            assert_eq!(node_id, "sandbox_node");
            assert_eq!(kind, "sandbox");
            assert!(reason.contains("NexusGenSandboxNode is not supported in local engine v1"));
        }
        _ => panic!("Expected UnsupportedNodeKind error"),
    }
}

#[test]
fn test_empty_parallel_branch_rejected() {
    let mut spec = create_base_workflow();
    spec.nodes
        .push(WorkflowNode::ParallelBranch(ParallelBranchNodeSpec {
            id: "pb_node".to_string(),
            label: "Parallel Tasks".to_string(),
            branch_nodes: vec![],
            join: JoinMode::All,
        }));
    let err = validate_workflow_spec(&spec).unwrap_err();
    assert_eq!(
        err,
        WorkflowValidationError::EmptyParallelBranch("pb_node".to_string())
    );
}
