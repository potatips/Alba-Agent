use std::collections::BTreeMap;
use uuid::Uuid;

use nexus_albaai::schema::{
    AgentSpec, AgentTier, ApprovalGateNodeSpec, ApprovalRequestSpec, ConsensusConfigSpec,
    CrewNodeSpec, CrewProcess, JoinMode, KnowledgeCapsuleSpec, NexusGenSandboxNodeSpec,
    NodePosition, PathRuleSpec, PropagationScope, RouteSpec, RouterNodeSpec, SensitivityLevel,
    StartNodeSpec, TaskSpec, WorkflowEdgeSpec, WorkflowModesSpec, WorkflowNode, WorkflowSpec,
};

#[test]
fn test_agent_spec_serialization_roundtrip() {
    let agent = AgentSpec {
        id: "architect_1".to_string(),
        role: "Chief Architect".to_string(),
        goal: "Design scalable modular systems".to_string(),
        backstory: "Veteran systems programmer".to_string(),
        model: "gemini-3.8-flash".to_string(),
        tool_ids: vec!["file_read".to_string(), "dir_list".to_string()],
        skill_ids: vec!["architect".to_string()],
        max_iterations: 15,
        temperature: 0.1,
        guardrails: vec!["No external network calls".to_string()],
        knowledge_capsule_ids: vec![Uuid::new_v4()],
        tier: Some(AgentTier::Lead),
    };

    let json_str = serde_json::to_string_pretty(&agent).unwrap();
    let decoded: AgentSpec = serde_json::from_str(&json_str).unwrap();
    assert_eq!(agent, decoded);
    assert_eq!(decoded.tier, Some(AgentTier::Lead));
}

#[test]
fn test_task_spec_serialization_roundtrip() {
    let task = TaskSpec {
        id: "task_design_api".to_string(),
        agent_id: "architect_1".to_string(),
        description: "Specify REST and SSE contracts".to_string(),
        expected_output: "docs/api-contract.md".to_string(),
        output_schema: "MarkdownDocument".to_string(),
        output_schema_json: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" }
            }
        })),
        context_task_ids: vec!["task_baseline".to_string()],
        expected_artifact_paths: vec!["docs/api-contract.md".to_string()],
    };

    let json_str = serde_json::to_string(&task).unwrap();
    let decoded: TaskSpec = serde_json::from_str(&json_str).unwrap();
    assert_eq!(task, decoded);
}

#[test]
fn test_full_workflow_spec_roundtrip() {
    let workflow_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();

    let start_node = WorkflowNode::Start(StartNodeSpec {
        id: "start".to_string(),
        label: "Start Execution".to_string(),
        trigger_type: "manual".to_string(),
        input_schema: serde_json::json!({ "type": "object" }),
        schedule: None,
    });

    let crew_agent = AgentSpec {
        id: "worker_1".to_string(),
        role: "Implementer".to_string(),
        goal: "Implement pure rust logic".to_string(),
        backstory: "Senior engineer".to_string(),
        model: "gemini-3.8-flash".to_string(),
        tool_ids: vec!["file_write".to_string()],
        skill_ids: vec![],
        max_iterations: 10,
        temperature: 0.2,
        guardrails: vec![],
        knowledge_capsule_ids: vec![],
        tier: Some(AgentTier::Specialist),
    };

    let crew_task = TaskSpec {
        id: "task_1".to_string(),
        agent_id: "worker_1".to_string(),
        description: "Write code".to_string(),
        expected_output: "source code".to_string(),
        output_schema: "CodeArtifact".to_string(),
        output_schema_json: None,
        context_task_ids: vec![],
        expected_artifact_paths: vec!["src/app.rs".to_string()],
    };

    let crew_node = WorkflowNode::Crew(CrewNodeSpec {
        id: "crew_build".to_string(),
        label: "Build Crew".to_string(),
        agents: vec![crew_agent],
        tasks: vec![crew_task],
        process: CrewProcess::Sequential,
        consensus_config: ConsensusConfigSpec::default(),
        manager_agent_id: None,
        join: JoinMode::Any,
        expected_artifact_paths: vec!["src/app.rs".to_string()],
        requires_visual_evidence: false,
        verification_command: None,
    });

    let approval_node = WorkflowNode::Approval(ApprovalGateNodeSpec {
        id: "gate_deploy".to_string(),
        label: "Deploy Gate".to_string(),
        action_description: "Approve deployment to staging".to_string(),
        required_role: "admin".to_string(),
        timeout_seconds: 3600,
        boundary_rules: vec!["No production deploy".to_string()],
    });

    let router_node = WorkflowNode::Router(RouterNodeSpec {
        id: "router_eval".to_string(),
        label: "Evaluation Router".to_string(),
        routes: vec![RouteSpec {
            name: "success".to_string(),
            field_path: "eval.status".to_string(),
            operator: "eq".to_string(),
            value: Some(serde_json::json!("passed")),
        }],
        default_route: "fallback".to_string(),
        artifact_gate: true,
        max_loop_iterations: 3,
    });

    let sandbox_node = WorkflowNode::Sandbox(Box::new(NexusGenSandboxNodeSpec {
        id: "sandbox_test".to_string(),
        label: "Sandbox Suite".to_string(),
        coder_model: "gemini-3.8-flash".to_string(),
        reviewer_model: "gemini-3.8-flash".to_string(),
        sandbox_provider: "docker".to_string(),
        coder_instructions: "Run tests".to_string(),
        reviewer_instructions: "Audit output".to_string(),
        tool_ids: vec!["run_cmd".to_string()],
        skill_ids: vec![],
        computer_id: None,
        max_rounds: 5,
        timeout_seconds: 120,
        verification_command: "cargo test".to_string(),
        on_unverified: "deliver_labeled".to_string(),
        on_failure: "fail".to_string(),
        min_added_tests: 0,
        mount_artifacts_from: vec!["crew_build".to_string()],
        consensus_config: ConsensusConfigSpec::default(),
        join: JoinMode::Any,
        expected_artifact_paths: vec!["tests/output.log".to_string()],
        requires_visual_evidence: false,
        clean_code_review: true,
    }));

    let mut layout = BTreeMap::new();
    layout.insert("start".to_string(), NodePosition { x: 0.0, y: 100.0 });
    layout.insert(
        "crew_build".to_string(),
        NodePosition { x: 200.0, y: 100.0 },
    );
    layout.insert(
        "sandbox_test".to_string(),
        NodePosition { x: 400.0, y: 100.0 },
    );
    layout.insert(
        "router_eval".to_string(),
        NodePosition { x: 600.0, y: 100.0 },
    );
    layout.insert(
        "gate_deploy".to_string(),
        NodePosition { x: 800.0, y: 100.0 },
    );

    let edges = vec![
        WorkflowEdgeSpec {
            source: "start".to_string(),
            target: "crew_build".to_string(),
            route: None,
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "crew_build".to_string(),
            target: "sandbox_test".to_string(),
            route: None,
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "sandbox_test".to_string(),
            target: "router_eval".to_string(),
            route: None,
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "router_eval".to_string(),
            target: "gate_deploy".to_string(),
            route: Some("success".to_string()),
            loop_back: false,
        },
        WorkflowEdgeSpec {
            source: "router_eval".to_string(),
            target: "crew_build".to_string(),
            route: Some("fallback".to_string()),
            loop_back: true,
        },
    ];

    let modes = WorkflowModesSpec {
        rigor: "standard".to_string(),
        path_rules: vec![PathRuleSpec {
            pattern: "src/**/*.rs".to_string(),
            rule: "Zero comments allowed".to_string(),
        }],
        channels: vec!["youtube".to_string()],
    };

    let workflow = WorkflowSpec {
        id: workflow_id,
        tenant_id,
        workspace_id,
        name: "Enterprise Build Pipeline".to_string(),
        version: 1,
        nodes: vec![
            start_node,
            crew_node,
            sandbox_node,
            router_node,
            approval_node,
        ],
        edges,
        learning_edges: vec![],
        layout,
        modes,
    };

    let json_val = serde_json::to_value(&workflow).unwrap();
    let decoded: WorkflowSpec = serde_json::from_value(json_val).unwrap();
    assert_eq!(decoded.id, workflow_id);
    assert_eq!(decoded.nodes.len(), 5);
    assert_eq!(decoded.edges.len(), 5);
    assert!(decoded.edges[4].loop_back);
}

#[test]
fn test_knowledge_capsule_spec_roundtrip() {
    let capsule = KnowledgeCapsuleSpec {
        capsule_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        source_agent_id: "agent_devops".to_string(),
        source_role: "SRE".to_string(),
        source_domain_tags: vec!["docker".to_string(), "debian".to_string()],
        trigger_preconditions: "When port 18000 is occupied".to_string(),
        distilled_heuristic: "Query ss -tulpn and free the listener gracefully".to_string(),
        prohibited_contexts: vec!["production_host".to_string()],
        propagation_scope: PropagationScope::WorkspaceVerified,
        sensitivity_level: SensitivityLevel::ConfidentialPiiStripped,
        source_workflow_id: Some(Uuid::new_v4()),
        source_crew_id: Some("crew_infra".to_string()),
        confidence: 0.95,
    };

    let json_str = serde_json::to_string(&capsule).unwrap();
    let decoded: KnowledgeCapsuleSpec = serde_json::from_str(&json_str).unwrap();
    assert_eq!(capsule, decoded);
    assert_eq!(
        decoded.propagation_scope,
        PropagationScope::WorkspaceVerified
    );
    assert_eq!(
        decoded.sensitivity_level,
        SensitivityLevel::ConfidentialPiiStripped
    );
}

#[test]
fn test_approval_request_spec_separated_author_approver() {
    let approval = ApprovalRequestSpec {
        id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        tool_name: "file_write".to_string(),
        parameters: serde_json::json!({
            "path": "src/main.rs",
            "content": "fn main() {}"
        }),
        diff_preview: Some("+ fn main() {}".to_string()),
        author_actor: "agent:coder_01".to_string(),
        status: "pending".to_string(),
        approver_actor: Some("user:lead_operator".to_string()),
        decision: Some("allow_once".to_string()),
    };

    let json_str = serde_json::to_string(&approval).unwrap();
    let decoded: ApprovalRequestSpec = serde_json::from_str(&json_str).unwrap();
    assert_eq!(approval, decoded);
    assert_ne!(decoded.author_actor, decoded.approver_actor.unwrap());
}
