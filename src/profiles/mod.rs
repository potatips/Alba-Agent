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
