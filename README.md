# Alba Agent

**Alba Agent** (`nexus-albaai` core engine + `alba-agent-desktop` Tauri v2 desktop application) is a zero-dependency, local-first AI coding and workflow runtime written in Rust, React 19, TypeScript, and SQLite. It ships as standalone native binaries with no Python runtime, Node.js server processes, external databases, or Docker Desktop dependencies required.

---

## Key Features

- **Native Rust Core Engine (`src/`):**
  - Multi-tool OpenAI-compatible agent loop with ordered execution, strict `tool_call_id` matching, and bounded conversation compaction (`HistoryLimits`).
  - Resumable DAG Flow orchestrator (`FlowExecutor`) supporting `Start`, `Transform`, `ToolTask`, `CommandTask`, `ParallelBranch`, `Router`, `ApprovalGate`, and `End` with atomic SQLite persistence per step.
  - Time-travel debugging and A/B trajectory forking (`fork_trajectory` & `rewind_and_fork_flow`) with per-branch `model_override` and prompt/variable patching.
- **Hardened Workspace Tools & OS-Native Sandbox (`src/tools/`, `src/sandbox/`):**
  - `read_file` (2 MiB cap, symlink/traversal proof, sensitive file filtering for `.env`, `.ssh`, keys).
  - `write_file` (atomic temp+fsync+rename with unified diff calculation and `SANDBOX_FILE_DIFF` event).
  - `list_dir` (depth-bounded, `.gitignore`-aware walker).
  - `run_command` via OS-native containment: Bubblewrap (`bwrap`) / kernel `unshare` namespaces on Linux and Seatbelt (`sandbox-exec`) on macOS, workspace-confined and network-disabled by default.
- **Human-in-the-Loop Governance (`src/checkpoints/`, `src/governance/`):**
  - Single-use atomic approval gates (`allowed_once -> executing -> consumed`) backed by `BEGIN IMMEDIATE` SQLite WAL transactions (`0600`).
  - Separation of Duties (`author_id != approver_id` for `AllowOnce` approvals) with immutable `audit_logs`.
- **Tauri v2 Desktop IDE (`src-tauri/`, `ui/`):**
  - Lightweight `12.68 MiB` release binary (`alba-agent`) embedding the React 19 + Tailwind CSS v4 frontend.
  - Three-pane IDE layout: workspace file tree & time-travel checkpoint history, Monaco Editor with inline unified diff viewer + Xterm.js sandbox output pane, interactive `[ Allow Once ]` / `[ Deny ]` Approval Card + live event timeline, and `Cmd/Ctrl+K` prompt bar.

---

## Measured Release Artifacts (Linux aarch64)

- **Headless CLI Engine (`nexus-albaai`):** `8,229,112` bytes (`7.85 MiB`), `60.35 ms` median cold startup (`--version`), `18.29 MiB` startup RSS, `113.67 ms` median single-tool loop (`read_file` fixture).
- **Full Desktop App (`alba-agent`):** `13,300,928` bytes (`12.68 MiB`), dynamically linked against OS WebKitGTK/GTK3 without bundled Chromium or Python. See [`docs/benchmark.md`](docs/benchmark.md) for methodology and raw measurements.

---

## Documentation Index

- [`docs/architecture.md`](docs/architecture.md) — Engine architecture, DAG flows, sandbox containment, and recovery semantics
- [`docs/events.md`](docs/events.md) — Runtime JSON event contract (`schema_version = "1.0"`)
- [`docs/compatibility.md`](docs/compatibility.md) — Contract compatibility with the Python control plane
- [`docs/benchmark.md`](docs/benchmark.md) — Reproducible latency, RSS, and artifact size measurements
- [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) — Consolidated permissive attributions for Rust crates and npm packages

---

## Building from Source

```bash
npm --prefix ui install
npm --prefix ui run build
cargo test --workspace
cargo build --release --workspace
```

---

## CLI Usage

Credentials are read exclusively from `NEXUS_API_KEY` or `--api-key-stdin` (no `--api-key` CLI flag exists so keys never appear in process trees). Trusted approver identity is read from `NEXUS_ACTOR_ID` or `--config` JSON.

### 1. Start a Run

```bash
NEXUS_API_KEY="sk-..." target/release/nexus-albaai run \
  --prompt "Inspect data.txt and summarize metrics" \
  --workspace-root ./workspace \
  --model gpt-4o-mini \
  --db-path albaai.db
```

### 2. Inspect Status, Approve or Deny, and Resume

```bash
target/release/nexus-albaai status --run-id <RUN_UUID> --db-path albaai.db

NEXUS_ACTOR_ID="reviewer@example.com" target/release/nexus-albaai approve \
  --run-id <RUN_UUID> \
  --approval-id <APPROVAL_UUID> \
  --decision allow_once \
  --db-path albaai.db

NEXUS_API_KEY="sk-..." target/release/nexus-albaai resume \
  --run-id <RUN_UUID> \
  --workspace-root ./workspace \
  --db-path albaai.db
```

### 3. Fork a Checkpoint Trajectory & Cancel

```bash
target/release/nexus-albaai fork \
  --run-id <RUN_UUID> \
  --sequence <SEQ> \
  --branch-label alt-model \
  --model-override gpt-4o \
  --prompt-override "Use concise bullet points." \
  --db-path albaai.db

target/release/nexus-albaai cancel --run-id <RUN_UUID> --db-path albaai.db
```

---

## Verification Suite

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check licenses
./scripts/check_no_comments.sh
npm --prefix ui run build
```
