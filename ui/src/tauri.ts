import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ApproveActionPayload,
  AuditLogDto,
  CheckpointDto,
  DefaultWorkspaceDto,
  ForkResultDto,
  ForkTrajectoryPayload,
  ResumeRunPayload,
  RunOutcomeDto,
  RuntimeEvent,
  StartRunPayload,
  TrajectoryBranchDto,
  WorkspaceEntryDto,
  WorkspaceFileContentDto,
} from "./types";

export function isTauriEnvironment(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function getDefaultWorkspace(): Promise<DefaultWorkspaceDto> {
  if (!isTauriEnvironment()) {
    try {
      const res = await fetch("/api/workspace/default");
      if (res.ok) {
        return (await res.json()) as DefaultWorkspaceDto;
      }
    } catch {
    }
    return {
      workspace_root: ".",
      api_base: "https://api.openai.com/v1",
      model: "Gemini 3.8 Flash High",
      default_author_id: "developer@local",
      default_approver_id: "reviewer@local",
      has_env_api_key: false,
    };
  }
  return invoke<DefaultWorkspaceDto>("get_default_workspace");
}

export async function startRun(request: StartRunPayload): Promise<RunOutcomeDto> {
  if (!isTauriEnvironment()) {
    const res = await fetch("/api/run/start", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    if (!res.ok) {
      throw new Error(`Run failed with status ${res.status}`);
    }
    return (await res.json()) as RunOutcomeDto;
  }
  return invoke<RunOutcomeDto>("start_run", { request });
}

export async function resumeRun(request: ResumeRunPayload): Promise<RunOutcomeDto> {
  if (!isTauriEnvironment()) {
    return {
      tenant_id: request.tenant_id,
      workspace_id: request.workspace_id,
      run_id: request.run_id,
      status: "completed",
      message: "Resumed and completed pipeline execution successfully.",
      approval_id: null,
      pending_approval: null,
      checkpoints: [],
      branches: [],
    };
  }
  return invoke<RunOutcomeDto>("resume_run", { request });
}

export async function approveAction(request: ApproveActionPayload): Promise<RunOutcomeDto> {
  if (!isTauriEnvironment()) {
    if (request.approver_id === "developer@local") {
      throw new Error("Separation of duties: approver_id must differ from author_id");
    }
    return {
      tenant_id: request.tenant_id,
      workspace_id: request.workspace_id,
      run_id: request.run_id,
      status: "completed",
      message: `Action ${request.approval_id} approved (${request.decision}) and executed.`,
      approval_id: null,
      pending_approval: null,
      checkpoints: [],
      branches: [],
    };
  }
  return invoke<RunOutcomeDto>("approve_action", { request });
}

export async function getCheckpoints(
  tenantId: string,
  workspaceId: string,
  runId: string,
  dbPath?: string,
): Promise<CheckpointDto[]> {
  if (!isTauriEnvironment()) {
    return [
      {
        id: `cp-${runId}-1`,
        tenant_id: tenantId,
        workspace_id: workspaceId,
        run_id: runId,
        sequence: 1,
        status: "completed",
        history_len: 2,
        active_tool_name: "list_dir",
        pending_tool_count: 0,
        pending_approval_id: null,
        author_id: "developer@local",
        created_at: new Date().toISOString(),
        preview: "Read workspace directory layout",
      },
    ];
  }
  return invoke<CheckpointDto[]>("get_checkpoints", {
    tenantId,
    workspaceId,
    runId,
    dbPath,
  });
}

export async function getBranches(
  tenantId: string,
  workspaceId: string,
  runId: string,
  dbPath?: string,
): Promise<TrajectoryBranchDto[]> {
  if (!isTauriEnvironment()) {
    return [];
  }
  return invoke<TrajectoryBranchDto[]>("get_branches", {
    tenantId,
    workspaceId,
    runId,
    dbPath,
  });
}

export async function getAuditLogs(
  tenantId: string,
  workspaceId: string,
  runId: string,
  dbPath?: string,
): Promise<AuditLogDto[]> {
  if (!isTauriEnvironment()) {
    return [
      {
        id: `audit-${Date.now()}`,
        event_type: "run.initiated",
        actor: "developer@local",
        details_json: JSON.stringify({ run_id: runId, mode: "interactive" }),
        created_at: new Date().toISOString(),
      },
    ];
  }
  return invoke<AuditLogDto[]>("get_audit_logs", {
    tenantId,
    workspaceId,
    runId,
    dbPath,
  });
}

export async function forkTrajectory(request: ForkTrajectoryPayload): Promise<ForkResultDto> {
  if (!isTauriEnvironment()) {
    const branchRunId = `fork-${Date.now().toString(16)}`;
    const branch: TrajectoryBranchDto = {
      id: `br-${Date.now()}`,
      parent_run_id: request.source_run_id,
      parent_sequence: request.checkpoint_sequence,
      branch_run_id: branchRunId,
      branch_label: request.branch_label,
      override_prompt: request.override_prompt || null,
      override_model: request.override_model || null,
      created_at: new Date().toISOString(),
    };
    return {
      branch,
      initial_checkpoint: {
        id: `cp-${branchRunId}-0`,
        tenant_id: request.tenant_id,
        workspace_id: request.workspace_id,
        run_id: branchRunId,
        sequence: request.checkpoint_sequence,
        status: "forked",
        history_len: 2,
        active_tool_name: null,
        pending_tool_count: 0,
        pending_approval_id: null,
        author_id: request.author_id,
        created_at: new Date().toISOString(),
        preview: `Forked from seq #${request.checkpoint_sequence}`,
      },
      outcome: {
        tenant_id: request.tenant_id,
        workspace_id: request.workspace_id,
        run_id: branchRunId,
        status: "completed",
        message: `Trajectory forked to branch "${request.branch_label}" and finished.`,
        approval_id: null,
        pending_approval: null,
        checkpoints: [],
        branches: [branch],
      },
    };
  }
  return invoke<ForkResultDto>("fork_trajectory", { request });
}

export async function listWorkspaceFiles(
  workspaceRoot: string,
  maxDepth?: number,
): Promise<WorkspaceEntryDto[]> {
  if (!isTauriEnvironment()) {
    try {
      const url = `/api/workspace/files?root=${encodeURIComponent(workspaceRoot)}&depth=${maxDepth ?? 5}`;
      const res = await fetch(url);
      if (res.ok) {
        return (await res.json()) as WorkspaceEntryDto[];
      }
    } catch {
      return [];
    }
    return [];
  }
  return invoke<WorkspaceEntryDto[]>("list_workspace_files", {
    workspaceRoot,
    maxDepth,
  });
}

export async function readWorkspaceFile(
  workspaceRoot: string,
  path: string,
): Promise<WorkspaceFileContentDto> {
  if (!isTauriEnvironment()) {
    const url = `/api/workspace/file?path=${encodeURIComponent(path)}&root=${encodeURIComponent(workspaceRoot)}`;
    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`Failed to read file ${path}`);
    }
    return (await res.json()) as WorkspaceFileContentDto;
  }
  return invoke<WorkspaceFileContentDto>("read_workspace_file", {
    workspaceRoot,
    path,
  });
}

export async function saveWorkspaceFile(
  path: string,
  content: string,
): Promise<{ ok: boolean }> {
  const res = await fetch("/api/workspace/file", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path, content }),
  });
  if (!res.ok) {
    throw new Error(`Failed to save file: ${res.statusText}`);
  }
  return (await res.json()) as { ok: boolean };
}

export async function executeTerminalCommand(
  command: string,
  cwd?: string,
): Promise<{ exit_code: number; stdout: string; stderr: string }> {
  const res = await fetch("/api/terminal/exec", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ command, cwd }),
  });
  if (!res.ok) {
    throw new Error(`Command execution error: ${res.statusText}`);
  }
  return (await res.json()) as { exit_code: number; stdout: string; stderr: string };
}

export async function executeNotebookBenchmark(): Promise<{ ok: boolean; benchmark?: Record<string, unknown>; error?: string }> {
  const res = await fetch("/api/notebook/run", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
  });
  return (await res.json()) as { ok: boolean; benchmark?: Record<string, unknown>; error?: string };
}

export async function subscribeRuntimeEvents(
  handler: (event: RuntimeEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriEnvironment()) {
    const listener = (e: Event) => {
      const customEvent = e as CustomEvent<RuntimeEvent>;
      if (customEvent.detail) {
        handler(customEvent.detail);
      }
    };
    window.addEventListener("nexus-runtime-event", listener);
    return () => {
      window.removeEventListener("nexus-runtime-event", listener);
    };
  }
  return listen<RuntimeEvent>("nexus-runtime-event", (event) => {
    handler(event.payload);
  });
}
