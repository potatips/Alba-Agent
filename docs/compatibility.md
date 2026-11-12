# Compatibility with Python Alba Control Plane Contracts

`nexus-albaai` and `alba-agent-desktop` implement the core local runtime semantics of the Alba Agent platform in native Rust + SQLite + OS-native sandboxing, preserving scope, checkpoint, single-use approval, workflow, and event contracts while eliminating Python, Node.js backend workers, and Docker Desktop requirements.

## 1. Capability Coverage

1. **Multi-Turn Tool Loop (`AgentEngine`):**
   - Bounded conversation history (`HistoryLimits { max_messages: 64, max_bytes: 524288 }`) with `compact_history` preserving system prompts and assistant/tool call pairs.
   - Sequential execution of all model `tool_calls` with `tool_call_id` matching and checkpoint persistence across pause/resume.
   - OpenAI-compatible chat completions HTTP client (`OpenAiCompatibleClient`) supporting `model_override` for forked A/B runs.
2. **Resumable DAG Workflows (`FlowExecutor`):**
   - `FlowSpec` with typed `FlowNodeKind` nodes (`Start`, `Transform`, `ToolTask`, `CommandTask`, `ParallelBranch`, `Router`, `ApprovalGate`, `End`) and per-step atomic SQLite transactions (`workflow_steps`).
3. **Time-Travel Checkpoints & A/B Trajectory Forking:**
   - Rewind to any checkpoint sequence via `fork_trajectory` or `rewind_and_fork_flow`, override model or prompt/variables in an isolated new `run_id`, and resume cleanly.
4. **Hardened Workspace Tools & OS-Native Sandbox:**
   - `read_file` (2 MiB cap, canonicalization, sensitive path denial), `write_file` (atomic write + unified diff), `list_dir` (`.gitignore`-aware recursive traversal), `run_command` (Linux `bwrap` / `unshare` namespaces; macOS Seatbelt `sandbox-exec`).
5. **Atomic Single-Use Approvals & Separation of Duties:**
   - Single-use claim (`allowed_once` -> `executing` -> `consumed`) via `BEGIN IMMEDIATE`, immutable `audit_logs`, and enforcement of `author_id != approver_id` for `AllowOnce` grants.
6. **Desktop IDE Shell (`Alba Agent`):**
   - Tauri v2 (`src-tauri/`) + React 19 (`ui/`) with Monaco Editor inline diff viewer, Xterm.js sandbox output pane, File Tree, Checkpoint & Fork explorer, Event Timeline, and Approval Card.

## 2. Preserved Contracts

### Scope Contract (`ExecutionScope`)
- `tenant_id: Uuid` (non-nil)
- `workspace_id: Uuid` (non-nil)
- `run_id: Uuid` (non-nil)

### Checkpoint State Contract
- Strictly monotonic integer `sequence` per run.
- Status values: `queued`, `running`, `paused`, `executing`, `completed`, `failed`, `cancelled`.
- Single-use `allow_once` approval consumption semantics (`consumed_at IS NULL` guard).

### Credential & Privacy Contract
- API keys accepted exclusively via `NEXUS_API_KEY` or `--api-key-stdin` (no `--api-key` CLI flag).
- Zero telemetry, zero persisted API keys or `Authorization` headers in SQLite or JSONL event logs, and mode `0600` creation for SQLite and file event logs on Unix.

## 3. Capabilities Remaining Out of Scope Locally

The following platform-wide control plane features live in the hosted/distributed control plane (`NexusAgent`) rather than this standalone local desktop binary:
- Hosted FastAPI control plane, PostgreSQL Row-Level Security across remote clusters, Stripe billing webhooks, and multi-user OAuth/JWT SSO.
- LlamaIndex + Qdrant distributed RAG clusters and remote MCP/OpenAPI server gateways.
- Remote cloud sandboxes (E2B, Daytona, CreateOS, Boat) and cross-agent server-side learning transfer / model arbitrage schedulers.
