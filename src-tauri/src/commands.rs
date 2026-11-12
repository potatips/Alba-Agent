use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::service::{
    ApproveActionPayload, AuditLogDto, CheckpointDto, DefaultWorkspaceDto, DesktopEngineService,
    ForkResultDto, ForkTrajectoryPayload, ResumeRunPayload, RunOutcomeDto, StartRunPayload,
    TrajectoryBranchDto, WorkspaceEntryDto, WorkspaceFileContentDto,
};

pub struct DesktopAppState {
    pub service: Arc<DesktopEngineService>,
}

pub fn build_tauri_event_callback(
    app: AppHandle,
) -> Arc<dyn Fn(nexus_albaai::events::types::RuntimeEvent) + Send + Sync> {
    Arc::new(move |event| {
        let topic = event.event_type.as_str().to_string();
        let _ = app.emit("alba://runtime-event", &event);
        let _ = app.emit(&topic, &event);
    })
}

#[tauri::command]
pub fn get_default_workspace(state: State<'_, DesktopAppState>) -> DefaultWorkspaceDto {
    state.service.get_default_workspace()
}

#[tauri::command]
pub fn list_workspace_files(
    state: State<'_, DesktopAppState>,
    workspace_root: String,
    max_depth: Option<usize>,
) -> Result<Vec<WorkspaceEntryDto>, String> {
    state
        .service
        .list_workspace_files(&workspace_root, max_depth)
}

#[tauri::command]
pub fn read_workspace_file(
    state: State<'_, DesktopAppState>,
    workspace_root: String,
    path: String,
) -> Result<WorkspaceFileContentDto, String> {
    state.service.read_workspace_file(&workspace_root, &path)
}

#[tauri::command]
pub async fn start_run(
    state: State<'_, DesktopAppState>,
    request: StartRunPayload,
) -> Result<RunOutcomeDto, String> {
    state.service.start_run(request).await
}

#[tauri::command]
pub async fn resume_run(
    state: State<'_, DesktopAppState>,
    request: ResumeRunPayload,
) -> Result<RunOutcomeDto, String> {
    state.service.resume_run(request).await
}

#[tauri::command]
pub async fn approve_action(
    state: State<'_, DesktopAppState>,
    request: ApproveActionPayload,
) -> Result<RunOutcomeDto, String> {
    state.service.approve_action(request).await
}

#[tauri::command]
pub async fn get_checkpoints(
    state: State<'_, DesktopAppState>,
    tenant_id: String,
    workspace_id: String,
    run_id: String,
    db_path: Option<String>,
) -> Result<Vec<CheckpointDto>, String> {
    state
        .service
        .get_checkpoints(&tenant_id, &workspace_id, &run_id, db_path.as_deref())
        .await
}

#[tauri::command]
pub async fn get_branches(
    state: State<'_, DesktopAppState>,
    tenant_id: String,
    workspace_id: String,
    run_id: String,
    db_path: Option<String>,
) -> Result<Vec<TrajectoryBranchDto>, String> {
    state
        .service
        .get_branches(&tenant_id, &workspace_id, &run_id, db_path.as_deref())
        .await
}

#[tauri::command]
pub async fn get_audit_logs(
    state: State<'_, DesktopAppState>,
    tenant_id: String,
    workspace_id: String,
    run_id: String,
    db_path: Option<String>,
) -> Result<Vec<AuditLogDto>, String> {
    state
        .service
        .get_audit_logs(&tenant_id, &workspace_id, &run_id, db_path.as_deref())
        .await
}

#[tauri::command]
pub async fn fork_trajectory(
    state: State<'_, DesktopAppState>,
    request: ForkTrajectoryPayload,
) -> Result<ForkResultDto, String> {
    state.service.fork_trajectory(request).await
}
