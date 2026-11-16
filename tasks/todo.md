# Plan: Lightweight Alba Agent (UI apps/web + single Rust backend)

## Decisions Locked
- UI: Keep `NexusAgent/apps/web` (Next.js 15, Tailwind v3). No framework change, no Tailwind v4 upgrade.
- Backend: Rust only (axum + tokio), single binary. No Go in v1.
- Storage: SQLite WAL replaces Postgres + Redis. Embedded vector store replaces Qdrant. In-process task queue and SSE broadcast.
- Workflow execution: In-engine WorkflowSpec executor replacing CrewAI.
- Model adapters: Direct OpenAI-compatible + Anthropic native adapters replacing LiteLLM.
- Packaging: Tauri (native system webview).

## Baseline Measured (NexusAgent)
- Core code is compact: `apps/` 20MB (`api` 1.5MB, `web` 1.8MB), `packages/` 1.5MB (~42k lines Python).
- Bloat is runtime-driven: Python, CrewAI, LiteLLM, Celery, Postgres, Redis, Qdrant, Electron (6 processes).
- Install size: 1.15 GB lean / 1.50 GB full bundle (documented in README). Source: 25.9 MB.
- Idle RAM: ~274 MB - 403 MB in default desktop lean mode (measured via `ps`); ~850 MB - 1,150 MB in full Postgres mode (documented).
- Startup time: Median 11.56s (min 7.00s, max 122.06s across all 32 starts in `startup.log`). Cold start ~60s (documented in README).
- Sample run time: 27.22s (measured from `worker.log`).

## Target Replacements
| Heavy Component | Replaced By |
|---|---|
| Python / FastAPI / Celery | Single Rust binary (axum + tokio), in-process async worker queue |
| Postgres + Redis | SQLite WAL (checkpoints, audit log, runs, capsules) + tokio broadcast for SSE |
| Qdrant | SQLite-vec / embedded vector storage |
| CrewAI Flow | Custom Rust WorkflowSpec executor (sequential, hierarchical, gates, routers) |
| LiteLLM | Native Rust OpenAI-compatible and Anthropic adapters |
| Electron | Tauri (system webview) |

## Retained Strengths
- Schemas: AgentSpec, TaskSpec, WorkflowSpec (Serde types generated from core-schema).
- Artifact contract: claims hook + automated machine verification.
- Approvals & Checkpoints: atomic approvals, author != approver separation, fail-closed audit, time-travel, fork.
- Memory: KnowledgeCapsule, scope firewall, negative-transfer detection.
- Studio Blueprints, SKILL.md progressive loader, Deny/Ask/Allow execution policy + seatbelt sandbox.
- apps/web UI and the REST/SSE /api/v1 contract.

## Phases
- [x] G0 Baseline: Measure NexusAgent (install size, idle RAM, startup time, sample run time); run pnpm lint/check/test + pytest. Record in `tasks/baseline.md`.
- [x] G1 Contract: Catalog `/api/v1` endpoints used by `apps/web`; define minimal v1 contract; generate Rust Serde types from core-schema; design SQLite WAL schema.
- [x] G2 Rust Engine: WorkflowSpec executor in Alba (Crew sequential/hierarchical, ApprovalGate, Router, loop_back, ParallelBranch).
  - [x] S1 Validation: WorkflowSpec graph validator (acyclicity except declared loop_back, reachable End, router targets, agent resolution, explicit rejection of unsupported kinds).
  - [x] S2 Linear Crew Run: Start -> Crew (sequential) -> End with mock model, SQLite checkpoint per node, events emitted, exhaustive state enum.
  - [x] S3 Approval Gate: ApprovalGate pauses run (AwaitingApproval), CLI approve/deny, author != approver separation, fail-closed audit.
  - [x] S4 Resume & Crash Safety: Tool idempotency journal in SQLite, resume from checkpoint, SIGKILL recovery test.
  - [x] S5 Router, loop_back, ParallelBranch: Router rule evaluation, max_loop_iterations enforcement, join=all and join=any.
  - [x] S6 Hierarchical Crew: Manager agent delegates to specialist agents via spec.
  - [x] S7 Fork & Time-Travel: fork run from checkpoint, rewind to checkpoint, hash compare.
  - [x] S8 CLI & Sample Workflow: CLI commands and 3-agent approval sample, end-to-end scenario automated in cargo test.
  - [x] CR1 Correction Round 1: Real SIGKILL (child.kill().await 20/20 iterations), break tests, failpoints, zero comments, MACHINE_VERDICT PASS.
- [x] G3 Compatible API: axum serves `/api/v1` + SSE; apps/web completes one run end-to-end.
- [x] G4 Verification & Hooks: Validator, claims, evidence, Deny/Ask/Allow policy.
- [x] G5 Memory: Checkpoints + capsules (scope firewall) + embedded vectors + conversation compaction.
- [ ] G6 Tauri Packaging: Single binary + static web build; remeasure against baseline.
- [ ] G7 UI: Port Fork / Checkpoint timeline / separated-approver Approval into apps/web (Tailwind v3).
- [ ] G8 Post-v1: Architect/team generator, team evolution, consensus, full RAG, MCP hub.

## Success Criteria (Empirical)
- Install size, idle RAM, cold start, sample run time strictly beat G0 baseline.
- All cargo verification gates pass: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo deny check`.
- Zero comments policy enforced across all source, test, and config files.
- All apps/web checks pass: `pnpm lint`, `pnpm check`, `pnpm test`.

## Review

### Phase G0 (Baseline) - Corrected & Verified
- Verified on NexusAgent:
  - `pnpm lint`: PASS (0 errors).
  - `pnpm check`: PASS (tsc clean).
  - `pnpm test`: PASS (89 passed in 791ms).
  - `pytest packages/`: PASS (89 passed, 2 skipped in 0.57s; run with `PYTHONDONTWRITEBYTECODE=1` and `-p no:cacheprovider`).
- Baseline metrics recorded in `tasks/baseline.md`:
  - Install Size: 1.15 GB lean / 1.50 GB full bundle (documented in `apps/desktop/README.md:38-51`, not measured as no bundle is built on disk). Source code: 25.9 MB measured via `du`.
  - Idle RAM (Default Lean Desktop): ~274 MB – 403 MB measured via `ps` (Next.js web: 22.6 MB, Electron main/GPU/renderer: 196.1 MB, Python API: 50.5 MB - 180 MB).
  - Idle RAM (Postgres Full Stack): ~850 MB – 1,150 MB documented architecture estimate (6 services + Electron).
  - Startup Time (Warm): Measured from `~/Library/Application Support/@nexus/desktop/logs/startup.log` across all 32 runs: Median 11.56s (min 7.00s, mean 23.04s, max 122.06s; 2026-11-09 13:24 start took 19.24s).
  - Startup Time (Cold): Documented in README as "takes about a minute"; `initdb` took 27.08s and first start 11.11s; not verified by any single 60.0s log line.
  - Sample Run Time: 27.22s measured from `worker.log` (`Task nexus.run_workflow succeeded in 27.221581999998307s`).
- Verdict: PASS.

### Phase G1 (Contract & Schemas) - Corrected & Verified
- Contract analysis:
  - 63 unique route/method signatures audited across `apps/web/src` (`lib/api.ts`, `lib/sse.ts`, components, hooks).
  - Minimal v1 set established: 24 core endpoints (including explicit `/api/v1/runs/:id/resume` for crash recovery and engine resumption, and `/api/v1/approvals/:id/resolve` for gate resolution).
  - Contract documents synced and identical: `alba/docs/api-contract.md` and `tasks/api-contract.md`.
  - Full specifications provided for all 24 minimal v1 endpoints.
- Generated Rust Serde types:
  - `src/schema/agent.rs`: `AgentSpec`, `AgentTier`.
  - `src/schema/task.rs`: `TaskSpec`.
  - `src/schema/node.rs`: `WorkflowNode` (`Start`, `Crew`, `Sandbox`, `ApprovalGate`, `Router`, `Route`, `RAG`).
  - `src/schema/workflow.rs`: `WorkflowSpec`, `WorkflowEdgeSpec`, `WorkflowModesSpec`, `PathRuleSpec`, `NodePosition`, `LearningEdgeSpec`.
  - `src/schema/capsule.rs`: `KnowledgeCapsuleSpec`, `PropagationScope`, `SensitivityLevel`.
  - `src/schema/approval.rs`: `ApprovalRequestSpec`.
  - `src/schema/mod.rs`: Full module exports.
- Schema roundtrip tests:
  - `tests/schema_contract_tests.rs`: 5 tests passing (AgentSpec, TaskSpec, ApprovalRequestSpec, KnowledgeCapsuleSpec, WorkflowSpec).
  - Clippy fix: Fixed bool assert comparison in `tests/schema_contract_tests.rs:243`.
- SQLite WAL schema:
  - Complete DDL replacing Postgres, Redis, and Qdrant documented in `docs/api-contract.md`.
- Quality Gates in `alba/`:
  - `cargo fmt --check`: PASS.
  - `cargo clippy --all-targets -- -D warnings`: PASS.
  - `cargo test`: PASS (43 tests passed across 3 test suites).
  - `cargo deny check`: PASS (advisories ok, bans ok, licenses ok, sources ok).
  - `./scripts/check_no_comments.sh`: PASS (0 comments found across 94 checked files).
- Git commit hygiene:
  - Commit history cleanly split into 3 atomic commits on `feat/g1-contract`:
    1. `7175317`: `chore: import albaAI core unchanged`
    2. `c68e2e9`: `feat(schema): G1 contract, types, SQLite schema`
    3. `daddc9e`: `fix(review): address G0/G1 review findings F1-F5`
  - Backup branch retained: `backup/feat-g1-contract-original` (`40de731`).
- Verdict: PASS.

### Phase G2 (Workflow Engine & CLI) - Implemented & Verified
- Engine implementation:
  - `src/workflow/validation.rs`: WorkflowSpec validator with topological cycle detection, reachability, router targets, and explicit rejection of RAG/Sandbox.
  - `src/workflow/state.rs`: Exhaustive state enums (`WorkflowRunState`, `WorkflowNodeState`) with zero wildcard matches.
  - `src/workflow/engine.rs`: Full `WorkflowExecutor` interpreting JSON/YAML specs without hardcoded logic; supports sequential and hierarchical crew (`manager_agent_id`), approval gates, router nodes with loopback iteration bounds, parallel branches (`JoinMode::All` and `JoinMode::Any`), time-travel forking, and idempotency journal replay.
  - `src/checkpoints/sqlite.rs`: Step checkpoints, atomic `claim_workflow_approval` (`BEGIN IMMEDIATE`), author != approver separation, `fork_workflow_run`, and tool journal (`executing -> consumed`).
  - `src/cli/commands.rs`: Added `--workflow` support to `run` and `resume`, `--actor` support to `approve` and `deny`, workflow status reporting in `status`, and workflow run forking in `fork`.
- Automated test coverage across 8 vertical slices:
  - `tests/validation_tests.rs`: 15 unit tests covering validation rules, unsupported node rejections, cycles, and loop_back edges.
  - `tests/workflow_s2_tests.rs`: Linear sequential crew execution end-to-end with step checkpoints and runtime events.
  - `tests/workflow_s3_tests.rs`: Approval gate pause (`AwaitingApproval`), CLI resolution (`allow_once` and `deny`), self-approval rejection, and fail-closed audit log verification.
  - `tests/workflow_s4_tests.rs`: SQLite tool journal idempotency, recovery from crashed tool calls, and WAL integrity verification.
  - `tests/workflow_s5_tests.rs`: Router rule evaluation (`equals`, `contains`, `is_empty`, `is_not_empty`), loopback execution bounded by `max_loop_iterations`, and parallel branch joins.
  - `tests/workflow_s6_tests.rs`: Hierarchical crew orchestration with manager directives passed down to specialist agents.
  - `tests/workflow_s7_tests.rs`: Historical checkpoint forking, trajectory branch persistence, parent run immutability assertion, and divergent execution.
  - `tests/workflow_s8_e2e_tests.rs`: Complete 5-step scenario on 3-agent workflow (`lead_researcher -> approval_gate -> deployer + verifier`) via CLI binary (`run`, `status`, `approve`, `resume`, `fork`, `deny`), exact SQLite assertions (10 parent checkpoints, 6 forked checkpoints, consumed/denied approvals, audit logs, trajectory branches), and 20/20 flakiness test.
- Empirical Verification Gates in `alba/`:
  - `cargo fmt --check`: PASS.
  - `cargo clippy --all-targets -- -D warnings`: PASS (0 warnings).
  - `cargo test`: PASS (70 passed, 0 failed across 10 test suites in 4.90s).
  - `cargo deny check`: PASS (advisories ok, bans ok, licenses ok, sources ok).
  - `./scripts/check_no_comments.sh`: PASS (0 comments found across 106 checked files).
- Verdict: PASS.

### Phase G3 (Compatible API & Web UI Integration) - Implemented & Verified
- Implementation:
  - REST API & SSE Broadcast: axum serves `/api/v1` routes and SSE event streams.
  - Integration: `apps/web` executes end-to-end runs with human approval resolution.
- Test suites:
  - `tests/api_auth_tests.rs`: 8 passed.
  - `tests/api_contract_tests.rs`: 13 passed.
  - `tests/api_runs_tests.rs`: 6 passed.
  - `tests/api_sse_tests.rs`: 5 passed.
  - `tests/api_approvals_tests.rs`: 3 passed.
- Verdict: PASS (Commit `2ca481731db0`).

### Phase G4 (Verification, Claims, Evidence, Policy & Sandbox) - Implemented & Verified
- Mandatory Verification Gates:
  - `cargo fmt --check`: exit code 0.
  - `cargo clippy --all-targets -- -D warnings`: exit code 0.
  - `cargo test -- --test-threads=2`: exit code 0 (161 passed, 0 failed across 24 test binaries).
  - `cargo deny check`: exit code 0 (advisories ok, bans ok, licenses ok, sources ok).
  - `./scripts/check_no_comments.sh`: exit code 0 (0 comments across 137 checked files).
- Component Implementation and Evidence Citations:
  - **Validator**: `src/verification/validator.rs:1-320` (`ArtifactContractHook::validate_file`, signature checks for `.png`, `.pdf`, `.zip`, `.gz`, `.tar`, `.sqlite`, size bounds, traversal/symlink guards). Tests: `tests/g4_artifact_tests.rs:1-323` (15 tests, e.g. `test_artifact_empty_fails` line 36, `test_artifact_wrong_signature_fails` line 52, `test_gzip_signature_valid_and_wrong_bytes` line 243).
  - **Claims**: `src/verification/claims.rs:1-150` (`verify_text_claims`, structured/narrative claim validation). Tests: `tests/g4_claims_tests.rs:1-170` (6 tests, e.g. `test_claimed_missing_file_fails_node_and_run` line 12, `test_claimed_present_file_passes_run` line 51).
  - **Evidence**: `src/workflow/engine.rs:850-1060`, `src/checkpoints/sqlite.rs:753-850`, `src/checkpoints/types.rs:96-120` (`insert_run_evidence`, `mark_run_verified`, `check_artifact_tampering`, `output_sha256`). Tests: `tests/g4_evidence_tests.rs:1-1550` (17 tests, e.g. `test_workflow_engine_real_verification_command_e2e` line 814, `test_artifact_replaced_after_verification_fails_recheck_persists_reason` line 691, `test_verification_store_error_writes_audit_and_leaves_unverified` line 1356, `test_unreadable_artifact_recorded_as_error_and_keeps_unverified` line 1412, `test_sandbox_node_verification_command_executed_and_verified` line 1471, `test_verification_command_policy_denied_sudo_true` line 1028).
  - **Deny/Ask/Allow Policy & Sandbox**: `src/verification/policy.rs:1-350` (`ExecutionPolicyEngine::evaluate`), `src/tools/run_cmd.rs:150-290`, `src/tools/file_read.rs`, `src/tools/file_write.rs`, `src/sandbox/macos.rs:20-90`. Tests: `tests/g4_policy_tests.rs:1-700` (2 tests, 72-rule matrix table at line 29, API wiring at line 519), `tests/g4_sandbox_tests.rs:1-298` (8 tests, e.g. `test_seatbelt_containment_working_runner` line 224, `test_read_ssh_blocked_and_audited` line 135, `test_write_outside_workspace_blocked_and_audited` line 110, `test_curl_url_from_run_command_blocked_and_audited` line 165).
- Verdict: PASS (Commit `db76b40` on `feat/g4-verify`).

### Phase G5 (Memory for Agent Teams) - Implemented & Verified
- Mandatory Verification Gates:
  - `cargo fmt --check`: exit code 0.
  - `cargo clippy --all-targets -- -D warnings`: exit code 0.
  - `cargo test -- --test-threads=2`: exit code 0 (185 passed, 0 failed across 27 test binaries).
  - `cargo deny check`: exit code 0 (advisories ok, bans ok, licenses ok, sources ok).
  - `./scripts/check_no_comments.sh`: exit code 0 (0 comments across 150 checked files).
- Component Implementation and Evidence Citations:
  - **KnowledgeCapsule Store & Schema**: `src/checkpoints/sqlite.rs:600-680`, `src/checkpoints/types.rs:180-210`, SQLite migration 10 (`knowledge_capsules`). Tests: `tests/g5_memory_s1_tests.rs:1-120` (5 tests passed: migration, insert, duplicate rejection, list with scope filtering).
  - **Scope Firewall & Redaction**: `src/memory/firewall.rs:1-350` (5 propagation scopes, RedactionEngine with 22 secret/PII regexes + path sanitization, domain compatibility, audit logging to `memory.cross_scope_denied`). Tests: `tests/g5_memory_s2_tests.rs:1-250` (6 tests passed: 20+ secret patterns, 10 benign non-redaction samples, cross-agent isolation, cross-tenant isolation).
  - **Episodic Knowledge Distiller & Vector Retrieval**: `src/memory/distiller.rs:1-180`, `src/memory/vector.rs:1-160`, SQLite migration 11 (`capsule_vectors`). Tests: `tests/g5_memory_s3_tests.rs:1-180` (3 tests passed: data-driven triggers, below-threshold refusal, SQL tenant & workspace isolation).
  - **Negative-Transfer Detector & Circuit Breaker**: `src/memory/transfer.rs:1-210` (rolling baseline formula, weights, `quarantine_if_negative` setting `negative_transfer = 1`). Tests: `tests/g5_memory_s4_tests.rs:1-150` (3 tests passed: impact scoring, flip rates, quarantine circuit breaker).
  - **Conversation Compaction**: `src/memory/compaction.rs:1-300` (atomic `MessageGroup`, system message & tail preservation, tool-call integrity, summary redaction, `CompactionTrace` source pointers). Tests: `tests/g5_memory_s5_tests.rs:1-210` (3 tests passed: mock ModelClient, atomic tool family, 1000 random histories property test with 0 orphaned tool messages).
  - **Engine Memory Injection & Post-Verification Distillation**: `src/workflow/engine.rs:360-410, 940-1010` (untrusted data prompt labeling bounded to 2048 chars, post-verification distillation only on `is_verified == true`). Tests: `tests/g5_memory_s6_tests.rs:1-220` (1 test passed: Run 1 verified writes capsule, Run 2 injects, Run 3 cross-tenant isolated, Run 4 unverified writes nothing).
  - **Crash Safety & Reviewer Break Attempts**: `tests/g5_memory_s7_tests.rs:1-290` (6 tests passed: 20/20 transaction drop leaves 0 partial rows, prompt injection in capsule, split secret, enormous capsule size, concurrent duplicate write, compaction on mid-tool_call history).
- Verdict: PASS (Commit `ef5e4f4` on `feat/g5-memory`).

### Phase G6 (Desktop Packaging & Release Baseline) - Implemented & Verified
- Mandatory Verification Gates:
  - `cargo fmt --check`: exit code 0.
  - `cargo clippy --all-targets -- -D warnings`: exit code 0.
  - `cargo test -- --test-threads=2`: exit code 0 (199 passed, 0 failed across 36 test targets).
  - `cargo deny check`: exit code 0 (advisories ok, bans ok, licenses ok, sources ok).
  - `./scripts/check_no_comments.sh`: exit code 0 (0 comments across 155 checked files).
  - `pnpm lint` (in `alba/web`): exit code 0 (0 errors, 0 warnings).
  - `pnpm check` (in `alba/web`): exit code 0 (0 errors).
  - `pnpm test` (in `alba/web`): exit code 0 (89 passed, 0 failed).
- Component Implementation and Evidence Citations:
  - **S1: Static Web Export & In-Process Backend Hosting**: Configured Next.js 15 static export (`output: "export"`) in `alba/web`, enabled `fs` feature in `tower-http 0.6.2`, mounted `ServeDir` in `src/server/mod.rs` protecting `/api/*` endpoints. Tests: `tests/g6_packaging_s1_tests.rs` (2 tests ok).
  - **S2: Tauri v2 Native Shell & Backend Launch**: Configured `src-tauri` with Tauri v2, initialized SQLite store in OS app data dir (`ALBA_DATA_DIR`), spawned loopback Axum listener writing `backend_port`, loaded local static UI with strict CSP. Tests: `tests/g6_packaging_s2_tests.rs` (1 test ok).
  - **S3: App Lifecycle, Single-Instance & SIGKILL Recovery**: Unix domain socket (`alba.sock`) IPC focus handoff on duplicate launch (exits code 0), graceful exit cleanup of sockets/ports, verified SQLite `PRAGMA integrity_check` and run resumption post-SIGKILL. Tests: `tests/g6_packaging_s3_tests.rs` (2 tests ok).
  - **S4: Hardening: Loopback Bind, Strict CSP & Offline Execution**: Strict `127.0.0.1` binding, local-only CSP forbidding external domains/CDNs in `tauri.conf.json`, restricted capabilities in `default.json`, verified full offline workflow execution without network access. Tests: `tests/g6_packaging_s4_tests.rs` (3 tests ok).
  - **S5: macOS Release App Bundle**: Built standalone `.app` bundle via `tauri build --bundles app`. Verified executable launches, serves `/health` 200 OK, root HTML, and process tree contains 0 Python, 0 Node.js, 0 external databases. Tests: `tests/g6_packaging_s5_tests.rs` (1 test ok).
  - **S6: Baseline Remeasurement & Canaries**: Full remeasurement against G0 baseline documented in `tasks/reports/G6-report.md`. All 9 canaries in `tasks/verify/canaries/G6.json` verified DETECTED.
- Verdict: PASS.
