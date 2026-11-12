export type RuntimeEventType =
  | "TOOL_INVOKED"
  | "TOOL_COMPLETED"
  | "APPROVAL_REQUIRED"
  | "APPROVAL_RESOLVED"
  | "FLOW_STATE_CHANGED"
  | "SANDBOX_TERMINAL_OUTPUT"
  | "SANDBOX_FILE_DIFF"
  | "TRAJECTORY_FORKED"
  | "RUN_COMPLETED"
  | "RUN_FAILED";

export interface RuntimeEvent {
  id: string;
  tenant_id: string;
  workspace_id: string;
  run_id: string;
  node_id: string | null;
  type: RuntimeEventType;
  occurred_at: string;
  payload: Record<string, unknown>;
}

export interface PendingApprovalDto {
  id: string;
  tool_name: string;
  action_description: string;
  path: string | null;
  program: string | null;
  args: string[] | null;
  diff: string | null;
  author_id: string | null;
  status: string;
  created_at: string;
}

export interface CheckpointDto {
  id: string;
  tenant_id: string;
  workspace_id: string;
  run_id: string;
  sequence: number;
  status: string;
  history_len: number;
  active_tool_name: string | null;
  pending_tool_count: number;
  pending_approval_id: string | null;
  author_id: string | null;
  created_at: string;
  preview: string;
}

export interface TrajectoryBranchDto {
  id: string;
  parent_run_id: string;
  parent_sequence: number;
  branch_run_id: string;
  branch_label: string;
  override_prompt: string | null;
  override_model: string | null;
  created_at: string;
}

export interface RunOutcomeDto {
  tenant_id: string;
  workspace_id: string;
  run_id: string;
  status: "completed" | "paused_for_approval" | "cancelled" | "failed";
  message: string | null;
  approval_id: string | null;
  pending_approval: PendingApprovalDto | null;
  checkpoints: CheckpointDto[];
  branches: TrajectoryBranchDto[];
}

export interface ForkResultDto {
  branch: TrajectoryBranchDto;
  initial_checkpoint: CheckpointDto;
  outcome: RunOutcomeDto | null;
}

export interface WorkspaceEntryDto {
  path: string;
  entry_type: "file" | "dir";
  size_bytes: number;
  modified: string | null;
}

export interface WorkspaceFileContentDto {
  path: string;
  content: string;
  byte_len: number;
}

export interface AuditLogDto {
  id: string;
  event_type: string;
  actor: string | null;
  details_json: string;
  created_at: string;
}

export interface DefaultWorkspaceDto {
  workspace_root: string;
  api_base: string;
  model: string;
  default_author_id: string;
  default_approver_id: string;
  has_env_api_key: boolean;
}

export interface StartRunPayload {
  workspace_root: string;
  prompt: string;
  model?: string;
  api_base?: string;
  api_key?: string;
  author_id: string;
  require_approval_for_reads?: boolean;
  tenant_id?: string;
  workspace_id?: string;
  run_id?: string;
  db_path?: string;
}

export interface ResumeRunPayload {
  tenant_id: string;
  workspace_id: string;
  run_id: string;
  workspace_root: string;
  model?: string;
  api_base?: string;
  api_key?: string;
  author_id?: string;
  db_path?: string;
}

export interface ApproveActionPayload {
  tenant_id: string;
  workspace_id: string;
  run_id: string;
  approval_id: string;
  decision: "allow_once" | "deny";
  approver_id: string;
  db_path?: string;
  resume_after?: boolean;
  workspace_root?: string;
  model?: string;
  api_base?: string;
  api_key?: string;
}

export interface ForkTrajectoryPayload {
  tenant_id: string;
  workspace_id: string;
  source_run_id: string;
  checkpoint_sequence: number;
  branch_label: string;
  override_prompt?: string;
  override_model?: string;
  author_id: string;
  db_path?: string;
  auto_resume?: boolean;
  workspace_root?: string;
  api_base?: string;
  api_key?: string;
}

export interface DiffSelection {
  path: string;
  original: string;
  modified: string;
  unifiedDiff: string;
}
