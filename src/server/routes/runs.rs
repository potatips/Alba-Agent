use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::server::auth::AuthenticatedUser;
use crate::server::error::ApiError;
use crate::server::state::AppState;

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
    }))
}
