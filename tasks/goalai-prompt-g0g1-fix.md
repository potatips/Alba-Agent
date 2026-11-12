/goalai

# GOAL
Correction round for G0 and G1. An independent review found that parts of the G0/G1 report are unsupported or inconsistent.
Fix ONLY the items below. Do NOT start G2. When finished, stop and wait for human review.

# CONTEXT
- Working directory: /Users/nguyenvokhang/Downloads/v3/alba (branch feat/g1-contract)
- Read-only references: /Users/nguyenvokhang/Downloads/v3/NexusAgent, /Users/nguyenvokhang/Downloads/v3/albaAI
- Files: tasks/baseline.md, tasks/todo.md, tasks/decisions.tsv, tasks/lessons.md, docs/api-contract.md (and its copy tasks/api-contract.md)
- Roles as before: Owner fixes, a fresh independent Reviewer verifies. The Owner never approves its own work.
  Every agent first RESTATES the job and waits for Coordinator confirmation.

# REVIEW FINDINGS TO FIX

## F1. Baseline numbers without evidence (G0)
Problems found:
- Idle RAM "850-1,150 MB" has per-process figures (API ~180MB, Celery ~320MB, Postgres ~55MB, Redis ~20MB, Qdrant ~105MB,
  Electron ~350MB) with no measurement source. apps/desktop/README.md says the default desktop (lean) mode does NOT run
  standalone Postgres/Redis/Qdrant (SQLite, in-process queue, embedded Qdrant), so "6 processes" may be wrong for the default mode.
- Startup: the report says "9.8s warm", but ~/Library/Application Support/@nexus/desktop/logs/startup.log shows the
  2026-11-09 13:24 start taking about 19s (13:24:32 -> 13:24:51 "Ready"). The "60s cold" figure was not verified.
- Install size 1.15 GB / 1.50 GB comes from the README ("~1.15 GB", and "reduces by ~355 MB"), not from a measurement.
Required:
- Measure idle RAM of the REAL default desktop mode: start the stack, wait until idle, record per-process RSS with ps (or
  footprint) and the total, plus the exact commands and raw output. If the Postgres mode is also reported, label it separately.
- For startup, compute every start in startup.log (time from first "Starting" line to "Ready") and report the list with min/median/max.
  For the cold start, cite the exact log lines or mark it "not verified".
- For install size, measure with du on an actual built runtime or app bundle if one exists; otherwise label it
  "documented (README), not measured".
- Keep the 27.22s sample run (verified in worker.log) and the pnpm/pytest logs.
- Update tasks/baseline.md: every number gets (value, method, command, raw output or log line, measured | documented | not verified).

## F2. API contract inconsistent with the report (G1)
Problems found:
- The report claimed "21 minimal v1 endpoints" and named /blueprints, /audit-logs, /health, /runs/{id}/resume, /capsules/search,
  /checkpoints/{id}; none of those exist in docs/api-contract.md. The doc uses /workflows/:id/runs, /approvals/:id/decide,
  /learning-transfer/capsules and marks ~36 rows "Yes", while only 12 endpoints have a detailed spec.
- "63 endpoints audited" vs 43 rows in the doc.
- There is no resume endpoint in the contract, but G2's "done" requires resume.
Required:
- Re-audit apps/web/src (lib/api.ts, lib/sse.ts, components, hooks) and produce ONE authoritative table: method, path,
  caller file:line, request/response shape taken from a real NexusAgent response or schema (do not write adapters from memory),
  and v1 status (required | deferred | out of scope) with a reason.
- Choose a genuinely minimal v1 set that lets apps/web complete one run end-to-end, including pause/approve/resume/fork.
  Add the resume endpoint (or justify the existing mechanism).
- Give every v1 endpoint a detailed spec (not only 12). Make the report's endpoint count and names match the doc exactly.
- Keep one source of truth: docs/api-contract.md; tasks/api-contract.md must be a pointer or an identical copy checked by a script.

## F3. Clippy does not cover tests
- `cargo clippy --all-targets -- -D warnings` fails at tests/schema_contract_tests.rs:243 (assert_eq! with a bool literal).
Required: fix it, and make `cargo clippy --all-targets -- -D warnings` the mandatory gate everywhere (todo.md, scripts, prompts).

## F4. Unexplained "Model Policy: Locked to Gemini 3.8 Flash (High)"
- This line appeared in the report but not in any instruction. State where it comes from (tool config or self-set) and
  whether it affects the work. If it was self-set, remove it from reports.

## F5. Commit hygiene
- One commit mixes the copied albaAI core with new G1 code. Split into: (1) "chore: import albaAI core unchanged"
  (2) "feat(schema): G1 contract, types, SQLite schema" and (3) a fix commit for this round. Do not rewrite shared history outside
  alba/; if the repository is still local-only, rebuild the branch cleanly and keep the old tip under a backup branch name.

# HARD RULES
- Same rules as the main prompt: no editing NexusAgent/albaAI/albaAIui; no .env or key reads; no merge/push/deploy; do not delete
  anything outside alba/; "build passed" is not evidence of done; mark anything unverified as "not verified".
- Generated files from running tests in NexusAgent (.pytest_cache, __pycache__) must be removed or avoided by setting
  PYTHONDONTWRITEBYTECODE=1 and `pytest -p no:cacheprovider`.
- Every number or claim in a report must point to a command and its raw output, a log line, or a file:line.
- Mandatory gates before reporting: cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test; cargo deny check;
  ./scripts/check_no_comments.sh.

# DONE CONDITIONS (all must hold)
1. tasks/baseline.md: all 4 metrics have method + raw evidence + status label; RAM measured for the real default mode.
2. docs/api-contract.md: one authoritative table; every v1 endpoint has a detailed spec; includes resume; counts match the report.
3. `cargo clippy --all-targets -- -D warnings` passes and is the documented gate.
4. Model-policy line explained or removed.
5. Commit history split as described.
6. A fresh Reviewer re-runs every gate and re-checks 3 randomly chosen numbers against their raw evidence, then reports
   PASS | ISSUES | BLOCKED per finding F1-F5.

# START
Fix F1-F5, submit the Reviewer's per-finding report, update tasks/decisions.tsv and tasks/todo.md (Review section), then STOP.
Do not begin G2 until a human approves.
