use axum::extract::{Query, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, Response};
use axum::response::Html;
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

fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

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

pub async fn local_mode_handler(
    State(state): State<AppState>,
) -> Result<Json<LocalModeResponse>, ApiError> {
    if !state.allow_local_auth {
        return Err(ApiError::not_found("Not found"));
    }
    Ok(Json(LocalModeResponse { enabled: true }))
}

pub async fn local_session_handler(
    State(state): State<AppState>,
    bytes: axum::body::Bytes,
) -> Result<Json<LocalSessionResponse>, ApiError> {
    if !state.allow_local_auth {
        return Err(ApiError::not_found("Not found"));
    }
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

#[derive(Serialize)]
pub struct SessionUser {
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub picture: Option<String>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<SessionUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub has_unclaimed_legacy_data: bool,
}

pub async fn get_session_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionResponse>, ApiError> {
    let unclaimed = state
        .store
        .is_legacy_data_claimed()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .is_none();

    if let Some(cookie_hdr) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|h| h.to_str().ok())
    {
        for pair in cookie_hdr.split(';') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                if k.trim() == "session_id" {
                    let sid = v.trim();
                    let sessions = state.active_sessions.lock().await;
                    if let Some(s) = sessions.get(sid) {
                        if s.expires_at > Utc::now().timestamp() {
                            return Ok(Json(SessionResponse {
                                authenticated: true,
                                user: Some(SessionUser {
                                    sub: s.sub.clone(),
                                    email: s.email.clone(),
                                    display_name: s.display_name.clone(),
                                    picture: s.picture.clone(),
                                }),
                                tenant_id: Some(s.tenant_id.clone()),
                                workspace_id: Some(s.workspace_id.clone()),
                                has_unclaimed_legacy_data: unclaimed,
                            }));
                        }
                    }
                }
            }
        }
    }

    if let Some(auth_hdr) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = auth_hdr.strip_prefix("Bearer ").map(|s| s.trim()) {
            if let Ok(claims) = crate::server::auth::verify_token_with_state(token, &state) {
                return Ok(Json(SessionResponse {
                    authenticated: true,
                    user: Some(SessionUser {
                        sub: claims.sub.clone(),
                        email: claims.email.clone(),
                        display_name: claims.display_name.clone(),
                        picture: None,
                    }),
                    tenant_id: Some(claims.tenant_id),
                    workspace_id: Some(claims.workspace_id),
                    has_unclaimed_legacy_data: unclaimed,
                }));
            }
        }
    }

    Ok(Json(SessionResponse {
        authenticated: false,
        user: None,
        tenant_id: None,
        workspace_id: None,
        has_unclaimed_legacy_data: unclaimed,
    }))
}

#[derive(Serialize)]
pub struct GoogleStartResponse {
    pub auth_url: String,
    pub state: String,
    pub handoff_id: String,
}

pub async fn google_start_handler(
    State(state): State<AppState>,
    parts: Parts,
) -> Result<Json<GoogleStartResponse>, ApiError> {
    crate::server::auth::validate_host_and_origin(&parts, true)?;

    let flow_state = crate::credentials::generate_secure_secret();
    let nonce = crate::credentials::generate_secure_secret();
    let verifier = crate::credentials::generate_secure_secret();
    let challenge = crate::server::auth::generate_pkce_challenge(&verifier);
    let handoff_id = uuid::Uuid::new_v4().to_string();

    let host = parts
        .headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1:4000");
    let redirect_uri = format!("http://{host}/api/v1/auth/google/callback");
    let client_id = state.oauth_client_id.as_deref().unwrap_or("MOCK_CLIENT_ID");

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
        state.google_auth_url,
        urlencoding_encode(client_id),
        urlencoding_encode(&redirect_uri),
        flow_state,
        nonce,
        challenge
    );

    let now = Utc::now().timestamp();
    let flow = crate::server::state::PendingOauthFlow {
        state: flow_state.clone(),
        nonce,
        code_verifier: verifier,
        handoff_id: handoff_id.clone(),
        expires_at: now + 600,
    };

    state
        .pending_oauth
        .lock()
        .await
        .insert(flow_state.clone(), flow);

    Ok(Json(GoogleStartResponse {
        auth_url,
        state: flow_state,
        handoff_id,
    }))
}

#[derive(Deserialize)]
pub struct GoogleCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
}

pub async fn google_callback_handler(
    State(state): State<AppState>,
    parts: Parts,
    Query(params): Query<GoogleCallbackParams>,
) -> Result<Html<String>, ApiError> {
    crate::server::auth::validate_host_and_origin(&parts, false)?;

    let code = params
        .code
        .ok_or_else(|| ApiError::bad_request("Missing code parameter"))?;
    let req_state = params
        .state
        .ok_or_else(|| ApiError::bad_request("Missing state parameter"))?;

    let flow = state
        .pending_oauth
        .lock()
        .await
        .remove(&req_state)
        .ok_or_else(|| ApiError::unauthorized("Invalid or expired OAuth state"))?;

    if flow.expires_at < Utc::now().timestamp() {
        return Err(ApiError::unauthorized("OAuth state has expired"));
    }

    let host = parts
        .headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1:4000");
    let redirect_uri = format!("http://{host}/api/v1/auth/google/callback");
    let client_id = state.oauth_client_id.as_deref().unwrap_or("MOCK_CLIENT_ID");

    let http_client = reqwest::Client::new();
    let token_resp = http_client
        .post(&state.google_token_url)
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", flow.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Token exchange request failed: {e}")))?;

    if !token_resp.status().is_success() {
        return Err(ApiError::unauthorized(
            "Token exchange rejected by provider",
        ));
    }

    let token_data: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse token response: {e}")))?;

    let id_token = token_data
        .get("id_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::unauthorized("Missing id_token in token response"))?;

    let expected_aud = state.oauth_client_id.as_deref();
    let verified_claims = crate::server::auth::verify_google_id_token(
        id_token,
        &state.google_jwks_url,
        expected_aud,
        Some(&flow.nonce),
    )
    .await?;

    let sub = verified_claims.sub;
    let email = verified_claims.email.unwrap_or_default();
    let display_name = verified_claims.name.unwrap_or_else(|| email.clone());
    let picture = verified_claims.picture;

    let existing_acct = state
        .store
        .get_account_by_sub(&sub)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let (tenant_id, workspace_id) = if let Some(acct) = existing_acct {
        (acct.tenant_id, acct.workspace_id)
    } else {
        (
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
        )
    };

    let now = Utc::now().timestamp();
    let acct_record = crate::profiles::GoogleAccountRecord {
        sub: sub.clone(),
        email: email.clone(),
        display_name: display_name.clone(),
        picture: picture.clone(),
        tenant_id: tenant_id.clone(),
        workspace_id: workspace_id.clone(),
        active: true,
        created_at: now,
        updated_at: now,
    };

    state
        .store
        .upsert_google_account(&acct_record)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    state
        .store
        .set_active_google_account(&sub)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let tokens_key = format!("account:{sub}:google_tokens");
    let tokens_json =
        serde_json::to_string(&token_data).map_err(|e| ApiError::internal(e.to_string()))?;
    state
        .credentials
        .set_secret(&tokens_key, &tokens_json)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let jwt_key = format!("account:{sub}:jwt_secret");
    state
        .credentials
        .get_or_set_secret(&jwt_key, &crate::credentials::generate_secure_secret)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let ready_session = crate::server::state::ReadyHandoffSession {
        handoff_id: flow.handoff_id.clone(),
        sub: sub.clone(),
        session_id: session_id.clone(),
        expires_at: now + 300,
    };

    state
        .ready_handoffs
        .lock()
        .await
        .insert(flow.handoff_id, ready_session);

    let active_info = crate::server::state::ActiveSessionInfo {
        session_id,
        sub,
        email,
        display_name,
        picture,
        tenant_id,
        workspace_id,
        expires_at: now + 86400 * 30,
    };
    state
        .active_sessions
        .lock()
        .await
        .insert(active_info.session_id.clone(), active_info);

    Ok(Html("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Alba - Authentication Successful</title></head><body style=\"font-family:sans-serif;text-align:center;padding:50px;\"><h2>Đăng nhập thành công</h2><p>Bạn có thể đóng tab trình duyệt này và quay lại ứng dụng Alba.</p></body></html>".to_string()))
}

#[derive(Deserialize)]
pub struct HandoffRequest {
    pub handoff_id: String,
}

pub async fn google_handoff_handler(
    State(state): State<AppState>,
    parts: Parts,
    Json(payload): Json<HandoffRequest>,
) -> Result<Response<axum::body::Body>, ApiError> {
    crate::server::auth::validate_host_and_origin(&parts, true)?;

    let handoff = state
        .ready_handoffs
        .lock()
        .await
        .remove(&payload.handoff_id)
        .ok_or_else(|| ApiError::unauthorized("Invalid, expired, or replayed handoff_id"))?;

    if handoff.expires_at < Utc::now().timestamp() {
        return Err(ApiError::unauthorized("handoff_id has expired"));
    }

    let sessions = state.active_sessions.lock().await;
    let session_info = sessions
        .get(&handoff.session_id)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("Active session not found"))?;
    drop(sessions);

    let unclaimed = state
        .store
        .is_legacy_data_claimed()
        .map_err(|e| ApiError::internal(e.to_string()))?
        .is_none();

    let resp_body = serde_json::json!({
        "status": "authenticated",
        "user": {
            "sub": session_info.sub,
            "email": session_info.email,
            "display_name": session_info.display_name,
            "picture": session_info.picture,
        },
        "tenant_id": session_info.tenant_id,
        "workspace_id": session_info.workspace_id,
        "has_unclaimed_legacy_data": unclaimed
    });

    let cookie_val = format!(
        "session_id={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000",
        handoff.session_id
    );

    let mut response = Response::new(axum::body::Body::from(
        serde_json::to_string(&resp_body).unwrap(),
    ));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie_val.parse().unwrap());

    Ok(response)
}

#[derive(Deserialize, Default)]
pub struct SignOutRequest {
    pub cancel_runs: Option<bool>,
}

pub async fn sign_out_handler(
    State(state): State<AppState>,
    parts: Parts,
    bytes: axum::body::Bytes,
) -> Result<Response<axum::body::Body>, ApiError> {
    crate::server::auth::validate_host_and_origin(&parts, true)?;

    let mut current_sub: Option<String> = None;
    let mut current_sid: Option<String> = None;
    let mut current_tenant: Option<String> = None;
    let mut current_workspace: Option<String> = None;

    if let Some(cookie_hdr) = parts
        .headers
        .get(axum::http::header::COOKIE)
        .and_then(|h| h.to_str().ok())
    {
        for pair in cookie_hdr.split(';') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                if k.trim() == "session_id" {
                    let sid = v.trim();
                    let sessions = state.active_sessions.lock().await;
                    if let Some(s) = sessions.get(sid) {
                        current_sub = Some(s.sub.clone());
                        current_sid = Some(sid.to_string());
                        current_tenant = Some(s.tenant_id.clone());
                        current_workspace = Some(s.workspace_id.clone());
                    }
                }
            }
        }
    }

    if current_sub.is_none() {
        if let Some(auth_hdr) = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
        {
            if let Some(token) = auth_hdr.strip_prefix("Bearer ").map(|s| s.trim()) {
                if let Ok(claims) = crate::server::auth::verify_token_with_state(token, &state) {
                    current_sub = Some(claims.sub);
                    current_tenant = Some(claims.tenant_id);
                    current_workspace = Some(claims.workspace_id);
                }
            }
        }
    }

    let sub = current_sub.ok_or_else(|| ApiError::unauthorized("No active session to sign out"))?;

    if let Some(sid) = &current_sid {
        state.active_sessions.lock().await.remove(sid);
    }
    {
        let mut sessions = state.active_sessions.lock().await;
        sessions.retain(|_, s| s.sub != sub);
    }

    let req: SignOutRequest = if bytes.is_empty() {
        SignOutRequest::default()
    } else {
        serde_json::from_slice(&bytes)
            .map_err(|e| ApiError::bad_request(format!("Invalid request body: {e}")))?
    };

    if req.cancel_runs.unwrap_or(true) {
        let cancellations = state.active_cancellations.lock().await;
        for sender in cancellations.values() {
            if let Err(err) = sender.send(true) {
                eprintln!("run cancellation signal dropped: {err}");
            }
        }
    }

    let mut failed_steps: Vec<String> = Vec::new();

    let (tenant_id_str, workspace_id_str) = match (current_tenant, current_workspace) {
        (Some(t), Some(w)) if !t.is_empty() && !w.is_empty() => (Some(t), Some(w)),
        _ => match state.store.get_account_by_sub(&sub) {
            Ok(Some(acc)) => (Some(acc.tenant_id), Some(acc.workspace_id)),
            Ok(None) => {
                failed_steps.push("account_scope_not_found".to_string());
                (None, None)
            }
            Err(e) => {
                failed_steps.push(format!("account_scope_lookup_error: {e}"));
                (None, None)
            }
        },
    };

    let tenant_uuid = tenant_id_str
        .as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .unwrap_or_else(|| {
            if tenant_id_str.is_some() {
                failed_steps.push("invalid_account_tenant_uuid".to_string());
            }
            uuid::Uuid::parse_str(crate::server::auth::SYSTEM_AUDIT_TENANT_ID).unwrap()
        });

    let workspace_uuid = workspace_id_str
        .as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .unwrap_or_else(|| {
            if workspace_id_str.is_some() {
                failed_steps.push("invalid_account_workspace_uuid".to_string());
            }
            uuid::Uuid::parse_str(crate::server::auth::SYSTEM_AUDIT_WORKSPACE_ID).unwrap()
        });

    let audit_scope =
        crate::domain::ExecutionScope::new(tenant_uuid, workspace_uuid, uuid::Uuid::new_v4())
            .unwrap_or_else(|_| crate::domain::ExecutionScope {
                tenant_id: tenant_uuid,
                workspace_id: workspace_uuid,
                run_id: uuid::Uuid::new_v4(),
            });

    if tenant_id_str.is_none()
        || tenant_uuid
            == uuid::Uuid::parse_str(crate::server::auth::SYSTEM_AUDIT_TENANT_ID).unwrap()
    {
        use crate::checkpoints::store::CheckpointStore;
        let entry = crate::checkpoints::types::AuditLogEntry {
            id: uuid::Uuid::new_v4(),
            scope: audit_scope,
            actor_id: Some(sub.clone()),
            action: "auth_sign_out_error".to_string(),
            target_id: None,
            details: serde_json::json!({
                "error": "Failed to resolve valid account scope for sign-out audit",
                "sub": sub.clone(),
                "tenant_id": tenant_id_str,
                "workspace_id": workspace_id_str,
            }),
            created_at: Utc::now(),
        };
        if let Err(e) = state.store.record_audit_log(&entry).await {
            failed_steps.push(format!("record_audit_log: {e}"));
        }
    }

    let tokens_key = format!("account:{sub}:google_tokens");
    match state.credentials.get_secret(&tokens_key) {
        Ok(Some(json_str)) => match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(tokens) => {
                let token_to_revoke = tokens
                    .get("refresh_token")
                    .or_else(|| tokens.get("access_token"))
                    .and_then(|v| v.as_str());

                if let Some(tok) = token_to_revoke {
                    let client_builder = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(5))
                        .build();

                    match client_builder {
                        Ok(client) => {
                            let res = client
                                .post(&state.google_revoke_url)
                                .form(&[("token", tok)])
                                .send()
                                .await;

                            match res {
                                Ok(resp) => {
                                    let status_code = resp.status();
                                    if !status_code.is_success() {
                                        let step = format!(
                                            "google_revocation_http_{}",
                                            status_code.as_u16()
                                        );
                                        failed_steps.push(step.clone());
                                        use crate::checkpoints::store::CheckpointStore;
                                        let entry = crate::checkpoints::types::AuditLogEntry {
                                            id: uuid::Uuid::new_v4(),
                                            scope: audit_scope,
                                            actor_id: Some(sub.clone()),
                                            action: "auth_token_revocation_failed".to_string(),
                                            target_id: None,
                                            details: serde_json::json!({
                                                "status_code": status_code.as_u16(),
                                                "error": format!("Google revocation returned HTTP {}", status_code.as_u16()),
                                            }),
                                            created_at: Utc::now(),
                                        };
                                        if let Err(e) = state.store.record_audit_log(&entry).await {
                                            failed_steps.push(format!("record_audit_log: {e}"));
                                        }
                                    }
                                }
                                Err(e) => {
                                    let is_timeout = e.is_timeout();
                                    let step = if is_timeout {
                                        "google_revocation_timeout".to_string()
                                    } else {
                                        format!("google_revocation_network: {e}")
                                    };
                                    failed_steps.push(step.clone());
                                    use crate::checkpoints::store::CheckpointStore;
                                    let entry = crate::checkpoints::types::AuditLogEntry {
                                        id: uuid::Uuid::new_v4(),
                                        scope: audit_scope,
                                        actor_id: Some(sub.clone()),
                                        action: "auth_token_revocation_failed".to_string(),
                                        target_id: None,
                                        details: serde_json::json!({
                                            "error": e.to_string(),
                                            "is_timeout": is_timeout,
                                        }),
                                        created_at: Utc::now(),
                                    };
                                    if let Err(e) = state.store.record_audit_log(&entry).await {
                                        failed_steps.push(format!("record_audit_log: {e}"));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let step = format!("http_client_build: {e}");
                            failed_steps.push(step.clone());
                            use crate::checkpoints::store::CheckpointStore;
                            let entry = crate::checkpoints::types::AuditLogEntry {
                                id: uuid::Uuid::new_v4(),
                                scope: audit_scope,
                                actor_id: Some(sub.clone()),
                                action: "auth_sign_out_error".to_string(),
                                target_id: None,
                                details: serde_json::json!({ "error": step }),
                                created_at: Utc::now(),
                            };
                            if let Err(e) = state.store.record_audit_log(&entry).await {
                                failed_steps.push(format!("record_audit_log: {e}"));
                            }
                        }
                    }
                } else {
                    let step = "missing_token_in_stored_credentials".to_string();
                    failed_steps.push(step.clone());
                    use crate::checkpoints::store::CheckpointStore;
                    let entry = crate::checkpoints::types::AuditLogEntry {
                        id: uuid::Uuid::new_v4(),
                        scope: audit_scope,
                        actor_id: Some(sub.clone()),
                        action: "auth_sign_out_error".to_string(),
                        target_id: None,
                        details: serde_json::json!({ "error": step }),
                        created_at: Utc::now(),
                    };
                    if let Err(e) = state.store.record_audit_log(&entry).await {
                        failed_steps.push(format!("record_audit_log: {e}"));
                    }
                }
            }
            Err(e) => {
                let step = format!("parse_google_tokens_json: {e}");
                failed_steps.push(step.clone());
                use crate::checkpoints::store::CheckpointStore;
                let entry = crate::checkpoints::types::AuditLogEntry {
                    id: uuid::Uuid::new_v4(),
                    scope: audit_scope,
                    actor_id: Some(sub.clone()),
                    action: "auth_sign_out_error".to_string(),
                    target_id: None,
                    details: serde_json::json!({ "error": step }),
                    created_at: Utc::now(),
                };
                if let Err(e) = state.store.record_audit_log(&entry).await {
                    failed_steps.push(format!("record_audit_log: {e}"));
                }
            }
        },
        Ok(None) => {}
        Err(e) => {
            let step = format!("keychain_get_google_tokens: {e}");
            failed_steps.push(step.clone());
            use crate::checkpoints::store::CheckpointStore;
            let entry = crate::checkpoints::types::AuditLogEntry {
                id: uuid::Uuid::new_v4(),
                scope: audit_scope,
                actor_id: Some(sub.clone()),
                action: "auth_sign_out_error".to_string(),
                target_id: None,
                details: serde_json::json!({ "error": step }),
                created_at: Utc::now(),
            };
            if let Err(e) = state.store.record_audit_log(&entry).await {
                failed_steps.push(format!("record_audit_log: {e}"));
            }
        }
    }

    let jwt_key = format!("account:{sub}:jwt_secret");
    if let Err(e) = state.credentials.delete_secret(&jwt_key) {
        let step = format!("keychain_delete_jwt_secret: {e}");
        failed_steps.push(step.clone());
        use crate::checkpoints::store::CheckpointStore;
        let entry = crate::checkpoints::types::AuditLogEntry {
            id: uuid::Uuid::new_v4(),
            scope: audit_scope,
            actor_id: Some(sub.clone()),
            action: "auth_sign_out_error".to_string(),
            target_id: None,
            details: serde_json::json!({ "error": step }),
            created_at: Utc::now(),
        };
        if let Err(e) = state.store.record_audit_log(&entry).await {
            failed_steps.push(format!("record_audit_log: {e}"));
        }
    }

    if let Err(e) = state.credentials.delete_secret(&tokens_key) {
        let step = format!("keychain_delete_google_tokens: {e}");
        failed_steps.push(step.clone());
        use crate::checkpoints::store::CheckpointStore;
        let entry = crate::checkpoints::types::AuditLogEntry {
            id: uuid::Uuid::new_v4(),
            scope: audit_scope,
            actor_id: Some(sub.clone()),
            action: "auth_sign_out_error".to_string(),
            target_id: None,
            details: serde_json::json!({ "error": step }),
            created_at: Utc::now(),
        };
        if let Err(e) = state.store.record_audit_log(&entry).await {
            failed_steps.push(format!("record_audit_log: {e}"));
        }
    }

    if let Err(e) = state.store.deactivate_all_google_accounts() {
        let step = format!("deactivate_all_google_accounts: {e}");
        failed_steps.push(step.clone());
        use crate::checkpoints::store::CheckpointStore;
        let entry = crate::checkpoints::types::AuditLogEntry {
            id: uuid::Uuid::new_v4(),
            scope: audit_scope,
            actor_id: Some(sub.clone()),
            action: "auth_sign_out_error".to_string(),
            target_id: None,
            details: serde_json::json!({ "error": step }),
            created_at: Utc::now(),
        };
        if let Err(e) = state.store.record_audit_log(&entry).await {
            failed_steps.push(format!("record_audit_log: {e}"));
        }
    }

    let (http_status, status_str) = if failed_steps.is_empty() {
        (axum::http::StatusCode::OK, "signed_out")
    } else {
        (axum::http::StatusCode::MULTI_STATUS, "signed_out_partial")
    };

    let resp_body = serde_json::json!({
        "status": status_str,
        "failed_steps": failed_steps
    });

    let mut response = Response::new(axum::body::Body::from(
        serde_json::to_string(&resp_body).unwrap(),
    ));
    *response.status_mut() = http_status;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    response.headers_mut().insert(
        header::SET_COOKIE,
        "session_id=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"
            .parse()
            .unwrap(),
    );

    Ok(response)
}

#[derive(Deserialize)]
pub struct ClaimLegacyDataRequest {
    pub claim: bool,
}

pub async fn claim_legacy_data_handler(
    State(state): State<AppState>,
    parts: Parts,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<ClaimLegacyDataRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::server::auth::validate_host_and_origin(&parts, true)?;

    if !payload.claim {
        return Ok(Json(serde_json::json!({
            "claimed": false,
            "deferred": true
        })));
    }

    let claimed = state
        .store
        .claim_legacy_data(&user.sub)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if !claimed {
        return Err(ApiError::conflict(
            "Legacy data has already been claimed by an account",
        ));
    }

    let mut sessions = state.active_sessions.lock().await;
    for sess in sessions.values_mut() {
        if sess.sub == user.sub {
            sess.tenant_id = "00000000-0000-0000-0000-000000000001".to_string();
        }
    }

    Ok(Json(serde_json::json!({
        "claimed": true,
        "tenant_id": "00000000-0000-0000-0000-000000000001"
    })))
}
