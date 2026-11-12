# Phase G3 Report: Compatible API (axum /api/v1 + SSE) for apps/web

## 1. Header
- **Phase**: G3 (Compatible API & Web UI Integration)
- **Branch**: feat/g3-api
- **Final Commit Hash**: 9ceb2d55ac366a1ccfe688b6adfea65a93dd5339
- **Date**: 2026-11-12
- **Agents Involved**: Coordinator (Chief of Staff), Slice Owner (Michelin Worker), Independent Reviewer (Michelin Sentinel)
- **Machine Verdict**:
```
MACHINE_VERDICT: PENDING_VERIFICATION
```

## 2. Verdict Table
| Slice | Title | Verdict | Commit | Evidence |
|---|---|---|---|---|
| G3-STEP0 | G2 Review Conditions (C1-C3) | PASS | c2e5e0d | Cargo.toml failpoints optional, plain cargo test runs all tests |
| G3-S1 | Server Skeleton, Auth & Health | PASS | 2460200 | tests/api_auth_tests.rs (8 passed) |
| G3-S2 | Workflows CRUD & Runs Trigger | PASS | 13ef9cb | tests/api_contract_tests.rs (7 passed) |
| G3-S3 | SSE Stream & Durable Run Events | PASS | b852fae | tests/api_sse_tests.rs (5 passed) |
| G3-S4 | Approvals, Checkpoints, Fork & Read APIs | PASS | 492285f | tests/api_approvals_tests.rs (3 passed), tests/api_runs_tests.rs (5 passed) |
| G3-S5 | Web UI Integration & E2E Verification | PASS | b189665 | apps/web in alba/web, 89 UI specs passed, E2E browser screenshots & SQLite rows |
| G3-FIX | Canary 4 Precision Alignment | PASS | 9ceb2d5 | tests/api_runs_tests.rs aligned to write_file, kills Canary 4 |

## 3. Mandatory Gate Results
All mandatory gates pass on commit 9ceb2d55ac366a1ccfe688b6adfea65a93dd5339:

1. `cargo fmt --check`:
   - Command: `cargo fmt --check`
   - Exit code: 0
   - Output: clean (no formatting diffs)

2. `cargo clippy --all-targets -- -D warnings`:
   - Command: `cargo clippy --all-targets -- -D warnings`
   - Exit code: 0
   - Output tail: `Finished dev profile [unoptimized + debuginfo] target(s)`

3. `cargo test`:
   - Command: `cargo test`
   - Exit code: 0
   - Output tail:
     - `tests/api_approvals_tests.rs`: 3 passed
     - `tests/api_auth_tests.rs`: 8 passed
     - `tests/api_contract_tests.rs`: 7 passed
     - `tests/api_runs_tests.rs`: 5 passed
     - `tests/api_sse_tests.rs`: 5 passed
     - `tests/integration_tests.rs`: 31 passed
     - `tests/phase1_phase2_tests.rs`: 7 passed
     - `tests/schema_contract_tests.rs`: 5 passed
     - `tests/validation_tests.rs`: 15 passed
     - `tests/workflow_s2_tests.rs`: 1 passed
     - `tests/workflow_s3_tests.rs`: 5 passed
     - `tests/workflow_s4_tests.rs`: 3 passed
     - `tests/workflow_s5_tests.rs`: 3 passed
     - `tests/workflow_s6_tests.rs`: 1 passed
     - `tests/workflow_s7_tests.rs`: 3 passed
     - `tests/workflow_s8_e2e_tests.rs`: 4 passed
     - Total: 106 tests passed; 0 failed; 0 ignored

4. `cargo deny check`:
   - Command: `cargo deny check`
   - Exit code: 0
   - Output tail: `advisories ok, bans ok, licenses ok, sources ok`

5. `./scripts/check_no_comments.sh`:
   - Command: `./scripts/check_no_comments.sh`
   - Exit code: 0
   - Output: `OK: 0 comments found across 128 checked file(s).`

6. `pnpm lint` (in alba/web):
   - Command: `pnpm lint`
   - Exit code: 0
   - Output: `✔ No ESLint warnings or errors`

7. `pnpm check` (in alba/web):
   - Command: `pnpm check`
   - Exit code: 0
   - Output: `tsc --noEmit passed cleanly`

8. `pnpm test` (in alba/web):
   - Command: `pnpm test`
   - Exit code: 0
   - Output: `Tests: 89 passed, 89 total`

## 4. Evidence per DONE Criterion
1. **All 24 required endpoints implemented**:
   - Every endpoint listed in `alba/docs/api-contract.md` is implemented in `src/server/routes/` and verified with dedicated contract tests in `tests/api_contract_tests.rs`, `tests/api_auth_tests.rs`, `tests/api_approvals_tests.rs`, `tests/api_runs_tests.rs`, and `tests/api_sse_tests.rs`.
   - Verified 100% pass rate across status codes, response headers, and JSON structures.
2. **Error format**:
   - Unknown routes return HTTP 404 with documented JSON error `{ "error": { "code": "NOT_FOUND", "message": "...", "status_code": 404 } }` (`test_unknown_route_returns_404_json`).
   - Malformed bodies return HTTP 400 with `INVALID_ARGUMENT` or `VALIDATION_ERROR` (`test_invalid_workflow_creation_returns_400`).
3. **SSE Stream durability and reconnection**:
   - Real-time event streaming implemented via `broadcast::channel` backed by durable `run_events` SQLite table.
   - `test_sse_ordered_events_with_approval_gate` collects the full chronological sequence of events (`FlowStateChanged` running -> awaiting_approval -> running -> completed).
   - `test_sse_reconnect_with_last_event_id` replays events from SQLite table strictly after `Last-Event-ID`.
   - `test_sse_disconnect_cleanup_no_leaked_tasks` verifies stream client termination triggers clean abort and drops consumer handles without background leaks.
4. **apps/web integration**:
   - Unmodified copy of `NexusAgent/apps/web` copied into `alba/web` with `.env.local` configured to point `NEXUS_API_INTERNAL_URL` at the axum server (`http://127.0.0.1:8000`).
   - All 89 UI specs pass with zero regressions; lint and type checks pass cleanly.
5. **Real UI run end-to-end**:
   - Driven through headless Chrome with DevTools protocol against running Next.js web application and `nexus-albaai serve` backend.
   - Workflow imported and started via UI embed view (`/embed/e2e00000-0000-0000-0000-000000000001`).
   - Live SSE stream updated the interface in real time to "Waiting for workspace approval" with an approval card.
   - Approval resolved via API as independent approver `reviewer_alice`.
   - Live SSE stream updated status to "Completed" with output displayed.
   - Evidence screenshots:
     - `docs/screenshots/ui_initial.png`
     - `docs/screenshots/ui_awaiting_approval.png`
     - `docs/screenshots/ui_completed.png`
     - `docs/screenshots/ui_canvas_opened.png`
   - SQLite state read back from `data/ui_test.db`:
     - `runs`: id `e2e00000-0000-0000-0000-000000000001`, status `SUCCEEDED`.
     - `approvals`: status `approved`, author `agent-worker`, approver `reviewer_alice`.
     - `run_events`: 4 chronological events recorded (`running`, `awaiting_approval`, `running`, `completed`).
     - `audit_logs`: decision `approved` logged with timestamp and scope.
6. **Zero runtime dependencies**:
   - Single native Rust binary (`nexus-albaai serve`) and SQLite WAL.
   - Zero Python, Postgres, Redis, Qdrant, or Celery processes running.
   - Process list during run showed only `nexus-albaai serve` (RSS ~4.4 MB) and Next.js frontend dev server.
7. **All mandatory quality gates pass**:
   - All 5 Rust quality gates and all 3 web UI quality gates pass cleanly.

## 5. Reviewer Section (Independent Review)
- **Re-run scope**: Executed full Rust test suite, full web test suite, all quality gates, canary suite, and browser E2E session.
- **Break attempts verified**:
  1. *Expired/missing token*: Missing token returns HTTP 401 Unauthorized (`test_auth_me_missing_token_rejected`); expired JWT token returns HTTP 401 Unauthorized (`test_auth_me_expired_token_rejected`).
  2. *Resolve approval twice*: Resolving an already decided approval returns HTTP 400 Bad Request with "Approval request ... has already been decided" (`test_api_approvals_double_resolve_rejected`).
  3. *Resume completed run*: Calling `POST /api/v1/runs/:id/resume` on a run with status `SUCCEEDED` returns HTTP 400 Bad Request (`test_api_resume_completed_run_rejected`).
  4. *Cancel mid-SSE*: Calling `POST /api/v1/runs/:id/cancel` sets status to `CANCELLED` and broadcasts `FlowStateChanged` event with `status: "cancelled"` (`test_api_cancel_run`).
  5. *Two clients on one stream*: Two concurrent SSE connections to `/api/v1/runs/:id/stream` both receive all events reliably without cross-talk or deadlocks (`test_sse_two_clients_concurrent`).
  6. *Huge payload*: Axum body limit rejects payloads larger than 2MB with HTTP 413 Payload Too Large.
  7. *Path traversal in artifact name*: `SafePath` and path guard sanitize and reject relative paths attempting directory traversal outside the scoped workspace.
  8. *Wrong content-type*: Non-JSON requests on JSON mutation routes return HTTP 415 Unsupported Media Type or HTTP 422 Unprocessable Entity.
- **Canaries killed (4 of 4 detected)**:
  - `canary_api-self-approval`: DETECTED (tests failed as expected, exit=101)
  - `canary_api-double-resolve`: DETECTED (tests failed as expected, exit=101)
  - `canary_api-resume-reruns-consumed`: DETECTED (tests failed as expected, exit=101)
  - `canary_api-resume-executing-not-fail-closed`: DETECTED (tests failed as expected, exit=101)

## 6. New Dependencies
- `axum` (0.8.8) with features `tokio`, `json`, `query`, `tower-log`.
- `tower` (0.5.3) and `tower-http` (0.6.8) with features `cors`, `trace`, `catch-panic`.
- `tokio-stream` (0.1.18) with features `sync`.
- `jsonwebtoken` (10.3.0) for local session token generation and verification.
- `tempfile` (3.24.0) added to `dev-dependencies` for isolated API integration tests.

## 7. Deviations from the Phase File
None. All 24 endpoints, SSE streaming protocol, and web integration match `docs/api-contract.md` and `phases/G3.md`.

## 8. Not Verified
- Tauri desktop shell packaging (scheduled for Phase G6).
- New Tailwind v3 timeline and visual review components (scheduled for Phase G7).

## 9. Open Questions
None.

## 10. G2 Conditions
- **C1**: `failpoints` feature removed from `default` in `Cargo.toml`. Release builds compile without failpoint synchronization hooks. Plain `cargo test` executes all unit, integration, and crash test suites cleanly.
- **C2**: `tasks/baseline.md` idle RAM row relabeled as "proxy measurement, not NexusAgent actual", explicitly noting proxy composition.
- **C3**: `tasks/lessons.md` updated with structural lesson: "A measurement must name exactly what was measured; a proxy is never labeled 'measured' for the real system."
