use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::checkpoints::types::RunRecord;
use crate::domain::ExecutionScope;
use crate::schema::WorkflowSpec;
use crate::server::auth::{AuthenticatedUser, DEFAULT_LOCAL_TENANT_ID, DEFAULT_LOCAL_WORKSPACE_ID};
use crate::server::error::{ApiError, AppJson};
use crate::server::state::AppState;
use crate::server::workflow_runner::run_workflow_background;
use crate::workflow::validation::validate_workflow_spec;

#[derive(Deserialize)]
pub struct ListWorkflowsQuery {
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct ImportWorkflowQuery {
    pub file_format: Option<String>,
}

pub async fn validate_workflow_handler(
    _auth: AuthenticatedUser,
    AppJson(spec): AppJson<WorkflowSpec>,
) -> Result<Json<Value>, ApiError> {
    validate_workflow_spec(&spec).map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({ "valid": true })))
}

pub async fn import_workflow_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Query(_query): Query<ImportWorkflowQuery>,
    body: axum::body::Bytes,
) -> Result<Json<WorkflowSpec>, ApiError> {
    let spec: WorkflowSpec =
        serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?;
    validate_workflow_spec(&spec).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let spec_json = serde_json::to_string(&spec).map_err(|e| ApiError::internal(e.to_string()))?;
    let id_str = spec.id.to_string();
    state
        .store
        .insert_workflow(&id_str, &spec.name, None, &spec_json)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(spec))
}

pub async fn list_conversations_handler(
    _auth: AuthenticatedUser,
) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(vec![]))
}

pub async fn list_templates_handler(
    _auth: AuthenticatedUser,
) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(vec![]))
}

#[derive(Deserialize)]
pub struct CreateRunRequest {
    #[serde(default)]
    pub input_data: Value,
}

#[derive(Serialize)]
pub struct CreateRunResponse {
    pub run_id: String,
    pub workflow_id: String,
    pub status: &'static str,
}

pub async fn list_workflows_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<ListWorkflowsQuery>,
) -> Result<Json<Vec<crate::checkpoints::types::WorkflowSummary>>, ApiError> {
    let limit = query.limit.unwrap_or(50);
    let list = state
        .store
        .list_workflows(limit)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(list))
}

pub async fn create_workflow_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    AppJson(spec): AppJson<WorkflowSpec>,
) -> Result<(StatusCode, Json<WorkflowSpec>), ApiError> {
    validate_workflow_spec(&spec).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let spec_json = serde_json::to_string(&spec).map_err(|e| ApiError::internal(e.to_string()))?;
    let id_str = spec.id.to_string();
    state
        .store
        .insert_workflow(&id_str, &spec.name, None, &spec_json)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(spec)))
}

pub async fn get_workflow_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowSpec>, ApiError> {
    let spec_json_opt = state
        .store
        .get_workflow(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let spec_json = spec_json_opt.ok_or_else(|| ApiError::not_found("Workflow not found"))?;
    let spec: WorkflowSpec =
        serde_json::from_str(&spec_json).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(spec))
}

pub async fn update_workflow_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    AppJson(spec): AppJson<WorkflowSpec>,
) -> Result<Json<WorkflowSpec>, ApiError> {
    validate_workflow_spec(&spec).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let spec_json = serde_json::to_string(&spec).map_err(|e| ApiError::internal(e.to_string()))?;
    let updated = state
        .store
        .update_workflow(&id, &spec.name, None, &spec_json)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if !updated {
        return Err(ApiError::not_found("Workflow not found"));
    }

    Ok(Json(spec))
}

pub async fn create_workflow_run_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    AppJson(body): AppJson<CreateRunRequest>,
) -> Result<(StatusCode, Json<CreateRunResponse>), ApiError> {
    let spec_json_opt = state
        .store
        .get_workflow(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let spec_json = spec_json_opt.ok_or_else(|| ApiError::not_found("Workflow not found"))?;
    let spec: WorkflowSpec =
        serde_json::from_str(&spec_json).map_err(|e| ApiError::internal(e.to_string()))?;

    let run_id = Uuid::new_v4();
    let tenant_id =
        Uuid::parse_str(DEFAULT_LOCAL_TENANT_ID).map_err(|e| ApiError::internal(e.to_string()))?;
    let workspace_id = Uuid::parse_str(DEFAULT_LOCAL_WORKSPACE_ID)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let now = Utc::now().to_rfc3339();
    let run_record = RunRecord {
        id: run_id.to_string(),
        workflow_id: id.clone(),
        status: "QUEUED".to_string(),
        input_data: body.input_data.to_string(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
    };

    state
        .store
        .insert_run(&run_record)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let state_clone = state.clone();
    let spec_clone = spec.clone();
    let input_val = body.input_data.clone();
    tokio::spawn(async move {
        run_workflow_background(state_clone, scope, spec_clone, Some(input_val)).await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateRunResponse {
            run_id: run_id.to_string(),
            workflow_id: id,
            status: "QUEUED",
        }),
    ))
}
