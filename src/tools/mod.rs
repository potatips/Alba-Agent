use std::path::Path;
use std::sync::Arc;

use crate::events::sink::EventSink;

pub mod dir_list;
pub mod file_read;
pub mod file_write;
pub mod path_guard;
pub mod run_cmd;
pub mod types;

pub use dir_list::*;
pub use file_read::*;
pub use file_write::*;
pub use path_guard::*;
pub use run_cmd::*;
pub use types::*;

pub fn register_workspace_tools(
    registry: &mut ToolRegistry,
    workspace_root: impl AsRef<Path>,
    event_sink: Option<Arc<dyn EventSink>>,
) {
    let root = workspace_root.as_ref();
    registry.register(Arc::new(ReadFileTool::new(root)));
    registry.register(Arc::new(ListDirTool::new(root)));
    let mut write_tool = WriteFileTool::new(root);
    let mut cmd_tool = RunCommandTool::new(root);
    if let Some(sink) = event_sink {
        write_tool = write_tool.with_event_sink(Arc::clone(&sink));
        cmd_tool = cmd_tool.with_event_sink(sink);
    }
    registry.register(Arc::new(write_tool));
    registry.register(Arc::new(cmd_tool));
}
