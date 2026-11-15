use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

use crate::schema::node::{CrewNodeSpec, CrewProcess, WorkflowNode};
use crate::schema::workflow::WorkflowSpec;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WorkflowValidationError {
    #[error("Workflow must contain exactly one Start node")]
    MissingStartNode,
    #[error("Workflow contains multiple Start nodes")]
    MultipleStartNodes,
    #[error("Workflow has no reachable End or terminal node from Start")]
    UnreachableEnd,
    #[error("Edge '{edge_source}' -> '{edge_target}' references missing node '{missing}'")]
    EdgeMissingNode {
        edge_source: String,
        edge_target: String,
        missing: String,
    },
    #[error("Non-loopback cycle detected involving node '{0}'")]
    CycleDetected(String),
    #[error("Router '{router_id}' default route references missing node '{target}'")]
    RouterDefaultTargetMissing { router_id: String, target: String },
    #[error("Router '{router_id}' route references missing target node '{target}'")]
    RouterTargetMissing { router_id: String, target: String },
    #[error("Crew node '{node_id}' task references missing agent '{agent_id}'")]
    AgentNotFound { node_id: String, agent_id: String },
    #[error(
        "Hierarchical crew node '{node_id}' references missing manager agent '{manager_agent_id}'"
    )]
    ManagerAgentNotFound {
        node_id: String,
        manager_agent_id: String,
    },
    #[error("Node '{node_id}' of kind '{kind}' is unsupported: {reason}")]
    UnsupportedNodeKind {
        node_id: String,
        kind: String,
        reason: String,
    },
    #[error("ParallelBranch node '{0}' must contain at least one branch node")]
    EmptyParallelBranch(String),
    #[error("Router '{router_id}' participates in loop_back edge but max_loop_iterations must be greater than 0")]
    InvalidLoopBackRouter { router_id: String },
}

pub fn validate_workflow_spec(spec: &WorkflowSpec) -> Result<(), WorkflowValidationError> {
    let mut node_ids = BTreeSet::new();
    let mut start_nodes = Vec::new();
    let mut nodes_by_id = BTreeMap::new();

    for node in &spec.nodes {
        let id = node.id().to_string();
        node_ids.insert(id.clone());
        nodes_by_id.insert(id.clone(), node);

        match node {
            WorkflowNode::Start(_) => {
                start_nodes.push(id);
            }
            WorkflowNode::Rag(rag) => {
                return Err(WorkflowValidationError::UnsupportedNodeKind {
                    node_id: rag.id.clone(),
                    kind: "rag".to_string(),
                    reason: "RAGNode is not supported in v1; external knowledge bases are deferred to post-v1".to_string(),
                });
            }
            WorkflowNode::Sandbox(sandbox) => {
                if sandbox.sandbox_provider == "docker" {
                    return Err(WorkflowValidationError::UnsupportedNodeKind {
                        node_id: sandbox.id.clone(),
                        kind: "sandbox".to_string(),
                        reason: "NexusGenSandboxNode is not supported in local engine v1"
                            .to_string(),
                    });
                }
            }
            WorkflowNode::ParallelBranch(pb) => {
                if pb.branch_nodes.is_empty() {
                    return Err(WorkflowValidationError::EmptyParallelBranch(pb.id.clone()));
                }
            }
            WorkflowNode::Crew(crew) => {
                validate_crew_agents(crew)?;
            }
            WorkflowNode::Approval(_) | WorkflowNode::Router(_) | WorkflowNode::End(_) => {}
        }
    }

    if start_nodes.is_empty() {
        return Err(WorkflowValidationError::MissingStartNode);
    }
    if start_nodes.len() > 1 {
        return Err(WorkflowValidationError::MultipleStartNodes);
    }
    let start_id = &start_nodes[0];

    for edge in &spec.edges {
        if !node_ids.contains(&edge.source) {
            return Err(WorkflowValidationError::EdgeMissingNode {
                edge_source: edge.source.clone(),
                edge_target: edge.target.clone(),
                missing: edge.source.clone(),
            });
        }
        if !node_ids.contains(&edge.target) {
            return Err(WorkflowValidationError::EdgeMissingNode {
                edge_source: edge.source.clone(),
                edge_target: edge.target.clone(),
                missing: edge.target.clone(),
            });
        }
    }

    validate_routers(spec, &node_ids)?;
    validate_acyclicity(spec, &node_ids)?;
    validate_reachability(spec, start_id, &nodes_by_id)?;

    Ok(())
}

fn validate_crew_agents(crew: &CrewNodeSpec) -> Result<(), WorkflowValidationError> {
    let defined_agents: BTreeSet<String> = crew.agents.iter().map(|a| a.id.clone()).collect();

    if crew.process == CrewProcess::Hierarchical {
        match &crew.manager_agent_id {
            Some(mgr) => {
                if !defined_agents.contains(mgr) {
                    return Err(WorkflowValidationError::ManagerAgentNotFound {
                        node_id: crew.id.clone(),
                        manager_agent_id: mgr.clone(),
                    });
                }
            }
            None => {
                return Err(WorkflowValidationError::ManagerAgentNotFound {
                    node_id: crew.id.clone(),
                    manager_agent_id: "<none>".to_string(),
                });
            }
        }
    }

    for task in &crew.tasks {
        if !defined_agents.contains(&task.agent_id) {
            return Err(WorkflowValidationError::AgentNotFound {
                node_id: crew.id.clone(),
                agent_id: task.agent_id.clone(),
            });
        }
    }

    Ok(())
}

fn validate_routers(
    spec: &WorkflowSpec,
    node_ids: &BTreeSet<String>,
) -> Result<(), WorkflowValidationError> {
    let loop_back_sources: BTreeSet<String> = spec
        .edges
        .iter()
        .filter(|e| e.loop_back)
        .map(|e| e.source.clone())
        .collect();

    for node in &spec.nodes {
        if let WorkflowNode::Router(router) = node {
            if !node_ids.contains(&router.default_route) {
                return Err(WorkflowValidationError::RouterDefaultTargetMissing {
                    router_id: router.id.clone(),
                    target: router.default_route.clone(),
                });
            }
            if loop_back_sources.contains(&router.id) && router.max_loop_iterations == 0 {
                return Err(WorkflowValidationError::InvalidLoopBackRouter {
                    router_id: router.id.clone(),
                });
            }
        }
    }

    for edge in &spec.edges {
        if let Some(_route_name) = &edge.route {
            if !node_ids.contains(&edge.target) {
                return Err(WorkflowValidationError::RouterTargetMissing {
                    router_id: edge.source.clone(),
                    target: edge.target.clone(),
                });
            }
        }
    }

    Ok(())
}

fn validate_acyclicity(
    spec: &WorkflowSpec,
    node_ids: &BTreeSet<String>,
) -> Result<(), WorkflowValidationError> {
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for id in node_ids {
        in_degree.insert(id.clone(), 0);
        adj.insert(id.clone(), Vec::new());
    }

    for edge in &spec.edges {
        if !edge.loop_back {
            if let Some(entry) = in_degree.get_mut(&edge.target) {
                *entry += 1;
            }
            if let Some(list) = adj.get_mut(&edge.source) {
                list.push(edge.target.clone());
            }
        }
    }

    let mut queue = VecDeque::new();
    for (id, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(id.clone());
        }
    }

    let mut visited_count = 0;
    while let Some(curr) = queue.pop_front() {
        visited_count += 1;
        if let Some(neighbors) = adj.get(&curr) {
            for neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
    }

    if visited_count < node_ids.len() {
        for (id, &deg) in &in_degree {
            if deg > 0 {
                return Err(WorkflowValidationError::CycleDetected(id.clone()));
            }
        }
    }

    Ok(())
}

fn validate_reachability(
    spec: &WorkflowSpec,
    start_id: &str,
    nodes_by_id: &BTreeMap<String, &WorkflowNode>,
) -> Result<(), WorkflowValidationError> {
    let mut forward_adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in nodes_by_id.keys() {
        forward_adj.insert(id.clone(), Vec::new());
    }

    for edge in &spec.edges {
        if !edge.loop_back {
            if let Some(list) = forward_adj.get_mut(&edge.source) {
                list.push(edge.target.clone());
            }
        }
    }

    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();
    reachable.insert(start_id.to_string());
    queue.push_back(start_id.to_string());

    while let Some(curr) = queue.pop_front() {
        if let Some(neighbors) = forward_adj.get(&curr) {
            for n in neighbors {
                if reachable.insert(n.clone()) {
                    queue.push_back(n.clone());
                }
            }
        }
    }

    let has_end_node = nodes_by_id
        .values()
        .any(|n| matches!(n, WorkflowNode::End(_)));
    let has_terminal = if has_end_node {
        reachable.iter().any(|node_id| {
            nodes_by_id
                .get(node_id)
                .map(|n| matches!(n, WorkflowNode::End(_)))
                .unwrap_or(false)
        })
    } else {
        reachable.iter().any(|node_id| {
            if node_id == start_id {
                return false;
            }
            forward_adj
                .get(node_id)
                .map(|v| v.is_empty())
                .unwrap_or(true)
        })
    };

    if !has_terminal {
        return Err(WorkflowValidationError::UnreachableEnd);
    }

    Ok(())
}
