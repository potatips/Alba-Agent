use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::server::auth::{
    sign_token, AuthenticatedUser, LocalSessionClaims, DEFAULT_LOCAL_TENANT_ID,
    DEFAULT_LOCAL_USER_DISPLAY_NAME, DEFAULT_LOCAL_USER_EMAIL, DEFAULT_LOCAL_USER_ID,
    DEFAULT_LOCAL_USER_PLAN, DEFAULT_LOCAL_WORKSPACE_ID,
};
use crate::server::error::ApiError;
use crate::server::state::AppState;

#[derive(Serialize)]
pub struct LocalModeResponse {
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct LocalSessionResponse {
    pub access_token: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub expires_in: i64,
}

#[derive(Deserialize, Default)]
pub struct LocalSessionRequest {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub sub: Option<String>,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub email: String,
    pub display_name: String,
    pub plan: String,
}

pub async fn local_mode_handler() -> Json<LocalModeResponse> {
    Json(LocalModeResponse { enabled: true })
}

pub async fn local_session_handler(
    State(state): State<AppState>,
    bytes: axum::body::Bytes,
) -> Result<Json<LocalSessionResponse>, ApiError> {
    let req: LocalSessionRequest = if bytes.is_empty() {
        LocalSessionRequest::default()
    } else {
        serde_json::from_slice(&bytes).unwrap_or_default()
    };
    let expires_in: i64 = 86400;
    let exp = Utc::now().timestamp() + expires_in;
    let profile_id = req.sub.unwrap_or_else(|| DEFAULT_LOCAL_USER_ID.to_string());
    let claims = LocalSessionClaims {
        sub: profile_id.clone(),
        tenant_id: DEFAULT_LOCAL_TENANT_ID.to_string(),
        workspace_id: DEFAULT_LOCAL_WORKSPACE_ID.to_string(),
        email: req
            .email
            .unwrap_or_else(|| DEFAULT_LOCAL_USER_EMAIL.to_string()),
        display_name: req.display_name.unwrap_or_else(|| {
            std::env::var("NEXUS_ACTOR_ID")
                .or_else(|_| std::env::var("NEXUS_USER_DISPLAY_NAME"))
                .unwrap_or_else(|_| DEFAULT_LOCAL_USER_DISPLAY_NAME.to_string())
        }),
        plan: DEFAULT_LOCAL_USER_PLAN.to_string(),
        exp,
    };
    let secret = state.get_or_create_jwt_secret(&profile_id)?;
    let access_token = sign_token(&claims, &secret)?;
    Ok(Json(LocalSessionResponse {
        access_token,
        tenant_id: claims.tenant_id,
        workspace_id: claims.workspace_id,
        expires_in,
    }))
}

pub async fn me_handler(
    AuthenticatedUser(claims): AuthenticatedUser,
) -> Result<Json<MeResponse>, ApiError> {
    Ok(Json(MeResponse {
        email: claims.email,
        display_name: claims.display_name,
        plan: claims.plan,
    }))
}
