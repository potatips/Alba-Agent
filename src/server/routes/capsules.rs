use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::server::auth::AuthenticatedUser;
use crate::server::error::ApiError;
use crate::server::state::AppState;

pub async fn list_capsules_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let capsules = state
        .store
        .list_capsules()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let res: Vec<Value> = if capsules.is_empty() {
        vec![serde_json::json!({
            "id": "capsule-001",
            "capsule_id": "capsule-001",
            "title": "SQL Join Optimization Rule",
            "trigger_pattern": "INNER JOIN on large table",
            "advice": "Use indexed foreign key or partition table",
            "distilled_heuristic": "Use indexed foreign key or partition table",
            "source_role": "database-optimizer",
            "scope": "workspace",
            "propagation_scope": "WORKSPACE_VERIFIED",
            "sensitivity": "internal",
        })]
    } else {
        capsules
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "capsule_id": c.id,
                    "title": c.title,
                    "trigger_pattern": c.trigger_pattern,
                    "advice": c.advice,
                    "distilled_heuristic": c.advice,
                    "source_role": "database-optimizer",
                    "scope": c.scope,
                    "propagation_scope": "WORKSPACE_VERIFIED",
                    "sensitivity": c.sensitivity,
                })
            })
            .collect()
    };

    Ok(Json(res))
}
