# albaAI Stage 3 Migration Roadmap

## 1. Scope and Ordering

Stages 1 and 2 established the standalone native single-tool execution engine (`nexus-albaai`) with scoped SQLite checkpoints, trusted CLI identity (`NEXUS_ACTOR_ID`, `NEXUS_TENANT_ID`, `NEXUS_WORKSPACE_ID`), atomic `allowed_once` -> `executing` -> `consumed` single-use tool approval claims, `read_file` workspace hardening, JSON Lines runtime events, `deny.toml` license checking, and zero-comments verification.

Stage 3 migrates the remaining core execution subsystems of **Alba Agent** out of the legacy Python stack (`/Users/nguyenvokhang/Downloads/NexusAgent/`, treated read-only during migration) into standalone Rust in this dependency order:

1. **Flow state machine (`@start` / `@listen` / `@router`)**
2. **Task and Crew**
3. **Sandbox adapter**
4. **RAG**
5. **Gateway**

Each stage extends the native binary while preserving Stage 1/2 guarantees:
- Strict domain isolation (`tenant_id`, `workspace_id`, `run_id`) and SQLite transactional state transitions.
- Zero external telemetry (`OTEL_SDK_DISABLED=true`, `CREWAI_DISABLE_TELEMETRY=true` equivalents — no telemetry code included).
- Explicit rejection of unsupported workflow specs, node kinds, or malformed schemas prior to execution.
- Strict compliance with the `AGENTS.md` permissive license allowlist (`deny.toml`) and the mandatory zero-comments check (`./scripts/check_no_comments.sh`).

---

## 2. Step 1: Flow State Machine (`@start` / `@listen` / `@router`)

### Legacy Behavior to Preserve

Source paths in `NexusAgent`:
- `packages/core-schema/python/nexus_core_schema/models.py` (`WorkflowSpec`, `ScopedContract`, `StartNodeSpec`, `CrewNodeSpec`, `NexusGenSandboxNodeSpec`, `ApprovalGateNodeSpec`, `RouterNodeSpec`, `RouteSpec`, `RAGNodeSpec`, `WorkflowNode`, `WorkflowEdgeSpec`, `WorkflowModesSpec`, `forward_edges`, `_unique_artifact_paths`)
- `packages/agent-runtime/flows/state.py` (`WorkflowState`)
- `packages/agent-runtime/flows/compiler.py` (`WorkflowCompiler`, `CompiledWorkflow`, `PauseRequested`, `_topological_order`, `_loop_region`, `_read_path`, `_choose_route`, `CONTEXT_CHARACTER_LIMIT`)
- `packages/agent-runtime/checkpoints/store.py` (`CheckpointStore`)
- `packages/agent-runtime/callbacks/event_bus.py` (`RuntimeEventEmitter`, `RuntimeEventType`)

Contracts and runtime behaviors to preserve:
1. **Strict Graph and Scope Validation (`WorkflowSpec.validate_graph`, `WorkflowCompiler.validate`, `compile`):**
   - Reject duplicate `node.id` values, empty node sets, or `layout` keys that do not match the exact set of `node_ids`.
   - Require `spec.tenant_id` and `spec.workspace_id` to match the active runtime `ExecutionScope`.
   - Require exactly one `StartNodeSpec` (`kind == "start"`) with zero incoming edges.
   - Require all forward edges (`loop_back == false`) to form an acyclic DAG in which every node is reachable from the `start` node.
   - Enforce router edge rules: only edges leaving `RouterNodeSpec` may carry `route`; `route` must match a declared `RouteSpec.name` or `default_route`; no `(source, route)` pair may be both forward and `loop_back`.
   - Reject unimplemented node kinds (`crew`, `sandbox`, `rag`) explicitly at validation/compile time until Steps 2–4 land rather than ignoring them or substituting no-op stubs.
2. **State Shape (`WorkflowState`):**
   - Typed checkpointable state preserving `extra="forbid"` behavior (`#[serde(deny_unknown_fields)]` on external boundaries): `id`, `run_id`, `workflow_id`, `tenant_id`, `workspace_id`, `status` (`queued`, `running`, `paused`, `completed`, `failed`, `cancelled`), monotonic `sequence`, `input_data`, `node_outputs`, `upstream_artifacts`, `completed_nodes`, `unverified_nodes`, `active_node_id`, `pending_approval_id`, `pending_boundary`.
3. **Graph Execution Semantics (`@start`, `@listen`, `@router`, `and_`, `or_`):**
   - `@start`: entry node populates output from `state.input_data`.
   - `@listen`: fan-in respects `join`: `"all"` waits for all incoming forward parents (`and_`), except when any incoming source is a `RouterNodeSpec` (`compiler.py` falls back to `or_` join semantics when fed by a router); `"any"` proceeds on first matching parent (`or_`).
   - `@router`: evaluates `routes` in order against dot-separated `field_path` on `{"input": state.input_data, **state.node_outputs}` supporting all seven operators (`eq`, `ne`, `exists`, `gt`, `gte`, `lt`, `lte`), falling back to `default_route`. Numeric comparisons (`gt`/`gte`/`lt`/`lte`) evaluate `false` if operand types are non-numeric. Supports optional `artifact_gate` evaluation stored at `{node.id}__gate`.
4. **Bounded Rework Loop-Back (`run_router`, `_loop_region`):**
   - A `loop_back` edge requires a `RouterNodeSpec` source with `max_loop_iterations >= 1` (bounded `1..=5`), must target an ancestor inside `_loop_region(spec, target, router_id)`, and the computed rework region must not contain any `ApprovalGateNodeSpec`.
   - On taking a loop-back route below budget: increment `{node.id}__loops`, populate `state.node_outputs["rework_feedback"]`, emit `FLOW_STATE_CHANGED` (`status: "rework"`), strip region nodes from `completed_nodes` and `node_outputs`, save a checkpoint, and re-execute the region in forward topological order. Fail the run cleanly once `iteration >= max_loop_iterations`.
5. **Approval Pause, Idempotent Resume, and Cancellation:**
   - `ApprovalGateNodeSpec` records an `ApprovalRequest`, sets `pending_approval_id`, emits `APPROVAL_REQUIRED`, saves a `paused` checkpoint, and halts execution (`PauseRequested` behavior).
   - On resume, check cancellation (`is_cancelled`) first, require `allowed_once` or `always_allowed` via Stage 2 atomic SQLite `approved -> executing` claim, skip nodes already present in `completed_nodes` (returning cached `node_outputs` without re-execution), clear `pending_approval_id`, emit `APPROVAL_RESOLVED`, and continue downstream nodes.
   - Enforce `CONTEXT_CHARACTER_LIMIT` (`24_000` chars) truncation rules when serializing node context across steps, plus `_nexus_node_overrides` validation (only `prompt`, `model`, `temperature` allowed on replay).

### Completion Criteria

- Typed Rust `workflow` module with `WorkflowSpec`, `WorkflowNode` enum (`Start`, `ApprovalGate`, `Router`, plus feature/stage-gated variants for `Crew`, `Sandbox`, `Rag`), `WorkflowEdgeSpec`, and strict serde validation matching `models.py`.
- Static graph compiler + async DAG executor in Rust (`albaAI`) persisting SQLite checkpoints after each node and integrating with `nexus-albaai` CLI (`run --workflow <spec.json>` alongside existing `--prompt` mode).
- Event parity emitting valid JSON Lines events (`FLOW_STATE_CHANGED`, `APPROVAL_REQUIRED`, `APPROVAL_RESOLVED`, `RUN_COMPLETED`, `RUN_FAILED`) validated against `tests/fixtures/` schemas.
- Automated integration tests proving:
  - Validation rejection of cyclic forward graphs, 0 or >1 start nodes, incoming edge to `start`, orphan/unreachable nodes, mismatched scope, invalid router routes, and `loop_back` regions containing an `ApprovalGateNodeSpec`.
  - Diamond fan-in (`join = "all"` vs `join = "any"`).
  - All seven `@router` operators plus `default_route` and bounded loop-back execution + budget exhaustion failure.
  - Multi-step workflow pause at `ApprovalGateNodeSpec`, process exit, `approve`/`deny` via trusted `NEXUS_ACTOR_ID`, and `resume` completing downstream nodes without re-executing earlier completed nodes.

### Risks

- **SQLite checkpoint size growth:** Snapshotting full `node_outputs` on every node step can inflate checkpoint rows if outputs carry large payloads. Enforce strict size bounds on stored outputs and reference artifacts cleanly on disk.
- **Parallel branch state races:** If independent fan-out branches run concurrently under Tokio tasks, uncoordinated writes to `completed_nodes`, `sequence`, and `pending_approval_id` will race. Serialize state mutations under an execution mutex per run and flush SQLite checkpoints sequentially.
- **Dynamic Python behavior vs static Rust enums:** Legacy `FlowWorkflowState` mixes fixed fields with arbitrary `node_outputs` keys (`{id}__loops`, `{id}__gate`, `rework_feedback`). Keep dynamic keys confined to explicit typed maps/fields inside `WorkflowState` so schema validation stays strict.
- **Unsupported node boundary:** Until Steps 2–4 are complete, tests and callers must receive explicit pre-flight rejection when submitting `WorkflowSpec` documents containing `crew`, `sandbox`, or `rag` nodes.

---

## 3. Step 2: Task and Crew

### Legacy Behavior to Preserve

Source paths in `NexusAgent`:
- `packages/core-schema/python/nexus_core_schema/models.py` (`AgentSpec`, `TaskSpec`, `CrewNodeSpec`, `AgentTier`, `ConsensusConfigSpec`)
- `packages/agent-runtime/crews/builder.py` (`CrewBuilder`, `BuiltCrew`, `_remove_jsonc_comments`, `_task_description`, `CONTEXT_PLACEHOLDERS`, `schema_for_task`, `from_crew`)
- `packages/agent-runtime/flows/compiler.py` (`CrewNodeSpec` execution block, artifact contract instruction blocks, agent failure attribution)
- `packages/agent-runtime/crews/converter.py` (`LlmCallConverter`)

Contracts and runtime behaviors to preserve:
1. **Crew Reference & Manager Invariants (`CrewNodeSpec.validate_references`):**
   - Enforce unique `agent.id` and `task.id` within each `CrewNodeSpec`.
   - Every `TaskSpec.agent_id` must exist in `agents`; every `TaskSpec.context_task_ids` entry must reference an earlier task in task execution order (preventing forward/missing context dependencies).
   - If `manager_agent_id` is provided: `process` must be `"hierarchical"`, manager must belong to `agents`, manager must have `tool_ids == ()` (delegation-only), no task may assign `agent_id == manager_agent_id`, and `agents.len() >= 2`.
   - Enforce `expected_artifact_paths` uniqueness and reject `..` traversal segments; reject unknown `tool_ids` or `skill_ids` before execution begins.
2. **Bidirectional Configuration Loading (JSON / JSONC / YAML):**
   - Support JSON and JSONC (preserving quoted strings while stripping `//`, `/* ... */`, and trailing commas cleanly as in `_remove_jsonc_comments`), enforcing a 1 MiB maximum config file size, plus schema-validated serialization back to JSON/config spec.
3. **Safe Task Description & Prompt Assembly (`_task_description`):**
   - Allow only `{input}` and `{nodes}` template substitutions; neutralize unknown `{identifier}` placeholders (e.g., rewrite to `(identifier)`) to prevent template injection or formatting failures; append `{input}`, `{nodes}`, and path-rule / artifact preambles when absent, keeping prompt assembly within bounded context limits.
4. **Sequential and Hierarchical Process Execution:**
   - **Sequential:** Execute tasks in topological/declared order; run each task via Stage 1/2 bounded agent tool loop (`max_iterations` capped at `1..=100`, respecting `Tool` approval gates and single-use resume claims), passing structured outputs from `context_task_ids` into subsequent tasks.
   - **Hierarchical:** Manager agent (without direct tools) coordinates specialist agents via bounded delegation turns and validates deliverables against task `expected_output` / `expected_artifact_paths`.
5. **Structured JSON Output Validation (`output_schema` / `output_schema_json` / `LlmCallConverter`):**
   - Enforce structured output conformance against declared task schemas (bounded dynamic schemas up to 64 scalar/scalar-array fields or registered schemas), retrying/converting malformed LLM outputs cleanly or failing the task explicitly rather than returning free-form unvalidated text.
6. **Runtime Telemetry & Progress Events:**
   - Emit `CREW_AGENT_THOUGHT` / `TOOL_INVOKED` progress notes capped safely (`<= 1200` chars) with secrets/reasoning stripped.

### Completion Criteria

- Implement `CrewNodeSpec`, `AgentSpec`, `TaskSpec`, `CrewRunner` (sequential + hierarchical processes), JSONC parser, placeholder sanitizer, and structured schema validator in `albaAI`.
- Enable `WorkflowNode::Crew` in the Step 1 Flow compiler (resolving `{input}`, `{nodes}`, and `rework_feedback` into crew tasks).
- Pause cleanly inside a crew task when an agent requests a guarded tool, preserving both Flow checkpoint state and active task/crew progress across process restart and `resume`.
- Validate all new dependencies (JSON Schema validator / optional YAML parser) with `cargo deny check licenses`.
- Integration tests covering:
  - Sequential multi-agent crew handing off structured `context_task_ids` outputs and executing approved tools.
  - Hierarchical crew delegation between a tool-less manager and >= 2 specialist agents.
  - Mid-crew `ApprovalGate` pause, restart, trusted-actor `approve`, and `resume` completing remaining tasks without rerunning finished tasks.
  - Schema rejection when model output fails `output_schema_json`, JSONC comment-stripping edge cases (URLs `https://...` inside strings), and validation rejections for tool-holding managers or out-of-order `context_task_ids`.

### Risks

- **Crate license compliance for JSON Schema / YAML:** Candidate crates such as `jsonschema` (often pulls `url` -> `idna` -> `Unicode-3.0`) or YAML parsers may pull disallowed `Unicode-3.0` or unmaintained crates. Inspect dependency graphs with `cargo tree` and `cargo deny check licenses` before adding them, or implement a focused JSON Schema subset validator using existing `serde_json` if full Draft-2020 brings disallowed transitive licenses.
- **Hierarchical model reliability without Python CrewAI runtime:** Small or local models frequently fail delegation formatting in hierarchical mode; enforce deterministic tool/delegation schemas, strict delegation depth + iteration caps, and explicit conversion retry (`LlmCallConverter` equivalent) before failing the node.
- **Partial-crew checkpoint granularity:** Without task-level checkpoint recording inside `CrewNodeSpec`, resuming a paused 4-task crew after task 3 would repeat tasks 1–2. Persist completed `task_id -> structured_output` inside the run checkpoint state alongside `completed_nodes`.

---

## 4. Step 3: Sandbox Adapter

### Legacy Behavior to Preserve

Source paths in `NexusAgent`:
- `packages/computer-sandbox/nexus_computer_sandbox/supervisor.py` (`ComputerAccess`, `ComputerHandle`, `Visibility`, `ComputerProvider`, `DockerSandboxProvider`, `ComputerSandboxSupervisor`, `acquire_run`, `release_run`, `retire_run`, `reap_orphans`, `execute_shell`, `_open_file`)
- `packages/computer-sandbox/nexus_computer_sandbox/execution_policy.py` (`Decision`, `ExecutionPolicy`, `Verdict`, `DEFAULT_RULES`)
- `packages/computer-sandbox/nexus_computer_sandbox/file_tools.py` (`ComputerFileTools`, `TEXT_LIMIT`, `view_file_numbered`, `surgical_file_edit`, `fast_workspace_search`)
- `packages/computer-sandbox/nexus_computer_sandbox/local_provider.py` (`seatbelt_profile`, `HOME_SECRETS`)
- `packages/agent-runtime/nexusgen_bridge/sandbox_team.py` (`NexusGenSandboxRunner`, `compact_log`, `_FAILURE_LINE_RE`)

Contracts and runtime behaviors to preserve:
1. **Computer Scopes, Visibility, and Provisioning (`ComputerAccess`, `ComputerSandboxSupervisor`):**
   - Separate `"shared"` team computers vs `"private"` computers (`private` restricted strictly to owning `bot_id` / `run_id`, all lookups scoped by `tenant_id` and `workspace_id`).
   - Serialize provisioning per `(tenant_id, workspace_id, owner)` so concurrent node starts attach cleanly to one running container instead of provisioning duplicates; track run leases (`acquire_run`/`release_run`) so workflow nodes retain installed packages until `retire_run`; sweep orphaned `nexus.run_id`-labeled containers (`reap_orphans`).
2. **Container Security Hardening (`DockerSandboxProvider.create`):**
   - Enforce `--pull=never`, `--network none` (blocking loopback, RFC1918, and cloud metadata access by default), `--read-only`, `--cap-drop ALL`, `--security-opt no-new-privileges:true`, `--ipc none`, `--tmpfs /tmp:rw,nosuid,nodev,size=64m`, `--pids-limit 256`, `--memory 2g`, `--memory-swap 2g`, `--cpus 2`, `--ulimit fsize=1073741824:1073741824`, non-root `--user uid:gid`, and identifying labels (`nexus.tenant_id`, `nexus.workspace_id`, `nexus.bot_id`, `nexus.run_id`).
   - Inject zero-telemetry environment defaults (`OTEL_SDK_DISABLED=true`, `CREWAI_DISABLE_TELEMETRY=true`, `NEXT_TELEMETRY_DISABLED=1`).
3. **Command Execution Policy & Boundary Guards (`ExecutionPolicy`, `execute_shell`):**
   - Classify commands into `Allow`, `Ask` (`needs_network` flag / boundary `"external_action"` / `"destructive_command"`), or `Deny` with precedence `DENY > ASK > ALLOW`.
   - `DENY` blocks secret file/directory reads (`.env`, `.ssh`, `.aws`, `.gnupg`, `id_rsa`, etc.), `sudo`, and destructive patterns (`mkfs`, root `rm`, fork bombs) immediately without asking.
   - `ASK` or destructive tokens invoke Stage 2's atomic human approval gate (`allowed_once` -> `executing` -> `consumed`) before running the command.
4. **Symlink-Safe Workspace File Operations (`_open_file`, `ComputerFileTools`):**
   - Traverse workspace components directory-by-directory with `O_RDONLY | O_DIRECTORY | O_NOFOLLOW` and `O_NOFOLLOW` on leaf files, preventing symlink escapes into host paths.
   - Provide `view_file_numbered` (max 1001 lines per read, 2 MiB file cap), `fast_workspace_search` (pattern length `1..=500`), and `surgical_file_edit` (require unique `old_text` match, check syntax for `.py`/`.json`/`.yml`/`.yaml`, snapshot before write, fail on concurrent modification between read and write, return unified diff).
5. **Coder / Reviewer Verification Loop & Log Compaction (`NexusGenSandboxNodeSpec`, `NexusGenSandboxRunner`):**
   - Run two-role coder/reviewer iterations up to `max_rounds` (`1..=50`) with bounded `timeout_seconds` (`1..=3600`), running `verification_command`, compacting output logs (`compact_log` with `16_384` char hard ceiling preserving deduplicated failure/traceback lines), mounting upstream artifacts (1 MiB input budget, 50 MiB upload budget), and honoring `on_unverified` (`deliver_labeled` | `fail`) and `on_failure` (`fail` | `continue`) plus `min_added_tests`.
   - Emit `NEXUSGEN_CODE_GENERATED` and `SANDBOX_TERMINAL_OUTPUT` JSONL events (documenting and schema-validating new event types in `docs/events.md` and `tests/fixtures/`).

### Completion Criteria

- `ComputerProvider` trait in Rust with production `DockerSandboxProvider` (using `docker` subprocess arguments cleanly via `tokio::process::Command` with fixed timeouts, no shell string interpolation) and pure-Rust `ExecutionPolicy` evaluator.
- Hardened file tools (`view_file_numbered`, `surgical_file_edit`, `fast_workspace_search`) and sandbox shell tool wired into the Stage 1/2 `ApprovalPolicy` and `ToolRegistry`.
- `WorkflowNode::Sandbox` (`NexusGenSandboxNodeSpec`) enabled in the Step 1 Flow compiler/executor with guaranteed cleanup on completion, cancellation, error, or panic drop.
- Automated tests covering:
  - Unit + boundary tests for `ExecutionPolicy` (`DENY > ASK > ALLOW`), command splitting errors, secret-path detection, and `compact_log` 16,384-char ceiling + deduplication.
  - Docker CLI command builder tests asserting all mandatory hardening flags (`--network none`, `--cap-drop ALL`, `--read-only`, `--security-opt no-new-privileges:true`, `--pids-limit 256`, resource limits, non-root user, scope labels).
  - Symlink escape rejection during file open/edit, stale-read rejection on `surgical_file_edit`, and `ASK` command pausing -> atomic single-use `approve`/`resume` execution.
  - Live Docker integration suite (gated cleanly by daemon availability) verifying container creation, execution, `retire_run` removal, and `reap_orphans`.

### Risks

- **Container leaks after `SIGKILL` or process crash:** If `nexus-albaai` is killed hard mid-run, `Drop` destructors do not run. Implement an explicit startup/periodic `reap_orphans` pass keyed on `nexus.run_id` container labels combined with SQLite run status checks.
- **Policy bypass via complex shell syntax:** Static token/regex rules (`ExecutionPolicy`) can be evaded inside `/bin/sh` (`eval`, encoding, globs). Treat `ExecutionPolicy` strictly as an approval/UX guard, relying on Docker `--network none`, `--cap-drop ALL`, `--read-only`, non-root UID, and workspace bind mounts as the true security boundary.
- **Host dependency on Docker daemon & macOS Seatbelt caveats:** Docker is an external daemon not always present in headless environments; keep provider failures explicit when Docker is absent. If optional macOS Seatbelt (`local_provider.py`) is later added, document plainly that Apple deprecated `sandbox-exec` and it lacks Docker's memory/process-count isolation and readable-rootfs containment.

---

## 5. Step 4: RAG

### Legacy Behavior to Preserve

Source paths in `NexusAgent`:
- `packages/rag-engine/nexus_rag_engine/engine.py` (`KnowledgeScope`, `EmbeddingAccess`, `RetrievalHit`, `MultiTenantRAGEngine`, `MAX_DOCUMENT_BYTES`)
- `packages/rag-engine/nexus_rag_engine/tools.py` (`TenantKnowledgeRetrievalTool`, `KnowledgeQuery`)
- `packages/agent-runtime/flows/compiler.py` (`RAGNodeSpec` execution branch)

Contracts and runtime behaviors to preserve:
1. **Mandatory Multi-Tenant Scope Filtering (`KnowledgeScope`, `scope_filter`):**
   - Bind every vector point to `tenant_id`, `workspace_id`, `knowledge_base_id`, `document_id`, `filename`, `text`, and `chunk_index`.
   - Enforce Qdrant payload filtering on both `prefetch` branches and top-level `query_filter` (`tenant_id`, `workspace_id`, `knowledge_base_id`, plus optional `document_id`).
   - Re-verify every returned Qdrant point payload in-process against `(tenant_id, workspace_id, knowledge_base_id)` and raise a hard error on any scope mismatch (defense-in-depth against backend/index misconfiguration).
2. **Document Ingestion, Chunking, Quota Guard, and Rollback:**
   - Reject uploads exceeding `MAX_DOCUMENT_BYTES = 20 * 1024 * 1024` (20 MiB) before parsing.
   - Check tenant vector count across collections against `max_chunks` prior to indexing; abort cleanly on quota overflow.
   - Split text into bounded units (`chunk_size = 384`) with semantic cosine boundary grouping (`similarity_threshold = 0.72`).
   - Name Qdrant collections deterministically by embedding configuration (`nexus_knowledge_{sha256[:16]}_{dimension}`) with named vectors `"dense"` (cosine) and `"lexical"` (sparse), creating payload keyword indexes (`tenant_id`, `workspace_id`, `knowledge_base_id`, `document_id`).
   - Derive deterministic point UUIDs via UUIDv5 (`document_id:chunk_index`).
   - Track document status in SQLite (`processing` -> `ready` / `failed`); if Qdrant upsert fails, delete partial points for `document_id` and mark the SQLite document row `failed`.
3. **Hybrid Dense + Lexical Retrieval (RRF):**
   - Validate non-empty query string and `1 <= limit <= 100`.
   - Issue Qdrant `query_points` with two `Prefetch` branches (`"dense"` cosine query vector and `"lexical"` hashed/sparse term vector, prefetch limit `max(limit * 4, 20)`) fused via Reciprocal Rank Fusion (`RRF`).
4. **Agent Tool & Flow Node Exposure:**
   - Register `nexus_knowledge_retrieve` tool (accepting `{ "query": string }`) returning JSON list of `RetrievalHit` items (`document_id`, `filename`, `text`, `score`, `chunk_index`).
   - Enable `WorkflowNode::Rag` (`RAGNodeSpec`) in Flow compiler, rendering `query_template` against workflow input/upstream outputs.

### Completion Criteria

- Pure-Rust Qdrant HTTP/REST adapter built on existing `reqwest` + `serde_json` (avoiding gRPC client bloat) and embeddings endpoint client (`POST /embeddings`).
- Chunking + sparse lexical vectorizer + ingest/rollback + hybrid RRF retrieval matching legacy `MultiTenantRAGEngine` behavior.
- `WorkflowNode::Rag` enabled in Flow execution and `nexus_knowledge_retrieve` available as a scoped `Tool` for single agents and Step 2 Crews.
- CLI / library commands to ingest UTF-8 text/Markdown/CSV documents into a scoped SQLite+Qdrant knowledge base and query them. (Binary PDF/DOCX extraction enabled only when an allowlist-clean parser/extractor is verified.)
- Automated tests (against local mock Qdrant/embeddings HTTP servers plus optional live Qdrant suite):
  - Reject payloads > 20 MiB and invalid `limit` (`0`, `101`, blank query).
  - Verify `max_chunks` tenant quota rejection before embedding/upsert.
  - Verify Qdrant requests always embed mandatory `tenant_id`/`workspace_id`/`knowledge_base_id` filters on every `prefetch` and `query_filter`.
  - Inject a cross-tenant point into mock Qdrant response and assert immediate runtime rejection.
  - Force upsert failure and assert point cleanup + SQLite document `failed` state.

### Risks

- **Binary document parser licensing & security:** Rust PDF/DOCX libraries (`pdf-extract`, XML/zip stacks) and tokenizers can pull disallowed licenses or parser vulnerabilities on untrusted documents. Audit candidate crates with `cargo deny check licenses`, start with strict UTF-8 text/Markdown/CSV in core Rust, and sandbox or isolate untrusted binary format extraction.
- **Qdrant gRPC vs REST dependency footprint:** Pulling `qdrant-client` adds `tonic`/`prost`/`h2` and complex build dependencies (`protoc`). Using Qdrant's official REST endpoints over existing `reqwest` preserves all required hybrid RRF capabilities with minimal compile time and license surface.
- **Embedding cost & latency spikes:** Re-embedding large batches serially slows ingestion and risks HTTP timeouts. Cap document size at 20 MiB, enforce `max_chunks` before calling `/embeddings`, batch requests cleanly with explicit per-request timeouts, and wire embedding calls through Step 5's `ModelGateway` once landed.

---

## 5. Step 5: Gateway

### Legacy Behavior to Preserve

Source paths in `NexusAgent`:
- `packages/model-gateway/nexus_model_gateway/router.py` (`Provider`, `BillingMode`, `METERED`, `SUBSCRIPTION_INCLUDED`, `ModelRate`, `ZERO_RATE`, `ResolvedCredential`, `GatewayRequest`, `GatewayResult`, `BudgetExceeded`, `LiteLLMRouter`, `price_for`)
- `packages/model-gateway/nexus_model_gateway/connection.py` & `provider_presets.py` (`effective_api_base`)
- `packages/model-gateway/nexus_model_gateway/auto_distill.py` (`GoldenTrace`, `GoldenTraceCollector`, `DistillationScore`)
- `packages/agent-runtime/bots/gateway.py` (`GatewayBotResponder`)

Contracts and runtime behaviors to preserve:
1. **Unified Multi-Provider Wire Routing (`Provider`, `ResolvedCredential`):**
   - Support the ten declared provider identifiers (`openai`, `anthropic`, `gemini`, `deepseek`, `openrouter`, `ollama`, `vllm`, `groq`, `mistral`, `together_ai`) across OpenAI-compatible wire (`wire: "openai"`) and native provider formats (`wire: "native"` for Anthropic Messages and Gemini GenerateContent), replacing Stage 1's single OpenAI client and eliminating Python `litellm`.
   - Enforce `effective_api_base` precedence (`request_base` -> `credential.api_base` -> vendor default): when a workspace supplies a BYOK credential for non-local providers, **never** fall back to `NEXUS_<PROVIDER>_API_BASE` so tenant BYOK keys cannot leak to a platform proxy host (`connection.py`).
   - Enforce HTTPS on caller/tenant `api_base` overrides, permitting plain HTTP solely for server-approved local `ollama` / `vllm` base URLs (`self._allowed_local_bases` allowlist check).
   - Enforce `billing_mode`: `"byok"` must fail immediately if no workspace credential resolves (`ValueError` equivalent) before network dispatch.
2. **Pre-Call Budget Reservation, Exact Finalization, and Atomic Release:**
   - Compute deterministic pricing using fixed-point `10^-8` USD precision (`MONEY = 0.00000001`, `ROUND_UP`) mirroring `ModelRate::cost(prompt_tokens, completion_tokens, markup)` (`markup >= 1`).
   - Bypass per-token spend when `billing_mode == "SUBSCRIPTION_INCLUDED"` or model pricing maps to `ZERO_RATE`, while still logging usage metrics.
   - When metered (`platform` mode with no BYOK override and positive ceiling cost): atomically reserve `rate.cost(max_input_tokens, max_output_tokens, markup)` inside a SQLite transaction before calling the model (`BudgetExceeded` on insufficient credit); on provider response, `finalize` actual cost (`prompt_tokens`, `completion_tokens`) plus `UsageRecord` (`tenant_id`, `workspace_id`, `workflow_id`, `run_id`, `agent_name`, `provider`, `model`, `prompt_tokens`, `completion_tokens`, `total_cost_usd`, `latency_ms`, `billing_mode`); on any error or cancellation, `release` the unused reservation cleanly.
   - Validate returned usage (`prompt_tokens >= 0`, `completion_tokens >= 0`), flagging `usage_limit_exceeded` when counts exceed configured ceilings.
3. **Secret Hygiene & Transport Hardening:**
   - Preserve Stage 2 guarantees across all new providers: credentials reside only in memory, `Debug` outputs redact API keys/headers, and zero authorization headers or keys are ever written to SQLite `usage_records`, checkpoints, or logs.
4. **Shadow Distillation & Usage Telemetry Foundations (`GoldenTraceCollector`):**
   - Capture eligible `GoldenTrace` records (`schema_valid && human_approved && retries == 0`), support few-shot example extraction, and compute `DistillationScore` eligibility (`schema_validity_rate >= 0.95`, `semantic_similarity >= 0.95`, `cost_reduction >= 0.40`).

### Completion Criteria

- Native `ModelGateway` trait and implementation supporting OpenAI-compatible, Anthropic native, and Gemini native HTTP requests (including tool definitions, tool call parsing, and normalized token usage extraction) built on existing `reqwest` + `rustls-tls`.
- Transactional SQLite `BudgetStore` using integer `i128`/`u128` nanocent (`10^-8` USD) arithmetic with `TransactionBehavior::Immediate` preventing concurrent overspend across runs in the same tenant/workspace.
- Full replacement of direct `OpenAiModelClient` calls in Stage 1 agent loop, Step 1 Flows, Step 2 Crews, and Step 4 RAG embeddings with `ModelGateway`.
- Strict URL/IP SSRF guard rejecting non-HTTPS external URLs, private/link-local/metadata IPs, and unapproved local provider bases.
- Integration tests covering:
  - Fixed-point pricing parity against `ModelRate.cost` (`ROUND_UP` to `10^-8` USD).
  - Pre-flight `BudgetExceeded` abort without issuing HTTP calls; concurrent budget reservation races preventing negative balances; HTTP error / timeout cleanly triggering `release` of reserved credit; success refunding unused reserved margin on `finalize`.
  - `byok` missing credential rejection; `SUBSCRIPTION_INCLUDED` / `ZERO_RATE` uncharged metering; SSRF rejection of unapproved `http://` or metadata `api_base`.
  - Mock HTTP wire normalization tests for OpenAI, Anthropic, and Gemini tool calls, plus Stage 2 secret leakage test across usage tables and logs.

### Risks

- **SSRF and DNS rebinding via tenant-supplied `api_base`:** URL scheme checks alone do not stop hostnames resolving to `127.0.0.1`, RFC1918, or `169.254.169.254`. Resolve and validate target IP addresses inside the HTTP client connector (or custom resolver) and disable cross-origin HTTP redirects on model calls.
- **Floating-point rounding drift in billing:** Using `f64` violates deterministic ledger accounting. Use integer fixed-point (`10^-8` USD) arithmetic strictly throughout reservation, finalization, and aggregation.
- **Native wire divergences across providers:** Anthropic (`tool_use` blocks, mandatory alternating roles, separate `system` field) and Gemini (`functionCall`/`functionResponse` parts) handle multi-turn tool histories and usage metadata differently from OpenAI. Bugs in message translation can silently drop tool results or corrupt parallel tool IDs; verify each wire adapter with round-trip multi-turn tool fixtures.
- **Reservation leaks on abnormal termination:** Uncaught task cancellation or panic between `reserve` and `finalize` can strand reserved tenant credit unless backed by a `Drop`/unwind release guard and stale-reservation reconciliation in SQLite.

---

## 6. Cross-Cutting Dependencies and Open Blockers

1. **License Decisions from Stage 2 (`deny.toml`):** Adding HTTP/TLS, JSON Schema, YAML, or PDF crates in Steps 2–4 depends on user resolution of the Stage 2 `cargo deny check licenses` report (`Unicode-3.0`, `(MIT OR Apache-2.0) AND Unicode-3.0`, `CDLA-Permissive-2.0`), especially because Step 4 Qdrant HTTP and Step 5 Gateway share the `reqwest`/`rustls` stack already flagged on `webpki-roots` (`CDLA-Permissive-2.0`) and `url`/`idna` (`Unicode-3.0`).
2. **Stage Ordering Discipline:** Keep Step 1 (`WorkflowCompiler` + state machine) schema-strict on `Crew`, `Sandbox`, and `Rag` node variants from day one so Steps 2–4 plug into stable typed node-executor interfaces rather than ad-hoc strings or dynamic JSON maps.
3. **No Stage 3 Implementation Yet:** This roadmap covers planning only; Stage 3 Rust modules will be implemented incrementally only after Stage 2 hardening review and license decisions are finalized.
