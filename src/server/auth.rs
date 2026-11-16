use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::server::error::ApiError;

pub const DEFAULT_LOCAL_USER_EMAIL: &str = "local@nexus.local";
pub const DEFAULT_LOCAL_USER_DISPLAY_NAME: &str = "Workspace Owner";
pub const DEFAULT_LOCAL_USER_PLAN: &str = "LOCAL";
pub const DEFAULT_LOCAL_TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";
pub const DEFAULT_LOCAL_WORKSPACE_ID: &str = "00000000-0000-0000-0000-000000000002";
pub const DEFAULT_LOCAL_USER_ID: &str = "00000000-0000-0000-0000-000000000003";
pub const SYSTEM_AUDIT_TENANT_ID: &str = "00000000-0000-0000-0000-0000000000fe";
pub const SYSTEM_AUDIT_WORKSPACE_ID: &str = "00000000-0000-0000-0000-0000000000fe";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSessionClaims {
    pub sub: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub email: String,
    pub display_name: String,
    pub plan: String,
    pub exp: i64,
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k_pad = [0u8; 64];
    if key.len() > 64 {
        let hash = Sha256::digest(key);
        k_pad[..32].copy_from_slice(&hash);
    } else {
        k_pad[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0u8; 64];
    let mut opad = [0u8; 64];
    for i in 0..64 {
        ipad[i] = k_pad[i] ^ 0x36;
        opad[i] = k_pad[i] ^ 0x5c;
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

pub fn base64url_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };
        out.push(CHARS[(b0 >> 2) as usize] as char);
        out.push(CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < data.len() {
            out.push(CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        }
        if i + 2 < data.len() {
            out.push(CHARS[(b2 & 0x3f) as usize] as char);
        }
        i += 3;
    }
    out
}

pub fn base64url_decode(s: &str) -> Result<Vec<u8>, ApiError> {
    fn decode_char(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut i = 0;
    while i < bytes.len() {
        let c0 = decode_char(bytes[i]).ok_or_else(|| ApiError::unauthorized("Invalid token"))?;
        let c1 = decode_char(bytes.get(i + 1).copied().unwrap_or(b'A'))
            .ok_or_else(|| ApiError::unauthorized("Invalid token"))?;
        out.push((c0 << 2) | (c1 >> 4));
        if i + 2 < bytes.len() {
            let c2 =
                decode_char(bytes[i + 2]).ok_or_else(|| ApiError::unauthorized("Invalid token"))?;
            out.push(((c1 & 0x0f) << 4) | (c2 >> 2));
            if i + 3 < bytes.len() {
                let c3 = decode_char(bytes[i + 3])
                    .ok_or_else(|| ApiError::unauthorized("Invalid token"))?;
                out.push(((c2 & 0x03) << 6) | c3);
            }
        }
        i += 4;
    }
    Ok(out)
}

pub fn sign_token(claims: &LocalSessionClaims, secret: &str) -> Result<String, ApiError> {
    let header = r#"{"alg":"HS256","typ":"JWT"}"#;
    let header_b64 = base64url_encode(header.as_bytes());
    let claims_json =
        serde_json::to_string(claims).map_err(|e| ApiError::internal(e.to_string()))?;
    let payload_b64 = base64url_encode(claims_json.as_bytes());
    let to_sign = format!("{header_b64}.{payload_b64}");
    let signature = hmac_sha256(secret.as_bytes(), to_sign.as_bytes());
    let sig_b64 = base64url_encode(&signature);
    Ok(format!("{to_sign}.{sig_b64}"))
}

pub fn verify_token(token: &str, secret: &str) -> Result<LocalSessionClaims, ApiError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ApiError::unauthorized("Invalid token structure"));
    }
    let to_sign = format!("{}.{}", parts[0], parts[1]);
    let expected_sig = hmac_sha256(secret.as_bytes(), to_sign.as_bytes());
    let actual_sig = base64url_decode(parts[2])?;
    if actual_sig.as_slice() != expected_sig {
        return Err(ApiError::unauthorized("Invalid token signature"));
    }
    let payload_bytes = base64url_decode(parts[1])?;
    let claims: LocalSessionClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|_| ApiError::unauthorized("Invalid token payload"))?;
    let now = chrono::Utc::now().timestamp();
    if claims.exp < now {
        return Err(ApiError::unauthorized("Token expired"));
    }
    Ok(claims)
}

pub fn verify_token_with_state(
    token: &str,
    state: &crate::server::state::AppState,
) -> Result<LocalSessionClaims, ApiError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(ApiError::unauthorized("Invalid token structure"));
    }
    let payload_bytes = base64url_decode(parts[1])?;
    let unverified: LocalSessionClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|_| ApiError::unauthorized("Invalid token payload"))?;

    let acct_key = format!("account:{}:jwt_secret", unverified.sub);
    let secret = match state.credentials.get_secret(&acct_key) {
        Ok(Some(s)) => s,
        _ => {
            let prof_key = format!("profile:{}:jwt_secret", unverified.sub);
            state
                .credentials
                .get_secret(&prof_key)
                .map_err(|e| ApiError::internal(e.to_string()))?
                .ok_or_else(|| ApiError::unauthorized("Invalid token signature"))?
        }
    };

    let to_sign = format!("{}.{}", parts[0], parts[1]);
    let expected_sig = hmac_sha256(secret.as_bytes(), to_sign.as_bytes());
    let actual_sig = base64url_decode(parts[2])?;
    if actual_sig.as_slice() != expected_sig {
        return Err(ApiError::unauthorized("Invalid token signature"));
    }
    let now = chrono::Utc::now().timestamp();
    if unverified.exp < now {
        return Err(ApiError::unauthorized("Token expired"));
    }
    Ok(unverified)
}

pub fn generate_pkce_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64url_encode(&hash)
}

fn is_trusted_origin(orig: &str) -> bool {
    orig.starts_with("http://127.0.0.1")
        || orig.starts_with("http://localhost")
        || orig.starts_with("tauri://localhost")
        || orig.starts_with("http://tauri.localhost")
        || orig.starts_with("https://tauri.localhost")
}

pub fn validate_host_and_origin(parts: &Parts, is_mutating: bool) -> Result<(), ApiError> {
    let host = parts
        .headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::forbidden("Missing Host header"))?;

    let is_loopback_host =
        host.starts_with("127.0.0.1") || host.starts_with("localhost") || host.starts_with("[::1]");
    if !is_loopback_host {
        return Err(ApiError::forbidden("Invalid Host header"));
    }

    let origin = parts
        .headers
        .get(axum::http::header::ORIGIN)
        .and_then(|h| h.to_str().ok());

    if is_mutating {
        let orig = origin
            .ok_or_else(|| ApiError::forbidden("Missing Origin header on mutating request"))?;
        if !is_trusted_origin(orig) {
            return Err(ApiError::forbidden("Untrusted Origin header"));
        }
    } else if let Some(orig) = origin {
        if !is_trusted_origin(orig) {
            return Err(ApiError::forbidden("Untrusted Origin header"));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleIdTokenClaims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JwksKey {
    pub kty: String,
    pub alg: Option<String>,
    pub kid: String,
    pub n: String,
    pub e: String,
}

#[derive(Debug, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<JwksKey>,
}

pub async fn verify_google_id_token(
    id_token: &str,
    jwks_url: &str,
    expected_aud: Option<&str>,
    expected_nonce: Option<&str>,
) -> Result<GoogleIdTokenClaims, ApiError> {
    let header =
        decode_header(id_token).map_err(|_| ApiError::unauthorized("Invalid token header"))?;
    if header.alg != Algorithm::RS256 {
        return Err(ApiError::unauthorized(
            "Invalid token algorithm, must be RS256",
        ));
    }
    let kid = header
        .kid
        .ok_or_else(|| ApiError::unauthorized("Missing kid in token header"))?;

    let client = reqwest::Client::new();
    let jwks: JwksResponse = client
        .get(jwks_url)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to fetch JWKS: {e}")))?
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse JWKS: {e}")))?;

    let key = jwks
        .keys
        .into_iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| ApiError::unauthorized("Unknown kid in JWKS"))?;

    let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
        .map_err(|e| ApiError::unauthorized(format!("Invalid RSA key components: {e}")))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    if let Some(aud) = expected_aud {
        validation.set_audience(&[aud]);
    } else {
        validation.validate_aud = false;
    }
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);

    let token_data = decode::<GoogleIdTokenClaims>(id_token, &decoding_key, &validation)
        .map_err(|e| ApiError::unauthorized(format!("Token verification failed: {e}")))?;

    if let Some(nonce) = expected_nonce {
        if token_data.claims.nonce.as_deref() != Some(nonce) {
            return Err(ApiError::unauthorized("Nonce mismatch"));
        }
    }

    Ok(token_data.claims)
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub LocalSessionClaims);

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    crate::server::state::AppState: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = crate::server::state::AppState::from_ref(state);

        if let Some(cookie_hdr) = parts
            .headers
            .get(axum::http::header::COOKIE)
            .and_then(|h| h.to_str().ok())
        {
            for pair in cookie_hdr.split(';') {
                let mut parts_kv = pair.splitn(2, '=');
                if let (Some(k), Some(v)) = (parts_kv.next(), parts_kv.next()) {
                    if k.trim() == "session_id" {
                        let sid = v.trim();
                        let sessions = app_state.active_sessions.lock().await;
                        if let Some(s) = sessions.get(sid) {
                            if s.expires_at > chrono::Utc::now().timestamp() {
                                return Ok(AuthenticatedUser(LocalSessionClaims {
                                    sub: s.sub.clone(),
                                    tenant_id: s.tenant_id.clone(),
                                    workspace_id: s.workspace_id.clone(),
                                    email: s.email.clone(),
                                    display_name: s.display_name.clone(),
                                    plan: "GOOGLE".to_string(),
                                    exp: s.expires_at,
                                }));
                            }
                        }
                    }
                }
            }
        }

        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("Missing authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ApiError::unauthorized("Invalid authorization header format"))?;

        let claims = verify_token_with_state(token, &app_state)?;
        Ok(AuthenticatedUser(claims))
    }
}
