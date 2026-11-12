/goalai

# GOAL
Build a lightweight "Alba Agent": a platform that runs a team of agents. Backend is a single Rust binary,
the UI is the existing NexusAgent apps/web, packaged with Tauri. Priorities, in order:
(1) small install size, (2) low idle RAM and fast startup, (3) keep NexusAgent's strengths.

# CONTEXT (read before starting)
- /Users/nguyenvokhang/Downloads/v3/NexusAgent  = REFERENCE, READ-ONLY. Do not edit or commit here.
- /Users/nguyenvokhang/Downloads/v3/albaAI      = existing Rust core (agent loop, SQLite checkpoints, atomic approvals,
  sandbox, path_guard, flow DAG, events). Basis for the new backend. READ-ONLY; copy into the working directory.
- /Users/nguyenvokhang/Downloads/v3/albaAIui    = build output only. Ignore.
- WORKING DIRECTORY: /Users/nguyenvokhang/Downloads/v3/alba (create it, git init, work on a dedicated branch).
- Read /Users/nguyenvokhang/Downloads/v3/tasks/todo.md and tasks/lessons.md first.

# DECISIONS ALREADY MADE (do not reopen)
- Backend: Rust only (axum + tokio). NO Go in v1.
- UI: keep apps/web (Next.js 15, Tailwind v3). No framework change, no Tailwind v4 upgrade.
- Storage: SQLite WAL replaces Postgres + Redis. Embedded vector store (sqlite-vec or usearch) replaces Qdrant.
  Queue and events run in-process (tokio); SSE via broadcast. No Celery, no Python runtime.
- Replace CrewAI with a Rust flow executor that runs WorkflowSpec. Replace LiteLLM with OpenAI-compatible + Anthropic adapters.
- Desktop: Tauri, not Electron.
- Workflows, agents and blueprints are DATA (JSON/YAML following AgentSpec/WorkflowSpec), not hard-coded in Rust.

# MUST KEEP (NexusAgent strengths)
1. AgentSpec / TaskSpec / WorkflowSpec schema (generate serde types from core-schema).
2. Artifact contract + claims hook + verification that decides "done" by machine.
3. Approval / checkpoint / time-travel / fork / audit; one-time approvals, author != approver, fail-closed on crash.
4. KnowledgeCapsule + scope firewall + negative-transfer detection.
5. Studio blueprints (data), SKILL.md loader (progressive loading), Deny/Ask/Allow execution policy + seatbelt sandbox.
6. apps/web UI and the REST/SSE /api/v1 contract the UI actually calls.
Port albaAI/ui's Fork / Checkpoint timeline / separated-approver Approval into apps/web (rewritten for Tailwind v3).

# AGENT TEAM AND ROLES
- Coordinator: reads todo.md, splits work into vertical slices, issues briefs (goal, acceptance criteria, required
  evidence, stop condition). Does not write large code itself. Owns the final verdict.
- Owner (one per slice): writes code and tests, submits evidence. Works in its own worktree/branch. NEVER approves its own work.
- Independent Reviewer (a fresh agent that has not seen the Owner's reasoning): reopens the commit, reruns commands,
  reproduces bugs, walks the real user journey. A verdict is valid only for the exact commit checked; any change means re-review.
- Every agent starts by RESTATING the job (goal, expected output, exclusions, open questions) and waits for Coordinator confirmation.

# COMMUNICATION AND HANDOFF
- Communicate through FILES, not free-form chat:
  - tasks/todo.md (plan, tick when done)
  - tasks/decisions.tsv (time, phase, decision, reason, evidence, result)
  - tasks/lessons.md (after every correction: record a rule)
  - tasks/handoff/<slice>.md (brief and resume note when stopping at an atomic boundary)
- Every worker reports PASS | ISSUES | BLOCKED with evidence (commands run, results, values read back from DB/files).
- Any review comment that repeats a second time must become a lint/test/CI check, not just a note.

# PHASES AND MEASURABLE "DONE"
G0 Baseline: on NexusAgent (read/run only): pnpm lint && pnpm check && pnpm test, and pytest. Measure install size,
   idle RAM, startup time, and time of one sample run. Record in tasks/baseline.md. DONE when all 4 numbers and command logs exist.
G1 Contract: list the /api/v1 endpoints apps/web actually uses (lib/api.ts, lib/sse.ts); choose the minimal v1 set;
   generate Rust types from core-schema; fix the SQLite schema. DONE when docs/api-contract.md exists and type-generation tests pass.
G2 Rust engine: move the albaAI core, add a WorkflowSpec executor (Crew sequential/hierarchical, ApprovalGate, Router,
   loop_back, ParallelBranch). DONE when: cargo test passes; a 3-agent sample workflow runs, pauses at ApprovalGate, and
   resume and fork give correct results; kill -9 mid-run then resume does not re-run a tool already consumed.
G3 Compatible API: axum serves /api/v1 + SSE; apps/web completes one run end-to-end. DONE when: API contract tests are green,
   apps/web tests are green, and the Reviewer runs one real run in the UI (screenshot + stored values read back).
G4 Verification + hooks: port validator, claims, evidence and Deny/Ask/Allow policy. DONE when: an agent claiming a file it did
   not write makes the run FAIL (tested); an empty or wrong-signature file makes it FAIL (tested).
G5 Memory: checkpoints + capsules (scope firewall) + embedded vectors + conversation compaction with summaries. DONE when:
   tests show the scope firewall redacts secrets/PII, capsules are created only under the stated conditions, and compaction
   keeps tool_call pairs intact.
G6 Tauri packaging: one binary + static web build. DONE when: the 4 G0 numbers are re-measured and better than baseline;
   no Python/Postgres/Redis/Qdrant processes remain.
G7 UI: port Fork / Checkpoint timeline / Approval (separated author and approver) into apps/web. DONE when: tests pass in light
   and dark, and the Reviewer completes the fork and approve journey in the real UI.
Out of scope for v1: architect/team_generator, team_evolution, consensus, full RAG, MCP hub, mobile.

# HARD RULES
- "Build passed" is NOT proof of done. Run the real behavior and read back the stored data.
- Reproduce a bug before fixing it; after the fix, rerun that exact reproduction.
- Never read, print, or commit .env files or API keys. Keys come only from environment variables or stdin.
- Do not modify NexusAgent, albaAI, or albaAIui. Do not delete files outside alba/.
- Do not merge to the main branch, push, or deploy. Stop at "ready for a merge decision" and wait for a human.
- Each slice is a small, revertible change. Minimal code, find root causes, no temporary patches.
- No TODO/stub/pass in orchestration code. No superfluous comments in Rust (use scripts/check_no_comments.sh if carried over).
- Sandbox: network off by default, writes only inside the workspace; run_command and write_file always need approval.
- Mandatory checks before reporting a slice done: cargo fmt --check, cargo clippy -- -D warnings, cargo test,
  cargo deny check, and pnpm lint && pnpm check && pnpm test (in apps/web).
- Anything that cannot be verified must be labeled "not verified". Do not guess numbers.

# BUDGET AND STOP CONDITIONS
- Stop and ask the human when: an architecture decision is needed outside the decided list; a slice fails after 3 fix rounds;
  anything would delete or overwrite data outside alba/; a security risk appears.
- Every long run needs a checkable stop condition (the phase criteria), never "run for N hours".
- For any recurring routine: run it once manually first, use the slowest useful schedule, set a usage cap.
- At the end of each phase: update tasks/todo.md (Review section: what was verified, what was not), then stop and wait
  for human approval before the next phase.

# START
In the first run do ONLY G0 and G1. Submit a PASS/ISSUES/BLOCKED report for both, then stop for human review.
