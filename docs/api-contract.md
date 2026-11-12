# Alba Platform API Contract & Data Schema (Phase G1)

Date: 2026-11-12
Status: Baseline Contract Established & Type-Generation Tests Verified
Working Branch: `feat/g1-contract`

---

## 1. Full Inventory of Detected Endpoints in `apps/web`

Scanning `NexusAgent/apps/web/src` identified 63 unique API endpoint patterns across components and libraries:

| Endpoint Pattern | Category | Included in v1 Minimal Set | Notes |
| :--- | :--- | :---: | :--- |
| `/api/v1/auth/local-mode` | Auth | **Yes** | Local-first detection for desktop/single-user |
| `/api/v1/auth/local-session` | Auth | **Yes** | Transparent workspace token generation |
| `/api/v1/auth/me` | Auth | **Yes** | Operator profile and workspace claims |
| `/api/v1/auth/keys` | Auth | No | Multi-tenant user API keys (deferred) |
| `/api/v1/auth/login` | Auth | No | Cloud web login (deferred in local-first) |
| `/api/v1/auth/token` | Auth | No | Cloud token refresh (deferred in local-first) |
| `/api/v1/auth/signup*` | Auth | No | Cloud multi-user signup (deferred) |
| `/api/v1/workflows` | Workflow | **Yes** | List workflows |
| `/api/v1/workflows` (POST) | Workflow | **Yes** | Create/import WorkflowSpec |
| `/api/v1/workflows/:id` | Workflow | **Yes** | Get/update/delete WorkflowSpec |
| `/api/v1/workflows/:id/runs` | Workflow | **Yes** | Trigger execution of workflow |
| `/api/v1/workflows/validate` | Workflow | **Yes** | Validate graph acyclicity and schemas |
| `/api/v1/workflows/import` | Workflow | **Yes** | Import JSON/YAML workflow definition |
| `/api/v1/runs` | Runs | **Yes** | List runs across workspace |
| `/api/v1/runs/:id` | Runs | **Yes** | Get run state, deliverables, errors |
| `/api/v1/runs/:id/stream` | Runs (SSE) | **Yes** | Real-time SSE streaming for live execution |
| `/api/v1/runs/:id/cancel` | Runs | **Yes** | Cooperative cancellation & abort |
| `/api/v1/runs/:id/fork` | Runs | **Yes** | Time-travel fork from checkpoint |
| `/api/v1/runs/:id/timeline` | Runs | **Yes** | Chronological checkpoints timeline |
| `/api/v1/runs/:id/artifacts` | Artifacts | **Yes** | List artifacts produced with verify status |
| `/api/v1/runs/:id/artifacts/:name` | Artifacts | **Yes** | Read artifact bytes / diff preview |
| `/api/v1/approvals` | Approvals | **Yes** | List pending approval gates |
| `/api/v1/approvals/:id/decide` | Approvals | **Yes** | Allow-once or Deny (author != approver) |
| `/api/v1/bots` | Bots | **Yes** | List & configure persistent agent bots |
| `/api/v1/bots/:id` | Bots | **Yes** | Manage individual AgentSpec configurations |
| `/api/v1/conversations` | Chat | **Yes** | Threaded interactive agent sessions |
| `/api/v1/conversations/:id` | Chat | **Yes** | Conversation message stream |
| `/api/v1/conversations/:id/messages` | Chat | **Yes** | Post new user prompt to agent |
| `/api/v1/settings/chat-models` | Settings | **Yes** | Discovered model provider catalog |
| `/api/v1/settings/execution` | Settings | **Yes** | Seatbelt policy (Deny/Ask/Allow domains) |
| `/api/v1/settings/credentials` | Settings | **Yes** | Encrypted provider API keys |
| `/api/v1/tools/builtin` | Tools | **Yes** | Built-in sandbox tools catalog |
| `/api/v1/tools/builtin/:id/enabled` | Tools | **Yes** | Enable/disable specific tools |
| `/api/v1/learning-transfer/capsules` | Knowledge | **Yes** | Distilled knowledge capsules (scope firewall) |
| `/api/v1/learning-transfer/telemetry` | Knowledge | **Yes** | Negative transfer metrics for UI radar |
| `/api/v1/skills` | Skills | **Yes** | SKILL.md progressive loader catalog |
| `/api/v1/skills/:slug` | Skills | **Yes** | Skill details and activation triggers |
| `/api/v1/architect/*` | Architect | No | Out of scope for v1 |
| `/api/v1/evolution/*` | Evolution | No | Out of scope for v1 |
| `/api/v1/knowledge-bases/*` | Heavy RAG | No | Replaced by embedded sqlite-vec / usearch |
| `/api/v1/analytics/distillation/*` | Analytics | No | Out of scope for v1 |
| `/api/v1/self-healing/*` | Self-healing| No | Out of scope for v1 |
| `/api/v1/sandboxes/*` | Cloud sandboxes | No | Local seatbelt sandbox used instead of Docker |

---

## 2. Minimal v1 API Specification

### A. Authentication & Local Bootstrap
* `GET /api/v1/auth/local-mode`
  * Response: `{ "enabled": true }`
* `POST /api/v1/auth/local-session`
  * Response:
    ```json
    {
      "access_token": "local_dev_token",
      "tenant_id": "00000000-0000-0000-0000-000000000001",
      "workspace_id": "00000000-0000-0000-0000-000000000001",
      "expires_in": 86400
    }
    ```
* `GET /api/v1/auth/me`
  * Response: `{ "id": "local_operator", "name": "Local Operator", "role": "owner" }`

### B. Workflows & Runs
* `GET /api/v1/workflows`
  * Response: `[ { "id": "uuid", "name": "...", "version": 1, "node_count": 5, "updated_at": "ISO" } ]`
* `POST /api/v1/workflows`
  * Request Body: `WorkflowSpec` JSON
  * Response: `{ "id": "uuid", "name": "...", "version": 1 }`
* `POST /api/v1/workflows/:id/runs`
  * Request Body: `{ "input_data": { ... } }`
  * Response: `{ "run_id": "uuid", "status": "running" }`
* `GET /api/v1/runs/:id`
  * Response:
    ```json
    {
      "id": "uuid",
      "workflow_id": "uuid",
      "status": "running|paused|completed|failed|cancelled",
      "current_node_id": "crew_1",
      "created_at": "ISO",
      "updated_at": "ISO",
      "error": null,
      "deliverables": [ { "path": "src/app.rs", "verified": true } ]
    }
    ```
* `GET /api/v1/runs/:id/stream` (SSE Server-Sent Events)
  * Format: `text/event-stream`
  * Event types:
    * `run_status`: `{ "status": "running" | "paused" | "completed" | "failed" }`
    * `step_started`: `{ "step_index": 1, "node_id": "node_1", "role": "worker" }`
    * `tool_call`: `{ "tool": "file_write", "arguments": { ... } }`
    * `approval_needed`: `{ "approval_id": "uuid", "tool": "file_write", "diff": "..." }`
    * `checkpoint_saved`: `{ "checkpoint_id": "uuid", "step": 1, "hash": "..." }`
    * `run_completed`: `{ "outcome": { ... }, "artifacts": [ ... ] }`

### C. Approvals (Separated Duties)
* `GET /api/v1/approvals`
  * Response: `[ { "id": "uuid", "run_id": "uuid", "tool_name": "file_write", "author_actor": "agent:coder", "diff_preview": "...", "created_at": "ISO" } ]`
* `POST /api/v1/approvals/:id/decide`
  * Request: `{ "decision": "allow_once" | "deny", "actor": "user:operator" }`
  * Invariant: `actor != author_actor` (an agent cannot approve its own request).
  * Response: `{ "status": "approved" | "denied", "decided_at": "ISO" }`

### D. Time-Travel & Trajectory Fork
* `GET /api/v1/runs/:id/timeline`
  * Response: `[ { "step_index": 0, "node_id": "start", "hash": "...", "created_at": "ISO" } ]`
* `POST /api/v1/runs/:id/fork`
  * Request: `{ "target_step_index": 2, "overrides": { ... } }`
  * Response: `{ "forked_run_id": "uuid", "parent_run_id": "uuid", "parent_step_index": 2 }`

---

## 3. Core-Schema Rust Serde Mapping

All data models are implemented under `nexus_albaai::schema`:

| Schema Component | Rust Type | Validation & Invariants |
| :--- | :--- | :--- |
| `AgentSpec` | `nexus_albaai::schema::AgentSpec` | `id` format regex, `tier` enum (director, lead, specialist), default `temperature = 0.2`, `max_iterations = 10`. |
| `TaskSpec` | `nexus_albaai::schema::TaskSpec` | `agent_id` reference check, `expected_artifact_paths` bounds check. |
| `StartNodeSpec` | `nexus_albaai::schema::StartNodeSpec` | `kind: "start"`, default trigger `manual`. |
| `CrewNodeSpec` | `nexus_albaai::schema::CrewNodeSpec` | `kind: "crew"`, sequential/hierarchical process, consensus config. |
| `NexusGenSandboxNodeSpec` | `nexus_albaai::schema::NexusGenSandboxNodeSpec` | `kind: "sandbox"`, `verification_command`, clean code review flag. |
| `ApprovalGateNodeSpec` | `nexus_albaai::schema::ApprovalGateNodeSpec` | `kind: "approval"`, timeout bounds, boundary rules. |
| `RouterNodeSpec` | `nexus_albaai::schema::RouterNodeSpec` | `kind: "router"`, route conditions, default fallback, loop iteration budget. |
| `RAGNodeSpec` | `nexus_albaai::schema::RAGNodeSpec` | `kind: "rag"`, embedded vector knowledge base connection. |
| `WorkflowNode` | `nexus_albaai::schema::WorkflowNode` | Tagged enum discriminated by `"kind"`. |
| `WorkflowSpec` | `nexus_albaai::schema::WorkflowSpec` | Scoped by tenant/workspace, graph validation, layout mapping. |
| `KnowledgeCapsuleSpec` | `nexus_albaai::schema::KnowledgeCapsuleSpec` | `PropagationScope`, `SensitivityLevel`, trigger preconditions, distilled heuristic. |
| `ApprovalRequestSpec` | `nexus_albaai::schema::ApprovalRequestSpec` | Author actor vs. approver actor isolation. |

---

## 4. SQLite Storage DDL (Target v1 Schema)

The unified SQLite schema replaces Postgres + Redis + Qdrant. SQLite operates in WAL mode (`PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;`).

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    spec_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_workflows_scope ON workflows(tenant_id, workspace_id);

CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    workflow_id TEXT NOT NULL,
    status TEXT NOT NULL,
    input_data TEXT NOT NULL DEFAULT '{}',
    output_data TEXT,
    current_node_id TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(tenant_id, workspace_id, status);

CREATE TABLE IF NOT EXISTS checkpoints (
    run_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    state_json TEXT NOT NULL,
    parent_checkpoint_hash TEXT,
    checkpoint_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (run_id, step_index)
);

CREATE TABLE IF NOT EXISTS workflow_checkpoints (
    run_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    flow_state_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (run_id, node_id, step_index)
);

CREATE TABLE IF NOT EXISTS trajectory_branches (
    branch_id TEXT PRIMARY KEY,
    parent_run_id TEXT NOT NULL,
    parent_step_index INTEGER NOT NULL,
    forked_run_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS approvals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    parameters TEXT NOT NULL,
    diff_preview TEXT,
    author_actor TEXT NOT NULL,
    approver_actor TEXT,
    status TEXT NOT NULL,
    decision TEXT,
    created_at TEXT NOT NULL,
    decided_at TEXT,
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_approvals_pending ON approvals(run_id, status);

CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL,
    details_json TEXT NOT NULL,
    prev_hash TEXT NOT NULL,
    entry_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cancellations (
    run_id TEXT PRIMARY KEY,
    reason TEXT NOT NULL,
    cancelled_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_capsules (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    source_agent_id TEXT NOT NULL,
    source_role TEXT NOT NULL,
    source_domain_tags TEXT NOT NULL DEFAULT '[]',
    trigger_preconditions TEXT NOT NULL,
    distilled_heuristic TEXT NOT NULL,
    prohibited_contexts TEXT NOT NULL DEFAULT '[]',
    propagation_scope TEXT NOT NULL,
    sensitivity_level TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.5,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_capsules_scope ON knowledge_capsules(tenant_id, workspace_id, propagation_scope);

CREATE TABLE IF NOT EXISTS artifacts (
    run_id TEXT NOT NULL,
    path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    verified INTEGER NOT NULL DEFAULT 0,
    claims_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    PRIMARY KEY (run_id, path),
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS bots (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    spec_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    title TEXT NOT NULL,
    bot_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    tool_calls_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

## 5. Verification Proof
* Contract Schema Types: Compiled and passing in `nexus-albaai` crate (`tests/schema_contract_tests.rs`).
* Zero-Comment Enforcement: Passes `./scripts/check_no_comments.sh` (0 comments found).
* Backward Compatibility: All core JSON Schema types in `NexusAgent/packages/core-schema` accurately map into Rust serde structures without field loss.
