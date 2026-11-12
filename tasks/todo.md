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
- [ ] G2 Rust Engine: WorkflowSpec executor in Alba (Crew sequential/hierarchical, ApprovalGate, Router, loop_back, ParallelBranch).
- [ ] G3 Compatible API: axum serves `/api/v1` + SSE; apps/web completes one run end-to-end.
- [ ] G4 Verification & Hooks: Validator, claims, evidence, Deny/Ask/Allow policy.
- [ ] G5 Memory: Checkpoints + capsules (scope firewall) + embedded vectors + conversation compaction.
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
