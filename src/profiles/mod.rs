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
}

impl ProviderConnection {
    pub fn to_credential_response(&self) -> CredentialResponse {
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
