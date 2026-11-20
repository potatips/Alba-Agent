use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub is_local: bool,
    pub active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConnectionMetadata {
    pub id: String,
    pub profile_id: String,
    pub provider: String,
    pub auth_type: String,
    pub account_label: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoogleAccountRecord {
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub picture: Option<String>,
    pub tenant_id: String,
    pub workspace_id: String,
    pub active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConnection {
    pub id: String,
    pub profile_id: String,
    pub provider: String,
    pub preset_id: String,
    pub auth_type: String,
    pub account_label: Option<String>,
    pub status: String,
    pub base_url: Option<String>,
    pub is_default: bool,
    pub available_models: Vec<String>,
    pub default_models: serde_json::Value,
    pub settings: serde_json::Value,
    pub masked_key: Option<String>,
    pub safe_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialResponse {
    pub id: String,
    pub preset_id: String,
    pub provider: String,
    pub masked_key: Option<String>,
    pub status: String,
    pub base_url: Option<String>,
    pub is_default: bool,
    pub available_models: Vec<String>,
    pub default_models: serde_json::Value,
    pub settings: serde_json::Value,
    pub balance: Option<serde_json::Value>,
    pub safe_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

pub const OFFICIAL_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const OFFICIAL_OPENAI_HOST: &str = "api.openai.com";

pub const OFFICIAL_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const OFFICIAL_ANTHROPIC_HOST: &str = "api.anthropic.com";

pub const OFFICIAL_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";
pub const OFFICIAL_GEMINI_HOST: &str = "generativelanguage.googleapis.com";

pub const OFFICIAL_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";
pub const OFFICIAL_DEEPSEEK_HOST: &str = "api.deepseek.com";

pub fn is_official_preset(preset_id: &str) -> bool {
    matches!(
        preset_id,
        "openai" | "anthropic" | "google_gemini" | "deepseek"
    )
}

pub fn is_local_preset(preset_id: &str) -> bool {
    matches!(preset_id, "local" | "ollama")
}

pub fn is_custom_preset(preset_id: &str) -> bool {
    matches!(
        preset_id,
        "custom_openai" | "custom" | "shopaikey" | "openrouter"
    ) || preset_id.starts_with("custom_")
}

pub fn official_preset_base_url(preset_id: &str) -> Option<&'static str> {
    match preset_id {
        "openai" => Some(OFFICIAL_OPENAI_BASE_URL),
        "anthropic" => Some(OFFICIAL_ANTHROPIC_BASE_URL),
        "google_gemini" => Some(OFFICIAL_GEMINI_BASE_URL),
        "deepseek" => Some(OFFICIAL_DEEPSEEK_BASE_URL),
        _ => None,
    }
}

pub fn official_preset_host(preset_id: &str) -> Option<&'static str> {
    match preset_id {
        "openai" => Some(OFFICIAL_OPENAI_HOST),
        "anthropic" => Some(OFFICIAL_ANTHROPIC_HOST),
        "google_gemini" => Some(OFFICIAL_GEMINI_HOST),
        "deepseek" => Some(OFFICIAL_DEEPSEEK_HOST),
        _ => None,
    }
}

pub fn is_loopback_host(host_str: &str) -> bool {
    if host_str == "localhost" {
        return true;
    }
    if let Ok(ip) = host_str.parse::<std::net::IpAddr>() {
        match ip {
            std::net::IpAddr::V4(v4) => v4.is_loopback(),
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6
                        .to_ipv4_mapped()
                        .map(|v4| v4.is_loopback())
                        .unwrap_or(false)
            }
        }
    } else {
        false
    }
}

pub fn validate_local_base_url(raw_url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw_url).map_err(|e| format!("invalid base_url format: {}", e))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("local base_url must use http or https".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("credentials are not permitted in base_url".to_string());
    }
    let host_str = parsed
        .host_str()
        .ok_or_else(|| "base_url must include a host".to_string())?;

    if !is_loopback_host(host_str) {
        return Err(
            "local preset only permits loopback hosts (127.0.0.1, localhost, ::1)".to_string(),
        );
    }

    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

pub fn validate_custom_base_url(raw_url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw_url).map_err(|e| format!("invalid base_url format: {}", e))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("credentials are not permitted in base_url".to_string());
    }
    let scheme = parsed.scheme();
    if scheme != "https" {
        return Err("custom presets require public HTTPS endpoints; HTTP and loopback hosts are only permitted for the 'local' preset".to_string());
    }
    let host_str = parsed
        .host_str()
        .ok_or_else(|| "base_url must include a host".to_string())?;

    if is_loopback_host(host_str) {
        return Err("loopback hosts (127.0.0.1, localhost) are only permitted for the 'local' preset; custom presets require public HTTPS endpoints".to_string());
    }

    if let Ok(ip) = host_str.parse::<std::net::IpAddr>() {
        let is_private = match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_unique_local()
                    || v6.is_unicast_link_local()
                    || v6.is_unspecified()
                    || v6
                        .to_ipv4_mapped()
                        .map(|v4| v4.is_private() || v4.is_link_local() || v4.is_unspecified())
                        .unwrap_or(false)
            }
        };
        if is_private {
            return Err("private network hosts are not permitted for custom base_url; only public HTTPS endpoints are allowed".to_string());
        }
    } else if host_str.ends_with(".local")
        || host_str.ends_with(".internal")
        || host_str.ends_with(".lan")
    {
        return Err("internal network domains are not permitted".to_string());
    }

    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

pub fn resolve_and_verify_provider_endpoint(
    preset_id: &str,
    stored_base_url: Option<&str>,
) -> Result<String, String> {
    if is_official_preset(preset_id) {
        if let Some(stored) = stored_base_url {
            let parsed =
                url::Url::parse(stored).map_err(|e| format!("stored base_url corrupted: {}", e))?;
            let expected_host = official_preset_host(preset_id).unwrap();
            if parsed.host_str() != Some(expected_host) {
                return Err(format!(
                    "host mismatch for official preset '{}': expected '{}', got '{:?}'",
                    preset_id,
                    expected_host,
                    parsed.host_str()
                ));
            }
        }
        Ok(official_preset_base_url(preset_id).unwrap().to_string())
    } else if is_local_preset(preset_id) {
        let url_str = stored_base_url.unwrap_or("http://127.0.0.1:11434/v1");
        validate_local_base_url(url_str)
    } else if is_custom_preset(preset_id) {
        let url_str =
            stored_base_url.ok_or_else(|| "custom preset requires base_url".to_string())?;
        validate_custom_base_url(url_str)
    } else {
        Err(format!(
            "unknown or unsupported provider preset: {}",
            preset_id
        ))
    }
}

impl ProviderConnection {
    pub fn to_credential_response(&self) -> CredentialResponse {
        let warning = if is_custom_preset(&self.preset_id) {
            self.base_url.as_ref().and_then(|u| {
                url::Url::parse(u).ok().and_then(|p| {
                    p.host_str()
                        .map(|h| format!("khóa API sẽ được gửi tới {}", h))
                })
            })
        } else {
            None
        };
        CredentialResponse {
            id: self.id.clone(),
            preset_id: self.preset_id.clone(),
            provider: self.provider.clone(),
            masked_key: self.masked_key.clone(),
            status: self.status.clone(),
            base_url: self.base_url.clone(),
            is_default: self.is_default,
            available_models: self.available_models.clone(),
            default_models: self.default_models.clone(),
            settings: self.settings.clone(),
            balance: None,
            safe_error: self.safe_error.clone(),
            warning,
        }
    }
}

pub fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.len() <= 8 {
        return "********".to_string();
    }
    let prefix_len = if trimmed.starts_with("sk-ant-") {
        7
    } else if trimmed.starts_with("sk-") {
        3
    } else if trimmed.starts_with("AIza") {
        4
    } else {
        3
    };
    let suffix_len = 4;
    if trimmed.len() <= prefix_len + suffix_len {
        format!(
            "{}...{}",
            &trimmed[..prefix_len],
            &trimmed[trimmed.len() - 2..]
        )
    } else {
        format!(
            "{}...{}",
            &trimmed[..prefix_len],
            &trimmed[trimmed.len() - suffix_len..]
        )
    }
}
