/goalai

# GOAL
Phase G2: build the Rust WorkflowSpec executor inside /Users/nguyenvokhang/Downloads/v3/alba so a data-defined workflow
(JSON/YAML) runs, pauses at approval, survives a hard crash, resumes without re-running consumed tools, and can be forked.
No HTTP server yet (that is G3). Deliver an engine library plus a CLI. When done, STOP and wait for human review.

# STEP 0 (small fix before G2, separate commit)
In tasks/baseline.md, relabel the Idle RAM "Default Lean Desktop" row from "measured" to "proxy measurement, not NexusAgent actual".
State exactly what was measured (next dev server, an empty Electron test app, a bare uvicorn) and that the 403 MB upper bound includes an
unmeasured 180 MB API figure. Keep the "documented, not measured" label on the 6-process figure.
Add to tasks/lessons.md: "A measurement must name exactly what was measured; a proxy is never labeled 'measured' for the real system."

# CONTEXT
- Working directory: /Users/nguyenvokhang/Downloads/v3/alba (branch feat/g1-contract; create feat/g2-engine from it)
- Inputs already in repo: src/schema/* (AgentSpec, TaskSpec, WorkflowSpec, WorkflowNode, ApprovalRequestSpec), docs/api-contract.md,
  SQLite DDL in the contract doc, the albaAI core (src/agent, src/checkpoints, src/governance, src/tools, src/sandbox, src/workflow/flow.rs, src/events).
- Read-only references: /Users/nguyenvokhang/Downloads/v3/NexusAgent (see packages/agent-runtime/flows/compiler.py and
  packages/agent-runtime/checkpoints/ for the semantics to match), /Users/nguyenvokhang/Downloads/v3/albaAI.
- Roles: Coordinator, Owner per slice, fresh independent Reviewer. The Owner never approves its own work.
  Every agent first RESTATES the job and waits for Coordinator confirmation.

# DESIGN CONSTRAINTS
- Workflows, agents and blueprints are DATA. The executor interprets WorkflowSpec; no workflow logic is hard-coded in Rust.
- Node kinds to support: Start, Crew (sequential and hierarchical with manager_agent_id), ApprovalGate, Router (with max_loop_iterations),
  Route, ParallelBranch (join any|all), loop_back edges, End. Sandbox (coder+reviewer) only if it falls out of existing albaAI tools; otherwise
  reject it at validation with a clear error. Unsupported kinds (RAG etc.) MUST fail validation explicitly, never be silently skipped.
- State machine: model run and node states as Rust enums with exhaustive match (no catch-all `_` arms on state enums).
  States include at least: Pending, Running, AwaitingApproval, Paused, Completed, Failed, Cancelled.
- Persistence: a checkpoint is written to SQLite (WAL) after every node transition, in a transaction. Approval keeps the albaAI
  protocol (allowed_once -> executing -> consumed, BEGIN IMMEDIATE, author != approver, fail-closed on crash while `executing`).
- Idempotency: every tool call gets a journal entry (run_id, node_id, tool_call_id, status). On resume, a call already `consumed` or `executing`
  is never executed again.
- Model access: use the existing ModelClient trait. Tests use a deterministic mock ModelClient (scripted responses, including tool calls).
  Any live-LLM test must be opt-in via an environment variable and excluded from the default `cargo test`.
- Events: emit through the existing EventSink; keep tests/fixtures/runtime_event.schema.json valid, extend it with node-level events if needed.
- Cancellation and timeouts via tokio cancellation; a cancelled run ends in Cancelled with a checkpoint.
- No new heavy dependencies. Justify every new crate in decisions.tsv (size impact, license via cargo deny).

# SLICES (vertical, each a small revertible commit)
S1 Validation: WorkflowSpec validator (acyclic except declared loop_back, reachable End, router targets exist, agent ids resolve,
   unsupported kinds rejected). DONE: unit tests for each rule, plus a table of error messages.
S2 Linear Crew run: Start -> Crew(sequential, 2 agents) -> End with mock model, checkpoint per node, events emitted.
   DONE: SQLite rows read back show the expected node states and ordered events.
S3 Approval gate: ApprovalGate pauses the run (AwaitingApproval); decision via CLI `approve`/`deny`; author != approver enforced.
   DONE: tests for allow-once, deny, self-approval rejected, double-consume rejected.
S4 Resume and crash safety: `resume` continues from the last checkpoint. DONE: an integration test spawns the engine binary, kills it with
   SIGKILL (a) while a tool call is `executing` and (b) after a tool is `consumed` and before the next checkpoint; resume then
   (a) fails closed for the `executing` call, (b) does not re-run the consumed tool. Read back the tool journal from SQLite to prove it.
S5 Router, loop_back, ParallelBranch: DONE: a test where a Router loops back twice then exits (max_loop_iterations enforced, exceeding it fails
   the run), and a parallel test with join=all and join=any, deterministic via the mock model.
S6 Hierarchical Crew: manager agent delegates to specialists via the spec (no hard-coded roles). DONE: test with a 3-agent hierarchical crew.
S7 Fork and time-travel: `fork <run> <checkpoint>` creates a new run from an earlier checkpoint without mutating the original;
   `rewind` as defined in docs/api-contract.md. DONE: original run rows unchanged (hash compare), forked run diverges correctly.
S8 CLI and sample: `alba run|status|approve|deny|resume|cancel|fork` using a data-only sample workflow at
   tests/fixtures/workflows/three_agent_approval.json (3 agents, one ApprovalGate). DONE: the end-to-end scenario below passes in an integration test.

# END-TO-END DONE SCENARIO (must be automated in cargo test)
1. Run the 3-agent sample; it pauses at ApprovalGate (state AwaitingApproval, checkpoint persisted).
2. Kill the process with SIGKILL. Restart. `status` shows the run still paused and no tool was re-executed.
3. A different actor approves (allow once). `resume` completes the run. Tool journal shows every call exactly once.
4. Fork from the checkpoint before the gate; deny on the fork; original run stays Completed, fork ends Failed/Denied.
5. Read back from SQLite: runs, checkpoints, approvals, audit rows, tool journal; assert exact values (not just exit codes).

# HARD RULES
- No editing NexusAgent, albaAI, albaAIui. No .env or key reads. No merge/push/deploy. No deleting outside alba/.
- Zero-comments policy stays (scripts/check_no_comments.sh). No TODO/stub/pass in orchestration code.
- "Build passed" is not evidence. Each slice reports PASS | ISSUES | BLOCKED with: commands run, raw output, and values read back from SQLite.
- Reproduce a bug before fixing it; rerun the same reproduction after the fix.
- Anything unverified must be labeled "not verified". Every number or claim cites a command and output, or file:line.
- Mandatory gates for every slice: cargo fmt --check; cargo clippy --all-targets -- -D warnings; cargo test; cargo deny check;
  ./scripts/check_no_comments.sh.
- Keep the 31+7+5 existing tests green throughout; adding tests is expected, deleting or weakening them is not allowed.
- A slice failing after 3 fix rounds: stop and ask the human. A needed decision outside this prompt: stop and ask.

# REVIEW PROTOCOL
- A fresh Reviewer (no access to the Owner's reasoning) re-runs all gates on the exact commit, reads the SQLite evidence itself,
  and re-runs the SIGKILL test at least 20 times to check for flakiness. Report the pass rate.
- The Reviewer also tries to break it: double-approve, approve from the author, resume a completed run, resume twice concurrently,
  fork from a missing checkpoint, a spec with a cycle without loop_back. Each attempt is recorded with result.
- Verdict is valid only for the reviewed commit hash.

# FINAL REPORT (and then STOP)
- Per slice S1-S8: PASS | ISSUES | BLOCKED, commit hash, evidence pointer.
- End-to-end scenario result and the 20-run flakiness rate.
- List of new dependencies with size and license.
- "Not verified" list. Update tasks/todo.md Review section and tasks/decisions.tsv.
- Do NOT begin G3 until a human approves.
