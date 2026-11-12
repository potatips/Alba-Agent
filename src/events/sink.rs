use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;

use crate::events::types::RuntimeEvent;

#[derive(Error, Debug)]
pub enum EventError {
    #[error("Failed to emit event: {0}")]
    EmitFailed(String),
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: RuntimeEvent) -> Result<(), EventError>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryEventSink {
    events: Arc<Mutex<Vec<RuntimeEvent>>>,
}

impl InMemoryEventSink {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn events(&self) -> Vec<RuntimeEvent> {
        let lock = self.events.lock().unwrap();
        lock.clone()
    }

    pub fn clear(&self) {
        let mut lock = self.events.lock().unwrap();
        lock.clear();
    }
}

impl EventSink for InMemoryEventSink {
    fn emit(&self, event: RuntimeEvent) -> Result<(), EventError> {
        let mut lock = self
            .events
            .lock()
            .map_err(|e| EventError::EmitFailed(format!("Mutex lock error: {}", e)))?;
        lock.push(event);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum JsonLinesTarget {
    Stdout,
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct JsonLinesEventSink {
    target: JsonLinesTarget,
    write_lock: Arc<Mutex<()>>,
}

impl JsonLinesEventSink {
    pub fn stdout() -> Self {
        Self {
            target: JsonLinesTarget::Stdout,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn to_file(path: impl Into<PathBuf>) -> Self {
        Self {
            target: JsonLinesTarget::File(path.into()),
            write_lock: Arc::new(Mutex::new(())),
        }
    }
}

impl EventSink for JsonLinesEventSink {
    fn emit(&self, event: RuntimeEvent) -> Result<(), EventError> {
        let line = serde_json::to_string(&event)
            .map_err(|e| EventError::EmitFailed(format!("JSON serialization error: {}", e)))?;

        let _guard = self
            .write_lock
            .lock()
            .map_err(|e| EventError::EmitFailed(format!("Mutex lock error: {}", e)))?;

        match &self.target {
            JsonLinesTarget::Stdout => {
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(handle, "{}", line)
                    .map_err(|e| EventError::EmitFailed(format!("Stdout write error: {}", e)))?;
                handle
                    .flush()
                    .map_err(|e| EventError::EmitFailed(format!("Stdout flush error: {}", e)))?;
            }
            JsonLinesTarget::File(path) => {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            EventError::EmitFailed(format!(
                                "Failed to create event directory: {}",
                                e
                            ))
                        })?;
                    }
                }
                let mut opts = OpenOptions::new();
                opts.create(true).append(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    opts.mode(0o600);
                }
                let mut file = opts
                    .open(path)
                    .map_err(|e| EventError::EmitFailed(format!("File open error: {}", e)))?;
                writeln!(file, "{}", line)
                    .map_err(|e| EventError::EmitFailed(format!("File write error: {}", e)))?;
                file.flush()
                    .map_err(|e| EventError::EmitFailed(format!("File flush error: {}", e)))?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn emit(&self, _event: RuntimeEvent) -> Result<(), EventError> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ConfiguredEventSink {
    Null(NullEventSink),
    JsonLines(JsonLinesEventSink),
}

impl EventSink for ConfiguredEventSink {
    fn emit(&self, event: RuntimeEvent) -> Result<(), EventError> {
        match self {
            Self::Null(s) => s.emit(event),
            Self::JsonLines(s) => s.emit(event),
        }
    }
}
