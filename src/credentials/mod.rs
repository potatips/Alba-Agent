use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Debug, thiserror::Error)]
pub enum CredentialStoreError {
    #[error("Platform error: {0}")]
    Platform(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Other error: {0}")]
    Other(String),
}

pub trait CredentialStore: Send + Sync {
    fn get_secret(&self, key: &str) -> Result<Option<String>, CredentialStoreError>;
    fn set_secret(&self, key: &str, secret: &str) -> Result<(), CredentialStoreError>;
    fn get_or_set_secret(
        &self,
        key: &str,
        generate: &dyn Fn() -> String,
    ) -> Result<String, CredentialStoreError>;
    fn delete_secret(&self, key: &str) -> Result<(), CredentialStoreError>;
    fn delete_profile_secrets(&self, profile_id: &str) -> Result<(), CredentialStoreError>;
}

static SHARED_MEMORY_STORE: OnceLock<Arc<RwLock<HashMap<String, String>>>> = OnceLock::new();

#[derive(Default, Clone)]
pub struct MemoryCredentialStore {
    map: Arc<RwLock<HashMap<String, String>>>,
}

impl MemoryCredentialStore {
    pub fn new() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn shared() -> Self {
        let map = SHARED_MEMORY_STORE
            .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
            .clone();
        Self { map }
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn get_secret(&self, key: &str) -> Result<Option<String>, CredentialStoreError> {
        let guard = self
            .map
            .read()
            .map_err(|e| CredentialStoreError::Other(e.to_string()))?;
        Ok(guard.get(key).cloned())
    }

    fn set_secret(&self, key: &str, secret: &str) -> Result<(), CredentialStoreError> {
        let mut guard = self
            .map
            .write()
            .map_err(|e| CredentialStoreError::Other(e.to_string()))?;
        guard.insert(key.to_string(), secret.to_string());
        Ok(())
    }

    fn get_or_set_secret(
        &self,
        key: &str,
        generate: &dyn Fn() -> String,
    ) -> Result<String, CredentialStoreError> {
        let mut guard = self
            .map
            .write()
            .map_err(|e| CredentialStoreError::Other(e.to_string()))?;
        if let Some(existing) = guard.get(key) {
            if !existing.trim().is_empty() {
                return Ok(existing.clone());
            }
        }
        let secret = generate();
        guard.insert(key.to_string(), secret.clone());
        Ok(secret)
    }

    fn delete_secret(&self, key: &str) -> Result<(), CredentialStoreError> {
        let mut guard = self
            .map
            .write()
            .map_err(|e| CredentialStoreError::Other(e.to_string()))?;
        guard.remove(key);
        Ok(())
    }

    fn delete_profile_secrets(&self, profile_id: &str) -> Result<(), CredentialStoreError> {
        let mut guard = self
            .map
            .write()
            .map_err(|e| CredentialStoreError::Other(e.to_string()))?;
        let prefix1 = format!("profile:{profile_id}:");
        let prefix2 = format!("account:{profile_id}:");
        guard.retain(|k, _| !k.starts_with(&prefix1) && !k.starts_with(&prefix2));
        Ok(())
    }
}

#[derive(Clone)]
pub struct MacosKeychainStore {
    service: String,
}

impl MacosKeychainStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn index_key(profile_id: &str) -> String {
        format!("profile:{profile_id}:__keys__")
    }

    fn parse_profile_id(key: &str) -> Option<&str> {
        let mut parts = key.split(':');
        let prefix = parts.next()?;
        if prefix == "profile" || prefix == "account" {
            let pid = parts.next()?;
            if let Some(item) = parts.next() {
                if !item.starts_with("__") {
                    return Some(pid);
                }
            }
        }
        None
    }

    fn set_raw_secret(&self, key: &str, secret: &str) -> Result<(), CredentialStoreError> {
        #[cfg(target_os = "macos")]
        {
            security_framework::passwords::set_generic_password(
                &self.service,
                key,
                secret.as_bytes(),
            )
            .map_err(|e| CredentialStoreError::Platform(format!("{}: {}", e.code(), e)))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _k = key;
            let _s = secret;
            Err(CredentialStoreError::Platform(
                "macOS Keychain only supported on macOS".to_string(),
            ))
        }
    }

    fn delete_raw_secret(&self, key: &str) -> Result<(), CredentialStoreError> {
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::delete_generic_password(&self.service, key) {
                Ok(()) => Ok(()),
                Err(err) if err.code() == -25300 => Ok(()),
                Err(err) => Err(CredentialStoreError::Platform(format!(
                    "{}: {}",
                    err.code(),
                    err
                ))),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _k = key;
            Err(CredentialStoreError::Platform(
                "macOS Keychain only supported on macOS".to_string(),
            ))
        }
    }

    fn register_profile_key(&self, key: &str) -> Result<(), CredentialStoreError> {
        if let Some(profile_id) = Self::parse_profile_id(key) {
            let idx_key = Self::index_key(profile_id);
            let existing = self.get_secret(&idx_key)?;
            let mut keys: Vec<String> = match existing {
                Some(s) if !s.trim().is_empty() => s.split(',').map(|k| k.to_string()).collect(),
                _ => Vec::new(),
            };
            if !keys.iter().any(|k| k == key) {
                keys.push(key.to_string());
                let serialized = keys.join(",");
                self.set_raw_secret(&idx_key, &serialized)?;
            }
        }
        Ok(())
    }

    fn unregister_profile_key(&self, key: &str) -> Result<(), CredentialStoreError> {
        if let Some(profile_id) = Self::parse_profile_id(key) {
            let idx_key = Self::index_key(profile_id);
            if let Some(existing) = self.get_secret(&idx_key)? {
                let keys: Vec<&str> = existing.split(',').filter(|k| *k != key).collect();
                if keys.is_empty() {
                    self.delete_raw_secret(&idx_key)?;
                } else {
                    let serialized = keys.join(",");
                    self.set_raw_secret(&idx_key, &serialized)?;
                }
            }
        }
        Ok(())
    }
}

impl CredentialStore for MacosKeychainStore {
    fn get_secret(&self, key: &str) -> Result<Option<String>, CredentialStoreError> {
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::get_generic_password(&self.service, key) {
                Ok(bytes) => {
                    let s = String::from_utf8(bytes)
                        .map_err(|e| CredentialStoreError::Platform(e.to_string()))?;
                    Ok(Some(s))
                }
                Err(err) if err.code() == -25300 => Ok(None),
                Err(err) => Err(CredentialStoreError::Platform(format!(
                    "{}: {}",
                    err.code(),
                    err
                ))),
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _k = key;
            Err(CredentialStoreError::Platform(
                "macOS Keychain only supported on macOS".to_string(),
            ))
        }
    }

    fn set_secret(&self, key: &str, secret: &str) -> Result<(), CredentialStoreError> {
        self.set_raw_secret(key, secret)?;
        self.register_profile_key(key)?;
        Ok(())
    }

    fn get_or_set_secret(
        &self,
        key: &str,
        generate: &dyn Fn() -> String,
    ) -> Result<String, CredentialStoreError> {
        if let Some(existing) = self.get_secret(key)? {
            if !existing.trim().is_empty() {
                return Ok(existing);
            }
        }
        let secret = generate();
        self.set_secret(key, &secret)?;
        Ok(secret)
    }

    fn delete_secret(&self, key: &str) -> Result<(), CredentialStoreError> {
        self.delete_raw_secret(key)?;
        self.unregister_profile_key(key)?;
        Ok(())
    }

    fn delete_profile_secrets(&self, profile_id: &str) -> Result<(), CredentialStoreError> {
        let idx_key = Self::index_key(profile_id);
        let mut keys_to_delete = Vec::new();
        keys_to_delete.push(format!("profile:{profile_id}:jwt_secret"));
        if let Some(existing) = self.get_secret(&idx_key)? {
            for k in existing.split(',') {
                let k_trim = k.trim();
                if !k_trim.is_empty() && !keys_to_delete.iter().any(|item| item == k_trim) {
                    keys_to_delete.push(k_trim.to_string());
                }
            }
        }
        for k in &keys_to_delete {
            self.delete_raw_secret(k)?;
        }
        self.delete_raw_secret(&idx_key)?;
        Ok(())
    }
}

pub fn generate_secure_secret() -> String {
    use std::io::Read;
    let mut buf = [0u8; 32];
    let read_ok = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_ok();
    if !read_ok {
        let u1 = uuid::Uuid::new_v4();
        let u2 = uuid::Uuid::new_v4();
        buf[..16].copy_from_slice(u1.as_bytes());
        buf[16..].copy_from_slice(u2.as_bytes());
    }
    let mut hex = String::with_capacity(64);
    for b in buf {
        use std::fmt::Write;
        if write!(&mut hex, "{:02x}", b).is_err() {
            break;
        }
    }
    hex
}

pub fn default_credential_store() -> Box<dyn CredentialStore> {
    if std::env::var("ALBA_USE_KEYCHAIN")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        #[cfg(target_os = "macos")]
        {
            return Box::new(MacosKeychainStore::new("alba"));
        }
    }
    Box::new(MemoryCredentialStore::shared())
}
