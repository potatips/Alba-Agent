use axum::Json;
use serde_json::Value;

use crate::server::auth::AuthenticatedUser;
use crate::server::error::ApiError;

pub async fn list_builtin_tools_handler(
    _auth: AuthenticatedUser,
) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(vec![
        serde_json::json!({
            "id": "file_read",
            "name": "Read File",
            "description": "Reads text content within workspace bounds",
            "category": "filesystem",
            "enabled": true,
        }),
        serde_json::json!({
            "id": "file_write",
            "name": "Write File",
            "description": "Writes file content requiring governance approval",
            "category": "filesystem",
            "enabled": true,
        }),
        serde_json::json!({
            "id": "run_cmd",
            "name": "Run Command",
            "description": "Executes shell commands in seatbelt sandbox",
            "category": "system",
            "enabled": true,
        }),
        serde_json::json!({
            "id": "dir_list",
            "name": "List Directory",
            "description": "Lists workspace directories",
            "category": "filesystem",
            "enabled": true,
        }),
    ]))
}

pub async fn list_skills_handler(_auth: AuthenticatedUser) -> Result<Json<Vec<Value>>, ApiError> {
    Ok(Json(vec![]))
}
