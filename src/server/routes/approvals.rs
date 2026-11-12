use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::checkpoints::store::CheckpointStore;
use crate::checkpoints::CheckpointError;
use crate::domain::ApprovalDecision;
use crate::schema::ApprovalRequestSpec;
use crate::schema::WorkflowSpec;
use crate::server::auth::AuthenticatedUser;
use crate::server::error::{ApiError, AppJson};
use crate::server::state::AppState;
use crate::server::workflow_runner::resume_workflow_background;

#[derive(Deserialize)]
pub struct ApprovalsQuery {
    pub run_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ResolveApprovalRequest {
    pub decision: String,
    pub original_ai_output: Option<Value>,
    pub human_corrected_output: Option<Value>,
    pub feedback: Option<String>,
}

#[derive(Serialize)]
pub struct ResolveApprovalResponse {
    pub status: String,
    pub approval_id: String,
    pub decision: String,
}

pub async fn list_approvals_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<ApprovalsQuery>,
) -> Result<Json<Vec<ApprovalRequestSpec>>, ApiError> {
    let list = state
        .store
        .list_pending_approvals_by_run(query.run_id.as_deref())
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(list))
}

pub async fn resolve_approval_handler(
    auth: AuthenticatedUser,
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    AppJson(body): AppJson<ResolveApprovalRequest>,
) -> Result<Json<ResolveApprovalResponse>, ApiError> {
    let approval_uuid =
        Uuid::from_str(&id).map_err(|_| ApiError::not_found("Approval not found"))?;

    let details = state
        .store
        .get_approval_details(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Approval request not found"))?;

    let (scope, _author, _status) = details;

    let actor = headers
        .get("x-actor-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&auth.0.display_name);

    let decision = match body.decision.as_str() {
        "allow_once" | "allow_always" => ApprovalDecision::AllowOnce,
        "deny" => ApprovalDecision::Deny,
        other => {
            return Err(ApiError::bad_request(format!(
                "Invalid decision: {}",
                other
            )))
        }
    };

    let resolve_res = state
        .store
        .resolve_approval(&scope, approval_uuid, decision, actor)
        .await;

    match resolve_res {
        Ok(()) => {
            let run_id_str = scope.run_id.to_string();
            if let Ok(Some(run_record)) = state.store.get_run(&run_id_str) {
                if run_record.status == "AWAITING_APPROVAL" {
                    if let Ok(Some(spec_json)) = state.store.get_workflow(&run_record.workflow_id) {
                        if let Ok(spec) = serde_json::from_str::<WorkflowSpec>(&spec_json) {
                            let state_clone = state.clone();
                            tokio::spawn(async move {
                                resume_workflow_background(state_clone, scope, spec).await;
                            });
                        }
                    }
                }
            }

            Ok(Json(ResolveApprovalResponse {
                status: "RESOLVED".to_string(),
                approval_id: id,
                decision: body.decision,
            }))
        }
        Err(CheckpointError::InvalidState(msg)) => {
            if msg.contains("Separation of duties violation") {
                Err(ApiError::forbidden(msg))
            } else {
                Err(ApiError::new(StatusCode::CONFLICT, msg))
            }
        }
        Err(e) => Err(ApiError::internal(e.to_string())),
    }
}
