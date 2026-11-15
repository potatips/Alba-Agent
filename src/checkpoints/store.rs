use thiserror::Error;
use uuid::Uuid;

use crate::checkpoints::types::{
    AuditLogEntry, ForkRunRequest, RunCheckpoint, TrajectoryBranch, WorkflowStepCheckpoint,
};
use crate::domain::{ApprovalDecision, ApprovalRequest, ExecutionScope};

#[derive(Error, Debug)]
pub enum CheckpointError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid checkpoint state: {0}")]
    InvalidState(String),
    #[error("Concurrent modification conflict: {0}")]
    Conflict(String),
}

pub trait CheckpointStore: Send + Sync {
    fn save_checkpoint(
        &self,
        checkpoint: &RunCheckpoint,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn get_latest_checkpoint(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<Option<RunCheckpoint>, CheckpointError>> + Send;

    fn list_checkpoints(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<Vec<RunCheckpoint>, CheckpointError>> + Send;

    fn save_approval(
        &self,
        approval: &ApprovalRequest,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn get_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<ApprovalRequest>, CheckpointError>> + Send;

    fn get_pending_approval(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<Option<ApprovalRequest>, CheckpointError>> + Send;

    fn resolve_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
        decision: ApprovalDecision,
        actor: &str,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn consume_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
    ) -> impl std::future::Future<Output = Result<bool, CheckpointError>> + Send;

    fn claim_approved_execution(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
        executing_checkpoint: &RunCheckpoint,
    ) -> impl std::future::Future<Output = Result<bool, CheckpointError>> + Send;

    fn complete_approval_execution(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
    ) -> impl std::future::Future<Output = Result<bool, CheckpointError>> + Send;

    fn record_audit_log(
        &self,
        entry: &AuditLogEntry,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn list_audit_logs(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<Vec<AuditLogEntry>, CheckpointError>> + Send;

    fn request_cancellation(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn is_cancelled(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<bool, CheckpointError>> + Send;

    fn get_checkpoint_at_sequence(
        &self,
        scope: &ExecutionScope,
        sequence: u32,
    ) -> impl std::future::Future<Output = Result<Option<RunCheckpoint>, CheckpointError>> + Send;

    fn fork_run(
        &self,
        req: &ForkRunRequest,
    ) -> impl std::future::Future<Output = Result<(RunCheckpoint, TrajectoryBranch), CheckpointError>>
           + Send;

    fn list_branches(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<Vec<TrajectoryBranch>, CheckpointError>> + Send;

    fn save_workflow_step(
        &self,
        step: &WorkflowStepCheckpoint,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn get_latest_workflow_step(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<Option<WorkflowStepCheckpoint>, CheckpointError>> + Send;

    fn list_workflow_steps(
        &self,
        scope: &ExecutionScope,
    ) -> impl std::future::Future<Output = Result<Vec<WorkflowStepCheckpoint>, CheckpointError>> + Send;

    fn claim_workflow_approval(
        &self,
        scope: &ExecutionScope,
        approval_id: Uuid,
        executing_step: &WorkflowStepCheckpoint,
    ) -> impl std::future::Future<Output = Result<bool, CheckpointError>> + Send;

    fn record_tool_call(
        &self,
        scope: &ExecutionScope,
        node_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn get_tool_call_status(
        &self,
        scope: &ExecutionScope,
        node_id: &str,
        tool_call_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<(String, Option<String>)>, CheckpointError>>
           + Send;

    fn complete_tool_call(
        &self,
        scope: &ExecutionScope,
        node_id: &str,
        tool_call_id: &str,
        output: &str,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send;

    fn fork_workflow_run(
        &self,
        source_scope: &ExecutionScope,
        target_sequence: u32,
        branch_label: &str,
    ) -> impl std::future::Future<Output = Result<(ExecutionScope, TrajectoryBranch), CheckpointError>>
           + Send;

    fn record_run_evidence(
        &self,
        record: &crate::checkpoints::types::RunEvidenceRecord,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send {
        async move {
            let _ = record;
            Ok(())
        }
    }

    fn record_artifact(
        &self,
        artifact: &crate::checkpoints::types::ArtifactRecord,
    ) -> impl std::future::Future<Output = Result<(), CheckpointError>> + Send {
        async move {
            let _ = artifact;
            Ok(())
        }
    }

    fn mark_verified(
        &self,
        run_id: &str,
        verified: bool,
    ) -> impl std::future::Future<Output = Result<bool, CheckpointError>> + Send {
        async move {
            let _ = (run_id, verified);
            Ok(false)
        }
    }
}
