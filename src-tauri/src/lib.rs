pub mod commands;
pub mod service;

use std::sync::Arc;

use tauri::Manager;

use crate::commands::{
    approve_action, build_tauri_event_callback, fork_trajectory, get_audit_logs, get_branches,
    get_checkpoints, get_default_workspace, list_workspace_files, read_workspace_file, resume_run,
    start_run, DesktopAppState,
};
use crate::service::DesktopEngineService;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let callback = build_tauri_event_callback(app.handle().clone());
            let service = Arc::new(DesktopEngineService::new(Some(callback)));
            app.manage(DesktopAppState { service });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_workspace,
            list_workspace_files,
            read_workspace_file,
            start_run,
            resume_run,
            approve_action,
            get_checkpoints,
            get_branches,
            get_audit_logs,
            fork_trajectory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Alba Agent desktop application");
}
