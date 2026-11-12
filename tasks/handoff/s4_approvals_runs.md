# S4 Handoff: Approvals, Runs Control, Checkpoints, Fork, Rewind, and Settings

## Deliverables
1. `src/server/routes/approvals.rs`:
   - `GET /api/v1/approvals`: List pending approvals.
   - `POST /api/v1/approvals/:id/resolve`: Resolve approval (`allow_once`, `allow_always`, `deny`) with strict separation of duties (author != approver) and atomic claim; returns 403 Forbidden on author == approver and 409 Conflict on duplicate decision.
2. `src/server/routes/runs.rs`:
   - `POST /api/v1/runs/:id/cancel`: Cancellation request emitting cancellation event and updating status.
   - `POST /api/v1/runs/:id/resume`: Idempotent resume interpreting workflow checkpoints; returns 400 Bad Request if already completed.
   - `GET /api/v1/runs/:id/checkpoints`: List sequence checkpoints.
   - `POST /api/v1/runs/:id/checkpoints/:id/fork`: Fork run at checkpoint into child branches.
   - `POST /api/v1/runs/:id/checkpoints/:id/rewind`: Rewind run to checkpoint.
   - `GET /api/v1/runs/:id/artifacts`: List run artifacts.
3. `src/server/routes/capsules.rs`:
   - `GET /api/v1/learning-transfer/capsules`: Returns knowledge capsules from store.
4. `src/server/routes/settings.rs`:
   - `GET /api/v1/settings/chat-models`: Returns model catalog.
   - `GET /api/v1/settings/execution`: Returns execution policy.
   - `PUT /api/v1/settings/execution`: Updates execution policy.
5. `src/server/routes/tools.rs`:
   - `GET /api/v1/tools/builtin`: Returns built-in workspace tools.
   - `GET /api/v1/skills`: Returns skills.
6. `tests/api_approvals_tests.rs`:
   - 3 passing integration tests verifying author != approver (kills Canary 1) and double-resolve rejection (kills Canary 2).
7. `tests/api_runs_tests.rs`:
   - 5 passing integration tests verifying idempotent resume without re-executing consumed tool (kills Canary 3), failing closed when tool is executing (kills Canary 4), completed run rejection, cancel, and checkpoints fork/rewind/artifacts.
8. `tests/api_contract_tests.rs`:
   - 7 passing integration tests covering endpoints end-to-end.

## Gates Status
- `cargo fmt --check`: PASS
- `cargo clippy --all-targets -- -D warnings`: PASS
- `./scripts/check_no_comments.sh`: PASS (0 comments across 127 files)
- `cargo deny check`: PASS
- All integration tests: PASS (87 tests passed across all test binaries)
