use std::path::Path;
use std::sync::Arc;

use crate::checkpoints::sqlite::SqliteCheckpointStore;
use crate::events::sink::EventSink;

pub mod dir_list;
pub mod file_read;
pub mod file_write;
pub mod path_guard;
pub mod run_cmd;
pub mod types;
pub mod video;

pub use dir_list::*;
pub use file_read::*;
pub use file_write::*;
pub use path_guard::*;
pub use run_cmd::*;
pub use types::*;
pub use video::*;

pub fn register_workspace_tools(
    registry: &mut ToolRegistry,
    workspace_root: impl AsRef<Path>,
    event_sink: Option<Arc<dyn EventSink>>,
) {
    let root = workspace_root.as_ref();
    let mut audit_store = None;
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(ext) = p.extension() {
                if (ext == "db" || ext == "sqlite") && p.is_file() {
                    if let Ok(s) = SqliteCheckpointStore::new(&p) {
                        audit_store = Some(Arc::new(s));
                        break;
                    }
                }
            }
        }
    }
    if audit_store.is_none() {
        let alba_db = root.join(".alba").join("checkpoints.db");
        if alba_db.is_file() {
            if let Ok(s) = SqliteCheckpointStore::new(&alba_db) {
                audit_store = Some(Arc::new(s));
            }
        }
    }
    register_workspace_tools_with_audit(registry, root, event_sink, audit_store);
}

pub fn register_workspace_tools_with_audit(
    registry: &mut ToolRegistry,
    workspace_root: impl AsRef<Path>,
    event_sink: Option<Arc<dyn EventSink>>,
    audit_store: Option<Arc<SqliteCheckpointStore>>,
) {
    let root = workspace_root.as_ref();
    let mut read_tool = ReadFileTool::new(root);
    let mut write_tool = WriteFileTool::new(root);
    let mut cmd_tool = RunCommandTool::new(root);

    if let Some(ref store) = audit_store {
        read_tool = read_tool.with_audit_store(store.clone());
        write_tool = write_tool.with_audit_store(store.clone());
        cmd_tool = cmd_tool.with_audit_store(store.clone());
    }

    if let Some(sink) = event_sink {
        write_tool = write_tool.with_event_sink(Arc::clone(&sink));
        cmd_tool = cmd_tool.with_event_sink(sink);
    }

    registry.register(Arc::new(read_tool));
    registry.register(Arc::new(ListDirTool::new(root)));
    registry.register(Arc::new(write_tool));
    registry.register(Arc::new(cmd_tool));
    registry.register(Arc::new(FfprobeInfoTool::new(root)));
}
