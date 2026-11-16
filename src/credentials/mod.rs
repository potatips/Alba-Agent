use std::collections::HashMap;
use std::process::Command;
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
        let prefix = format!("profile:{profile_id}:");
        guard.retain(|k, _| !k.starts_with(&prefix));
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
}

impl CredentialStore for MacosKeychainStore {
    fn get_secret(&self, key: &str) -> Result<Option<String>, CredentialStoreError> {
        let output = Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                &self.service,
                "-a",
                key,
                "-w",
            ])
            .output()
            .map_err(|e| CredentialStoreError::Io(e.to_string()))?;

        if output.status.success() {
            let s = String::from_utf8(output.stdout)
                .map_err(|e| CredentialStoreError::Platform(e.to_string()))?;
            Ok(Some(s.trim_end_matches(['\r', '\n']).to_string()))
        } else {
            Ok(None)
        }
    }

    fn set_secret(&self, key: &str, secret: &str) -> Result<(), CredentialStoreError> {
        let output = Command::new("security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                &self.service,
                "-a",
                key,
                "-w",
                secret,
            ])
            .output()
            .map_err(|e| CredentialStoreError::Io(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Err(CredentialStoreError::Platform(err))
        }
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
        let _ = self.set_secret(key, &secret);
        if let Some(actual) = self.get_secret(key)? {
            Ok(actual)
        } else {
            Ok(secret)
        }
    }

    fn delete_secret(&self, key: &str) -> Result<(), CredentialStoreError> {
        let _ = Command::new("security")
            .args(["delete-generic-password", "-s", &self.service, "-a", key])
            .output();
        Ok(())
    }

    fn delete_profile_secrets(&self, profile_id: &str) -> Result<(), CredentialStoreError> {
        let jwt_key = format!("profile:{profile_id}:jwt_secret");
        self.delete_secret(&jwt_key)?;
        Ok(())
    }
}

pub fn generate_secure_secret() -> String {
    use std::io::Read;
    let mut buf = [0u8; 32];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    } else {
        let u1 = uuid::Uuid::new_v4();
        let u2 = uuid::Uuid::new_v4();
        buf[..16].copy_from_slice(u1.as_bytes());
        buf[16..].copy_from_slice(u2.as_bytes());
    }
    let mut hex = String::with_capacity(64);
    for b in buf {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{:02x}", b);
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
