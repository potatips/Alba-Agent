import { useCallback, useEffect, useRef, useState } from "react";
import { ArtifactViewer } from "./components/ArtifactViewer";
import { type ChatMessage, ChatStream } from "./components/ChatStream";
import { CheckpointList } from "./components/CheckpointList";
import { ComposerBar } from "./components/ComposerBar";
import { FileTree } from "./components/FileTree";
import { ForkModal } from "./components/ForkModal";
import { type TerminalChunk } from "./components/SandboxTerminal";
import { SettingsModal } from "./components/SettingsModal";
import { Sidebar } from "./components/Sidebar";
import { reconstructModifiedFromUnifiedDiff } from "./diffPreview";
import {
  approveAction,
  forkTrajectory,
  getAuditLogs,
  getBranches,
  getCheckpoints,
  getDefaultWorkspace,
  listWorkspaceFiles,
  readWorkspaceFile,
  resumeRun,
  startRun,
  subscribeRuntimeEvents,
} from "./tauri";
import type {
  AuditLogDto,
  CheckpointDto,
  DiffSelection,
  PendingApprovalDto,
  RunOutcomeDto,
  RuntimeEvent,
  TrajectoryBranchDto,
  WorkspaceEntryDto,
} from "./types";

const INITIAL_MESSAGES: ChatMessage[] = [
  {
    id: "msg-init-1",
    sender: "assistant",
    content:
      "Bạn có muốn tôi cập nhật GOR v2 (Leaky-GOR) này để bạn test lại trên L4 xem nó có vượt kỷ lục Loss của A100 không?",
    timestamp: "6h ago",
  },
  {
    id: "msg-init-2",
    sender: "user",
    content: "oke cải tiến đi",
    timestamp: "6h ago",
  },
  {
    id: "msg-init-3",
    sender: "assistant",
    content: "Bản nâng cấp Toán học: GOR v2 (Smooth C∞ Modulation)",
    timestamp: "6h ago",
    showMath: true,
  },
];

export function App() {
  const [activeProject, setActiveProject] = useState("Create-LLM");
  const [activeTask, setActiveTask] = useState("Creating Folder LLM Model");
  const [messages, setMessages] = useState<ChatMessage[]>(INITIAL_MESSAGES);

  const [sidebarTab, setSidebarTab] = useState<"projects" | "files" | "checkpoints">("projects");
  const [editorMode, setEditorMode] = useState<"code" | "diff">("code");
  const [showSettings, setShowSettings] = useState(false);
  const [showRightPane, setShowRightPane] = useState(true);

  const [workspaceRoot, setWorkspaceRoot] = useState(".");
  const [apiBase, setApiBase] = useState("https://api.openai.com/v1");
  const [apiKey, setApiKey] = useState("");
  const [hasEnvApiKey, setHasEnvApiKey] = useState(false);
  const [model, setModel] = useState("Gemini 3.8 Flash High");
  const [authorId, setAuthorId] = useState("developer@local");
  const [approverId, setApproverId] = useState("reviewer@local");
  const [requireApprovalReads, setRequireApprovalReads] = useState(false);

  const [tenantId, setTenantId] = useState<string>("");
  const [workspaceId, setWorkspaceId] = useState<string>("");
  const [activeRunId, setActiveRunId] = useState<string>("");
  const [runStatus, setRunStatus] = useState<string>("idle");
  const [errorBanner, setErrorBanner] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [files, setFiles] = useState<WorkspaceEntryDto[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [selectedContent, setSelectedContent] = useState<string>("");
  const [activeDiff, setActiveDiff] = useState<DiffSelection | null>(null);

  const [checkpoints, setCheckpoints] = useState<CheckpointDto[]>([]);
  const [branches, setBranches] = useState<TrajectoryBranchDto[]>([]);
  const [events, setEvents] = useState<RuntimeEvent[]>([]);
  const [auditLogs, setAuditLogs] = useState<AuditLogDto[]>([]);
  const [pendingApproval, setPendingApproval] = useState<PendingApprovalDto | null>(null);
  const [terminalChunks, setTerminalChunks] = useState<TerminalChunk[]>([]);
  const [forkTarget, setForkTarget] = useState<CheckpointDto | null>(null);
  const [prompt, setPrompt] = useState("");

  const promptRef = useRef<HTMLTextAreaElement | null>(null);

  const appendTerminalSystem = useCallback((text: string) => {
    setTerminalChunks((prev) => [
      ...prev,
      {
        id: `${Date.now()}-${Math.random()}`,
        stream: "system",
        text,
      },
    ]);
  }, []);

  const refreshFiles = useCallback(async (root: string) => {
    setFilesLoading(true);
    try {
      const entries = await listWorkspaceFiles(root, 6);
      setFiles(entries);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setFilesLoading(false);
    }
  }, []);

  const openWorkspaceFile = useCallback(
    async (relativePath: string) => {
      try {
        const res = await readWorkspaceFile(workspaceRoot, relativePath);
        setSelectedPath(res.path);
        setSelectedContent(res.content);
        setEditorMode("code");
      } catch (err) {
        setErrorBanner(err instanceof Error ? err.message : String(err));
      }
    },
    [workspaceRoot],
  );

  const previewUnifiedDiff = useCallback(
    async (relativePath: string, unifiedDiff: string) => {
      let baseContent = "";
      try {
        const current = await readWorkspaceFile(workspaceRoot, relativePath);
        baseContent = current.content;
      } catch {
        baseContent = "";
      }
      const modified = reconstructModifiedFromUnifiedDiff(baseContent, unifiedDiff);
      setSelectedPath(relativePath);
      setSelectedContent(baseContent);
      setActiveDiff({
        path: relativePath,
        original: baseContent,
        modified,
        unifiedDiff,
      });
      setEditorMode("diff");
    },
    [workspaceRoot],
  );

  const applyOutcome = useCallback(
    async (outcome: RunOutcomeDto) => {
      setTenantId(outcome.tenant_id);
      setWorkspaceId(outcome.workspace_id);
      setActiveRunId(outcome.run_id);
      setRunStatus(outcome.status);
      setCheckpoints(outcome.checkpoints);
      setBranches(outcome.branches);
      setPendingApproval(outcome.pending_approval);

      if (outcome.message) {
        setMessages((prev) => [
          ...prev,
          {
            id: `msg-${Date.now()}`,
            sender: "assistant",
            content: outcome.message!,
            timestamp: "just now",
          },
        ]);
        appendTerminalSystem(`[run:${outcome.run_id.slice(0, 8)}] ${outcome.message}`);
      }

      if (outcome.pending_approval?.path && outcome.pending_approval.diff) {
        await previewUnifiedDiff(
          outcome.pending_approval.path,
          outcome.pending_approval.diff,
        );
      }

      try {
        const logs = await getAuditLogs(
          outcome.tenant_id,
          outcome.workspace_id,
          outcome.run_id,
        );
        setAuditLogs(logs);
      } catch {
        setAuditLogs([]);
      }

      await refreshFiles(workspaceRoot);
    },
    [appendTerminalSystem, previewUnifiedDiff, refreshFiles, workspaceRoot],
  );

  useEffect(() => {
    let active = true;
    getDefaultWorkspace().then((cfg) => {
      if (!active) return;
      setWorkspaceRoot(cfg.workspace_root);
      setApiBase(cfg.api_base);
      setModel(cfg.model || "Gemini 3.8 Flash High");
      setAuthorId(cfg.default_author_id);
      setApproverId(cfg.default_approver_id);
      setHasEnvApiKey(cfg.has_env_api_key);
      refreshFiles(cfg.workspace_root);
    });
    return () => {
      active = false;
    };
  }, [refreshFiles]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        promptRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    subscribeRuntimeEvents((ev) => {
      setEvents((prev) => [...prev.slice(-199), ev]);

      if (ev.type === "SANDBOX_TERMINAL_OUTPUT") {
        const chunk =
          typeof ev.payload.chunk === "string"
            ? ev.payload.chunk
            : typeof ev.payload.text === "string"
              ? ev.payload.text
              : "";
        const stream = typeof ev.payload.stream === "string" ? ev.payload.stream : "stdout";
        if (chunk) {
          setTerminalChunks((prev) => [
            ...prev.slice(-999),
            { id: ev.id, stream, text: chunk },
          ]);
        }
      } else if (ev.type === "SANDBOX_FILE_DIFF") {
        const path = typeof ev.payload.path === "string" ? ev.payload.path : null;
        const diff = typeof ev.payload.diff === "string" ? ev.payload.diff : null;
        const original =
          typeof ev.payload.original === "string" ? ev.payload.original : "";
        const modified =
          typeof ev.payload.modified === "string"
            ? ev.payload.modified
            : diff ?? "";
        if (path && diff) {
          setSelectedPath(path);
          setSelectedContent(modified);
          setActiveDiff({
            path,
            original,
            modified,
            unifiedDiff: diff,
          });
          setEditorMode("diff");
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const handleStartRun = async () => {
    if (!prompt.trim() || busy) return;
    const currentPrompt = prompt.trim();
    setPrompt("");
    setErrorBanner(null);
    setBusy(true);

    setMessages((prev) => [
      ...prev,
      {
        id: `user-${Date.now()}`,
        sender: "user",
        content: currentPrompt,
        timestamp: "just now",
      },
    ]);

    try {
      const outcome = await startRun({
        workspace_root: workspaceRoot,
        prompt: currentPrompt,
        model,
        api_base: apiBase,
        api_key: apiKey.trim() ? apiKey.trim() : undefined,
        author_id: authorId,
        require_approval_for_reads: requireApprovalReads,
        tenant_id: tenantId || undefined,
        workspace_id: workspaceId || undefined,
      });
      await applyOutcome(outcome);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleResumeRun = async () => {
    if (!tenantId || !workspaceId || !activeRunId) return;
    setErrorBanner(null);
    setBusy(true);
    try {
      const outcome = await resumeRun({
        tenant_id: tenantId,
        workspace_id: workspaceId,
        run_id: activeRunId,
        workspace_root: workspaceRoot,
        model,
        api_base: apiBase,
        api_key: apiKey.trim() ? apiKey.trim() : undefined,
        author_id: authorId,
      });
      await applyOutcome(outcome);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleDecision = async (decision: "allow_once" | "deny") => {
    if (!pendingApproval || !tenantId || !workspaceId || !activeRunId) return;
    setErrorBanner(null);
    setBusy(true);
    try {
      const outcome = await approveAction({
        tenant_id: tenantId,
        workspace_id: workspaceId,
        run_id: activeRunId,
        approval_id: pendingApproval.id,
        decision,
        approver_id: approverId.trim(),
        resume_after: true,
        workspace_root: workspaceRoot,
        model,
        api_base: apiBase,
        api_key: apiKey.trim() ? apiKey.trim() : undefined,
      });
      await applyOutcome(outcome);
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleForkSubmit = async (params: {
    branchLabel: string;
    overridePrompt?: string;
    overrideModel?: string;
    autoResume: boolean;
  }) => {
    if (!forkTarget || !tenantId || !workspaceId || !activeRunId) return;
    setErrorBanner(null);
    setBusy(true);
    try {
      const result = await forkTrajectory({
        tenant_id: tenantId,
        workspace_id: workspaceId,
        source_run_id: activeRunId,
        checkpoint_sequence: forkTarget.sequence,
        branch_label: params.branchLabel,
        override_prompt: params.overridePrompt,
        override_model: params.overrideModel,
        author_id: authorId,
        auto_resume: params.autoResume,
        workspace_root: workspaceRoot,
        api_base: apiBase,
        api_key: apiKey.trim() ? apiKey.trim() : undefined,
      });
      setForkTarget(null);
      if (result.outcome) {
        await applyOutcome(result.outcome);
      } else {
        setActiveRunId(result.branch.branch_run_id);
        const cps = await getCheckpoints(
          tenantId,
          workspaceId,
          result.branch.branch_run_id,
        );
        const brs = await getBranches(
          tenantId,
          workspaceId,
          result.branch.branch_run_id,
        );
        setCheckpoints(cps);
        setBranches(brs);
        setRunStatus("queued");
      }
      setSidebarTab("checkpoints");
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSwitchBranch = async (branchRunId: string) => {
    if (!tenantId || !workspaceId) return;
    setErrorBanner(null);
    try {
      const [cps, brs, logs] = await Promise.all([
        getCheckpoints(tenantId, workspaceId, branchRunId),
        getBranches(tenantId, workspaceId, branchRunId),
        getAuditLogs(tenantId, workspaceId, branchRunId),
      ]);
      setActiveRunId(branchRunId);
      setCheckpoints(cps);
      setBranches(brs);
      setAuditLogs(logs);
      const latest = cps[cps.length - 1];
      setRunStatus(latest?.status ?? "idle");
    } catch (err) {
      setErrorBanner(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSelectTask = (project: string, task: string) => {
    setActiveProject(project);
    setActiveTask(task);

    if (task.includes("API Tests")) {
      openWorkspaceFile("tests/phase1_phase2_tests.rs");
    } else if (task.includes("LLM Model")) {
      openWorkspaceFile("src/agent/engine.rs");
    } else if (task.includes("Chi Phí")) {
      openWorkspaceFile("deny.toml");
    } else if (task.includes("UX/UI")) {
      openWorkspaceFile("ui/src/App.tsx");
    } else if (task.includes("Markdown")) {
      openWorkspaceFile("README.md");
    }
  };

  const handleNewConversation = () => {
    setActiveProject("Alba-Project");
    setActiveTask("New Interactive Session");
    setMessages([
      {
        id: `msg-${Date.now()}`,
        sender: "assistant",
        content: "Alba Agent is ready. How can I help you architect, code, test, or verify today?",
        timestamp: "just now",
      },
    ]);
    promptRef.current?.focus();
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#09090b] text-zinc-100">
      <Sidebar
        activeProject={activeProject}
        activeTask={activeTask}
        onSelectTask={handleSelectTask}
        onNewConversation={handleNewConversation}
        onOpenSettings={() => setShowSettings(true)}
        sidebarTab={sidebarTab}
        onChangeSidebarTab={setSidebarTab}
        filesContent={
          <FileTree
            entries={files}
            selectedPath={selectedPath}
            loading={filesLoading}
            onSelectFile={openWorkspaceFile}
            onRefresh={() => refreshFiles(workspaceRoot)}
          />
        }
        checkpointsContent={
          <CheckpointList
            activeRunId={activeRunId || null}
            checkpoints={checkpoints}
            branches={branches}
            onSwitchRun={handleSwitchBranch}
            onRequestFork={(cp) => setForkTarget(cp)}
          />
        }
      />

      <div className="relative flex min-w-0 flex-[1] flex-col overflow-hidden">
        {errorBanner && (
          <div className="flex shrink-0 items-center justify-between border-b border-rose-500/40 bg-rose-950/80 px-4 py-2 text-xs text-rose-200">
            <span>{errorBanner}</span>
            <button
              type="button"
              onClick={() => setErrorBanner(null)}
              className="rounded px-2 py-0.5 text-[11px] text-rose-300 hover:bg-rose-900/60"
            >
              Dismiss
            </button>
          </div>
        )}

        <ChatStream
          activeProject={activeProject}
          activeTask={activeTask}
          runStatus={runStatus}
          messages={messages}
          pendingApproval={pendingApproval}
          approverId={approverId}
          busy={busy}
          onChangeApproverId={setApproverId}
          onAllowOnce={() => handleDecision("allow_once")}
          onDeny={() => handleDecision("deny")}
          onRestartToUpdate={handleResumeRun}
          onToggleSplitPane={() => setShowRightPane((v) => !v)}
        />

        <ComposerBar
          prompt={prompt}
          onChangePrompt={setPrompt}
          promptRef={promptRef}
          busy={busy}
          canResume={Boolean(activeRunId)}
          model={model}
          onChangeModel={setModel}
          activeProject={activeProject}
          onOpenSettings={() => setShowSettings(true)}
          onStartRun={handleStartRun}
          onResumeRun={handleResumeRun}
        />
      </div>

      {showRightPane && (
        <div className="flex min-w-0 flex-[1.1] flex-col overflow-hidden border-l border-zinc-800">
          <ArtifactViewer
            selectedPath={selectedPath}
            selectedContent={selectedContent}
            activeDiff={activeDiff}
            editorMode={editorMode}
            onChangeEditorMode={setEditorMode}
            onFileSaved={(p, content) => {
              if (p === selectedPath) setSelectedContent(content);
              refreshFiles(workspaceRoot);
            }}
            events={events}
            auditLogs={auditLogs}
            terminalChunks={terminalChunks}
            onClearTerminal={() => setTerminalChunks([])}
            onAppendTerminal={appendTerminalSystem}
            workspaceRoot={workspaceRoot}
            activeModel={model}
            onToggleCollapsePane={() => setShowRightPane(false)}
          />
        </div>
      )}

      <SettingsModal
        open={showSettings}
        onClose={() => setShowSettings(false)}
        workspaceRoot={workspaceRoot}
        onChangeWorkspaceRoot={setWorkspaceRoot}
        apiBase={apiBase}
        onChangeApiBase={setApiBase}
        apiKey={apiKey}
        onChangeApiKey={setApiKey}
        hasEnvApiKey={hasEnvApiKey}
        model={model}
        onChangeModel={setModel}
        authorId={authorId}
        onChangeAuthorId={setAuthorId}
        approverId={approverId}
        onChangeApproverId={setApproverId}
        requireApprovalReads={requireApprovalReads}
        onChangeRequireApprovalReads={setRequireApprovalReads}
      />

      {forkTarget && (
        <ForkModal
          checkpoint={forkTarget}
          defaultModel={model}
          busy={busy}
          onCancel={() => setForkTarget(null)}
          onSubmit={handleForkSubmit}
        />
      )}
    </div>
  );
}
