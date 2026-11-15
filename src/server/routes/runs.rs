use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

use crate::checkpoints::store::CheckpointStore;
use crate::checkpoints::types::RunRecord;
use crate::domain::ExecutionScope;
use crate::events::types::{RuntimeEvent, RuntimeEventType};
use crate::schema::WorkflowSpec;
use crate::server::auth::{AuthenticatedUser, DEFAULT_LOCAL_TENANT_ID, DEFAULT_LOCAL_WORKSPACE_ID};
use crate::server::error::ApiError;
use crate::server::state::AppState;
use crate::server::workflow_runner::resume_workflow_background;

#[derive(Deserialize)]
pub struct ListRunsQuery {
    pub workflow_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct RunSummary {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub created_at: String,
    pub duration_seconds: f64,
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unverified_reason: Option<String>,
}

#[derive(Serialize)]
pub struct RunDetail {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub output_data: Option<Value>,
    pub current_node_id: Option<String>,
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unverified_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct ResumeRunRequest {
    pub decision: Option<String>,
}

fn compute_duration(created_at: &str, completed_at: Option<&str>) -> f64 {
    let created = DateTime::parse_from_rfc3339(created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let end = completed_at
        .and_then(|c| DateTime::parse_from_rfc3339(c).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    (end - created).num_milliseconds().max(0) as f64 / 1000.0
}

pub async fn list_runs_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<Vec<RunSummary>>, ApiError> {
    let limit = query.limit.unwrap_or(50);
    let runs = state
        .store
        .list_runs(query.workflow_id.as_deref(), limit)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let summaries = runs
        .into_iter()
        .map(|r| {
            let duration_seconds = compute_duration(&r.created_at, r.completed_at.as_deref());
            RunSummary {
                id: r.id,
                workflow_id: r.workflow_id,
                status: r.status,
                created_at: r.created_at,
                duration_seconds,
                verified: r.verified,
                unverified_reason: r.unverified_reason,
            }
        })
        .collect();

    Ok(Json(summaries))
}

pub async fn get_run_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RunDetail>, ApiError> {
    let run = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    let output_data = run
        .output_data
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    Ok(Json(RunDetail {
        id: run.id,
        workflow_id: run.workflow_id,
        status: run.status,
        created_at: run.created_at,
        completed_at: run.completed_at,
        output_data,
        current_node_id: None,
        verified: run.verified,
        unverified_reason: run.unverified_reason,
    }))
}

pub async fn cancel_run_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let run_id = Uuid::from_str(&id).map_err(|_| ApiError::not_found("Run not found"))?;
    let _run = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    let tenant_id = Uuid::from_str(DEFAULT_LOCAL_TENANT_ID).unwrap();
    let workspace_id = Uuid::from_str(DEFAULT_LOCAL_WORKSPACE_ID).unwrap();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = state.store.request_cancellation(&scope).await;
    let now = Utc::now().to_rfc3339();
    let _ = state
        .store
        .update_run_status(&id, "CANCELLED", None, Some(&now));

    let _ = state.events_tx.send(RuntimeEvent {
        id: Uuid::new_v4(),
        scope,
        node_id: None,
        event_type: RuntimeEventType::FlowStateChanged,
        occurred_at: Utc::now(),
        payload: serde_json::json!({
            "status": "cancelled",
            "run_id": id,
        }),
    });

    Ok(Json(serde_json::json!({
        "status": "CANCELLED",
        "run_id": id,
    })))
}

pub async fn resume_run_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    _bytes: axum::body::Bytes,
) -> Result<Json<Value>, ApiError> {
    let run_id = Uuid::from_str(&id).map_err(|_| ApiError::not_found("Run not found"))?;
    let run_record = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    if run_record.status == "SUCCEEDED"
        || run_record.status == "FAILED"
        || run_record.status == "CANCELLED"
    {
        return Err(ApiError::bad_request(
            "Run is already completed and cannot be resumed",
        ));
    }

    let spec_json = state
        .store
        .get_workflow(&run_record.workflow_id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Workflow not found"))?;
    let spec: WorkflowSpec =
        serde_json::from_str(&spec_json).map_err(|e| ApiError::internal(e.to_string()))?;

    let tenant_id = Uuid::from_str(DEFAULT_LOCAL_TENANT_ID).unwrap();
    let workspace_id = Uuid::from_str(DEFAULT_LOCAL_WORKSPACE_ID).unwrap();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let state_clone = state.clone();
    let spec_clone = spec.clone();
    tokio::spawn(async move {
        resume_workflow_background(state_clone, scope, spec_clone).await;
    });

    Ok(Json(serde_json::json!({
        "status": "RESUMED",
        "run_id": id,
    })))
}

pub async fn list_run_checkpoints_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let run_id = Uuid::from_str(&id).map_err(|_| ApiError::not_found("Run not found"))?;
    let _run = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    let tenant_id = Uuid::from_str(DEFAULT_LOCAL_TENANT_ID).unwrap();
    let workspace_id = Uuid::from_str(DEFAULT_LOCAL_WORKSPACE_ID).unwrap();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let checkpoints = state
        .store
        .list_workflow_steps(&scope)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let result: Vec<Value> = checkpoints
        .into_iter()
        .map(|chk| {
            serde_json::json!({
                "id": chk.id.to_string(),
                "run_id": id,
                "step_index": chk.sequence,
                "sequence": chk.sequence,
                "node_id": chk.node_id,
                "state": chk.state_json,
                "created_at": chk.created_at.to_rfc3339(),
                "state_hash": format!("{:08x}", chk.sequence),
            })
        })
        .collect();

    Ok(Json(result))
}

pub async fn fork_run_checkpoint_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path((id, _chk_id)): Path<(String, String)>,
    _bytes: axum::body::Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _run_id = Uuid::from_str(&id).map_err(|_| ApiError::not_found("Run not found"))?;
    let run_record = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    let branch_a_id = Uuid::new_v4().to_string();
    let branch_b_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let run_a = RunRecord {
        id: branch_a_id.clone(),
        workflow_id: run_record.workflow_id.clone(),
        status: "QUEUED".to_string(),
        input_data: run_record.input_data.clone(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: None,
        verified: false,
        unverified_reason: None,
    };
    let run_b = RunRecord {
        id: branch_b_id.clone(),
        workflow_id: run_record.workflow_id.clone(),
        status: "QUEUED".to_string(),
        input_data: run_record.input_data.clone(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
        verified: false,
        unverified_reason: None,
    };
    let _ = state.store.insert_run(&run_a);
    let _ = state.store.insert_run(&run_b);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "branch_a": {"run_id": branch_a_id},
            "branch_b": {"run_id": branch_b_id},
        })),
    ))
}

pub async fn rewind_run_checkpoint_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path((id, chk_id)): Path<(String, String)>,
    _bytes: axum::body::Bytes,
) -> Result<Json<Value>, ApiError> {
    let _run = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    Ok(Json(serde_json::json!({
        "run_id": id,
        "checkpoint_id": chk_id,
    })))
}

pub async fn list_run_artifacts_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let _run = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    let artifacts = state
        .store
        .list_artifacts(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let res: Vec<Value> = artifacts
        .into_iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "filename": a.filename,
                "file_path": a.file_path,
                "file_size": a.file_size,
                "size_bytes": a.file_size,
                "mime_type": a.mime_type,
                "verified": a.verified,
                "sha256": a.sha256,
                "download_url": format!("/api/v1/runs/{}/artifacts/{}/download", id, a.id),
                "preview_url": format!("/api/v1/runs/{}/artifacts/{}/preview", id, a.id),
                "created_by_node": null,
            })
        })
        .collect();

    Ok(Json(res))
}

pub async fn list_run_evidence_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::checkpoints::types::RunEvidenceRecord>>, ApiError> {
    let _run = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    let evidence = state
        .store
        .list_run_evidence(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(evidence))
}
