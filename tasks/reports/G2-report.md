# Phase G2 Report: Rust WorkflowSpec Executor

## 1. Header
- **Phase**: G2 (Rust WorkflowSpec Executor)
- **Branch**: feat/g2-engine
- **Final Commit Hash**: 8243add
- **Date**: 2026-11-12
- **Agents Involved**: Coordinator (Chief of Staff), Slice Owner (Michelin Worker), Independent Reviewer (Michelin Sentinel)
- **Machine Verdict**:
```
MACHINE_VERDICT: PENDING_FIRST_RUN
```

## 2. Verdict Table
| Slice | Title | Verdict | Commit | Evidence |
|---|---|---|---|---|
| G2-S1 | WorkflowSpec Validator | PASS | 7d14c5a86b0c | tests/validation_tests.rs (15 tests passed) |
| G2-S2 | Linear Crew Executor | PASS | 7d14c5a86b0c | tests/workflow_s2_tests.rs (1 test passed) |
| G2-S3 | Approval Gate & Separation of Duties | PASS | 0c6a4ac9d062 | tests/workflow_s3_tests.rs (5 tests passed) |
| G2-S4 | SQLite Tool Journal & Idempotency | PASS | 0c6a4ac9d062 | tests/workflow_s4_tests.rs (3 tests passed) |
| G2-S5 | Router & Parallel Branches | PASS | 7d14c5a86b0c | tests/workflow_s5_tests.rs (3 tests passed) |
| G2-S6 | Hierarchical Crew Orchestration | PASS | 7d14c5a86b0c | tests/workflow_s6_tests.rs (1 test passed) |
| G2-S7 | Trajectory Fork & Time-Travel | PASS | 0c6a4ac9d062 | tests/workflow_s7_tests.rs (3 tests passed) |
| G2-S8 | CLI Commands & 3-Agent E2E Scenario | PASS | 0c6a4ac9d062 | tests/workflow_s8_e2e_tests.rs (4 tests passed, 20/20 real SIGKILL) |
| G2-CR1 | Correction Round 1 (F1-F5 fixes) | PASS | 0c6a4ac9d062 | Real child.kill() SIGKILL crash tests, break tests, zero comments |

## 3. Mandatory Gate Results
All five mandatory gates pass on commit 0c6a4ac9d062d983d9b7e3478bb1b7ee9bc442f8:

1. `cargo fmt --check`:
   - Command: `cargo fmt --check`
   - Exit code: 0
   - Output: clean (no diffs)

2. `cargo clippy --all-targets -- -D warnings`:
   - Command: `cargo clippy --all-targets -- -D warnings`
   - Exit code: 0
   - Output tail: `Finished dev profile [unoptimized + debuginfo] target(s) in 11.09s`

3. `cargo test`:
   - Command: `cargo test`
   - Exit code: 0
   - Output tail:
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
     - Total: 78 tests passed; 0 failed; 0 ignored

4. `cargo deny check`:
   - Command: `cargo deny check`
   - Exit code: 0
   - Output tail: `advisories ok, bans ok, licenses ok, sources ok`

5. `./scripts/check_no_comments.sh`:
   - Command: `./scripts/check_no_comments.sh`
   - Exit code: 0
   - Output: `OK: 0 comments found across 107 checked file(s).`

## 4. Evidence per DONE Criterion
1. **Data-defined workflow execution**:
   - `WorkflowSpec` validated without hardcoding logic: `test_valid_minimal_workflow`, `test_linear_crew_two_agents_end_to_end`.
   - Node kinds validated: Start, Crew (sequential and hierarchical), ApprovalGate, Router (bounded loops), ParallelBranch (all/any joins), End.
   - Unsupported kinds explicitly rejected: `test_unsupported_rag_node_explicit_rejection` and `test_unsupported_sandbox_node_explicit_rejection`.
2. **State Machine**:
   - Exhaustive enum matching without wildcard `_` fallbacks for `WorkflowRunState` and `WorkflowNodeState`.
3. **Approval protocol with separation of duties**:
   - `test_approval_gate_pause_and_allow_once_resume`: pauses at `AwaitingApproval`, rejects author approval (`Separation of duties violation`), accepts different approver, resolves `AllowedOnce`, executes, and transitions to `Consumed`.
   - Double-approval rejection: `test_approval_gate_double_approve_rejected` verifies a second approval attempt returns `already decided` error and writes `approval.already_decided_rejected` to `audit_logs`.
   - Deny after decision rejection: `test_approval_gate_already_decided_deny_rejected`.
   - Double-consume rejection: `test_approval_gate_double_consume_rejected` verifies `consume_approval` and `claim_workflow_approval` return false on duplicate execution attempts.
4. **Tool journal & crash safety**:
   - `test_tool_journal_idempotency_avoids_reexecution`: pre-seeded `consumed` tool is served from journal without re-execution (asserts execution counter remains 0).
   - Real SIGKILL crash while executing: `test_sigkill_crash_while_executing_fails_closed_20_iterations` sends real `child.kill().await` when engine reaches `tool_after_executing_before_complete` failpoint; resume fails closed with `was interrupted mid-execution (crashed), failing closed` and tool is not re-executed (0 side effects on disk across 20 iterations).
   - Real SIGKILL crash after consumed: `test_sigkill_crash_after_consumed_resumes_without_rerunning_20_iterations` sends real `child.kill().await` at `tool_after_consumed_before_checkpoint`; resume succeeds using cached output without re-executing tool (file content verified unmodified across 20 iterations).
5. **Workflow Fork & Time-Travel**:
   - `test_workflow_fork_preserves_parent_immutability_and_diverges`: fork at sequence 3 preserves parent checkpoints and creates new trajectory branch.
   - Break attempts: `test_fork_non_existent_sequence_fails` and `test_fork_non_existent_run_fails` return `CheckpointError::NotFound`.
6. **E2E CLI Scenario**:
   - `test_three_agent_workflow_e2e_cli_and_sqlite_verifications`: automated scenario with `run`, `status`, self-approval rejection, `approve`, `resume`, `fork`, `deny`, and SQLite state assertions (10 parent checkpoints, 6 forked checkpoints).
   - `test_resume_completed_workflow_run_returns_completed_without_rerun`: resuming a Completed run immediately returns Completed without re-running model requests.

## 5. Reviewer Section (Independent Review)
- **Re-run scope**: Executed all five quality gates, full unit and integration test suite, and the independent verifier.
- **Claims verified**:
  1. *Real SIGKILL*: Verified that tests spawn `nexus-albaai`, wait for failpoint on stderr, and issue `.kill()` via tokio Process `child.kill().await`. Flakiness run executed 20 iterations each for both crash scenarios with 100% pass rate.
  2. *Canary Detection*: Verified that Canary 2 (`consumed-tool-rerun`) and Canary 5 (`double-decision-allowed`) fail when mutations are injected, proving tests actively protect required invariants.
  3. *Zero Comments*: Verified `./scripts/check_no_comments.sh` inspects 107 files and passes with 0 comments.
  4. *Break Attempts*:
     - Double-approval: fails with InvalidState, logs `approval.already_decided_rejected`.
     - Deny on decided approval: fails with InvalidState.
     - Double-consume: returns false.
     - Fork non-existent sequence or run: returns `CheckpointError::NotFound`.
     - Spec with cycle and no loop_back: rejected by validator (`test_cycle_detected_without_loop_back`).
     - Concurrent resume processes: atomic single-winner claim verified by `test_concurrent_process_resume_executes_tool_once`.

## 6. New Dependencies
None. No external dependencies were added in G2.

## 7. Deviations from the Phase File
- Added cargo feature `failpoints` (default enabled in development) for deterministic SIGKILL test synchronization (`ALBA_FAILPOINT` environment variable). Recorded in `tasks/decisions.tsv`.

## 8. Not Verified
- Standalone Qdrant or Celery runtimes: Not applicable (G0 decision replaces them with embedded SQLite and in-process tokio queues).
- HTTP daemon / axum endpoints: Scheduled for Phase G3.

## 9. Open Questions
None. All review findings F1-F5 have been addressed and validated.

## 10. Correction Round 1 Summary
- **F1**: Implemented deterministic failpoints and real process `child.kill().await` crash tests running 20 iterations each for (a) `tool_after_executing_before_complete` and (b) `tool_after_consumed_before_checkpoint`. Renamed clean close simulation tests.
- **F2**: Added break tests for double-approval rejection with audit logging, deny on already decided approvals, double-consume rejection, resuming completed workflows, and non-existent fork targets. Cited `test_cycle_detected_without_loop_back` and `test_concurrent_process_resume_executes_tool_once`.
- **F3**: Delivered formal report document `tasks/reports/G2-report.md` following COMMON.md template.
- **F4**: Explicitly documented that pause/resume in CLI e2e test represents clean process exit between CLI invocations, while crash resilience is verified by dedicated SIGKILL test suites.
- **F5**: Updated idempotency tests to pre-seed consumed state and verify zero re-executions, reliably detecting and killing Canary 2 (`consumed-tool-rerun`).
