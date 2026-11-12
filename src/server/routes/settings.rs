use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::checkpoints::types::ExecutionPolicyRecord;
use crate::server::auth::AuthenticatedUser;
use crate::server::error::{ApiError, AppJson};
use crate::server::state::AppState;

pub async fn get_chat_models_handler(_auth: AuthenticatedUser) -> Result<Json<Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "models": [
            {
                "id": "gemini-3.8-flash-high",
                "name": "Gemini 3.8 Flash (High)",
                "provider": "google",
                "context_length": 1048576,
            },
            {
                "id": "claude-3-7-sonnet",
                "name": "Claude 3.7 Sonnet",
                "provider": "anthropic",
                "context_length": 200000,
            }
        ],
        "default": "gemini-3.8-flash-high"
    })))
}

pub async fn get_execution_policy_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<ExecutionPolicyRecord>, ApiError> {
    let policy = state
        .store
        .get_execution_policy()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(policy))
}

pub async fn update_execution_policy_handler(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
    AppJson(body): AppJson<ExecutionPolicyRecord>,
) -> Result<Json<ExecutionPolicyRecord>, ApiError> {
    state
        .store
        .update_execution_policy(&body)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(body))
}
