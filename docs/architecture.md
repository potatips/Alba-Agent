# Alba Agent Architecture

## 1. Zero-Dependency Local Stack Overview

**Alba Agent** combines a native Rust agent and workflow engine (`nexus-albaai`), OS-native process sandboxing, embedded SQLite WAL state persistence, and a Tauri v2 desktop IDE shell (`alba-agent-desktop` + React 19 UI in `ui/`):

```text
+-------------------------------------------------------------------------+
|                    Alba Agent Desktop Shell (Tauri v2)                  |
|  ui/: React 19 + Tailwind CSS v4 + Monaco Editor (Diff) + Xterm.js      |
|  - Left Pane: FileTree, Time-Travel Checkpoints, ForkModal              |
|  - Center Pane: Code/Diff Editor (Monaco) & Sandbox Terminal (Xterm)    |
|  - Right Pane: Interactive ApprovalCard & Runtime EventTimeline         |
|  - Top Bar: Cmd/Ctrl+K Prompt ComposerBar                               |
+------------------------------------+------------------------------------+
                                     | Tauri Async IPC + alba://runtime-event
                                     v
+-------------------------------------------------------------------------+
|                  Desktop Service Bridge (src-tauri/)                    |
|  DesktopEngineService: commands (start_run, resume_run, approve_action, |
|  get_checkpoints, get_branches, get_audit_logs, fork_trajectory, ...)   |
+------------------------------------+------------------------------------+
                                     | Direct in-process Rust crate
                                     v
+-------------------------------------------------------------------------+
|                  Alba Agent Core Engine (nexus-albaai)                  |
|                                                                         |
|  +-----------------------+     +-------------------------------------+  |
|  |   AgentEngine         |     |   FlowExecutor (src/workflow/)      |  |
|  |   Multi-tool calling  |     |   Start, Transform, ToolTask,       |  |
|  |   Bounded history     |     |   CommandTask, ParallelBranch,      |  |
|  |   ModelRequest +      |     |   Router, ApprovalGate, End         |  |
|  |   model_override      |     +------------------+------------------+  |
|  +-----------+-----------+                        |                     |
|              |                                    |                     |
|              v                                    v                     |
|  +-----------------------+     +-------------------------------------+  |
|  |   Governance & Gate   |     |   Tools & OS-Native Sandbox         |  |
|  |   ApprovalPolicy      |     |   read_file, write_file (diff),     |  |
|  |   author != approver  |     |   list_dir (.gitignore)             |  |
|  |   audit_logs          |     |   run_command -> Linux (bwrap /     |  |
|  +-----------+-----------+     |   unshare) | macOS (sandbox-exec)   |  |
|              |                 +------------------+------------------+  |
|              v                                    |                     |
|  +------------------------------------------------+------------------+  |
|  |   SqliteCheckpointStore (WAL, Unix mode 0600)                     |  |
|  |   checkpoints, workflow_steps, approvals, audit_logs,             |  |
|  |   trajectory_branches, cancelled_runs                             |  |
|  +-------------------------------------------------------------------+  |
+-------------------------------------------------------------------------+
```

---

## 2. Core Runtime Mechanisms

### Multi-Tool Agent Loop & Bounded History (`src/agent/engine.rs`)
- Executes every tool call returned by `ModelClient::complete` in sequential order, appending a matching `ChatMessage::tool(call_id, content)` for every `tool_call_id` even if an intermediate call errors or pauses on approval.
- Applies `compact_history(&history, &limits)` (`HistoryLimits { max_messages: 64, max_bytes: 524288 }`) to keep in-memory context bounded while preserving leading system prompts and active assistant/tool exchanges.

### Resumable DAG Flows (`src/workflow/flow.rs`)
- `FlowSpec` graphs compile into step-by-step execution across `Start`, `Transform`, `ToolTask`, `CommandTask`, `ParallelBranch`, `Router`, `ApprovalGate`, and `End`.
- Every step transition commits a `WorkflowStepCheckpoint` into SQLite (`workflow_steps`), emitting `FLOW_STATE_CHANGED` so a crashed or paused flow resumes cleanly without repeating already committed steps.

### Time-Travel Checkpoints & Trajectory Forking (`src/checkpoints/sqlite.rs`)
- `fork_trajectory` (and CLI `fork`) copies checkpoint state at any `target_sequence` into a new `ExecutionScope` (`ForkRunRequest`), writes a `trajectory_branches` lineage record, and persists `model_override` and `prompt_override`.
- On `resume_run`, `AgentState.model_override` flows through `ModelRequest.model_override` to `OpenAiCompatibleClient`, allowing forked branches to compare alternate models side-by-side.
- `rewind_and_fork_flow` rewinds DAG execution to any prior `WorkflowStepCheckpoint`, patches `state.variables`, and resumes along an isolated branch.

---

## 3. Single-Use Approval, Separation of Duties & Crash Recovery

- **Separation of Duties:** `RunCheckpoint` and `ApprovalRequest` persist `author_id` (creator/author identity). In `SqliteCheckpointStore::resolve_approval`, `ApprovalDecision::AllowOnce` fails closed if `author_id == approver_id` (self-approval blocked), while explicit operator `Deny` remains permitted to abort a run and records both identities in `audit_logs`.
- **Atomic Claim (`allowed_once` -> `executing`):** On resume, `claim_approved_execution` runs a `BEGIN IMMEDIATE` SQLite transaction performing `UPDATE approvals SET status = 'executing' WHERE id = ? AND status = 'allowed_once' AND consumed_at IS NULL`, writes an `executing` checkpoint row, and records `approval.executing`. Concurrent thread or process workers observe 0 updated rows or `executing` status and abort without invoking the tool.
- **Post-Tool Completion (`executing` -> `consumed`):** After tool completion, `complete_approval_execution` marks `status = 'consumed'` and sets `consumed_at`, preventing replay.
- **Fail-Closed Crash Recovery:** If the process terminates while status is `executing`, restart refuses to re-execute an in-flight external side effect automatically.

---

## 4. OS-Native Process Sandboxing & Credential Hardening

- **Linux (`src/sandbox/linux.rs`):** Runs commands via Bubblewrap (`bwrap`, primary) or `unshare` (`--user --map-root-user --mount --pid --fork`, with `--net` whenever `allow_network == false`), binding only the canonicalized workspace root read-write while mounting `/` read-only and blank tmpfs over `/tmp`, `/home`, and `/root`.
- **macOS (`src/sandbox/macos.rs`):** Generates deterministic Apple Seatbelt profiles (`generate_seatbelt_profile`) denying default access and outbound network by default (`(deny default)` / `(deny network*)`), restricting writes to the canonical workspace root when invoked through `sandbox-exec`.
- **Credential Hardening (`src/cli/commands.rs`):** No `--api-key` CLI flag exists (preventing secret leakage in `ps` output). Credentials come strictly from `NEXUS_API_KEY` or `--api-key-stdin`, `ModelConfig`'s `Debug` implementation renders `api_key` as `[REDACTED]`, and SQLite/JSONL log files are created with mode `0600`.
