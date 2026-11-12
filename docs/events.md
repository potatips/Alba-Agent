# Alba Agent Runtime Event Specification

All runtime events emitted by `nexus-albaai` and forwarded by the Tauri desktop host (`alba-agent-desktop` on channel `alba://runtime-event` and topic-named channels) conform strictly to `tests/fixtures/runtime_event.schema.json`.

## 1. JSON Envelope

Every event is emitted as a single JSON object (JSON Lines for CLI streams; serialized Tauri event payloads for the desktop UI):

```json
{
  "schema_version": "1.0",
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "tenant_id": "00000000-0000-0000-0000-000000000001",
  "workspace_id": "00000000-0000-0000-0000-000000000002",
  "run_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
  "event_type": "TOOL_INVOKED",
  "node_id": null,
  "timestamp": "2026-11-11T15:00:00Z",
  "payload": {}
}
```

| Field | Type | Constraints | Description |
|---|---|---|---|
| `schema_version` | `string` | const `"1.0"` | Event contract version |
| `event_id` | `string` | UUID | Unique event identifier |
| `tenant_id` | `string` | UUID | Tenant boundary identifier |
| `workspace_id` | `string` | UUID | Workspace boundary identifier |
| `run_id` | `string` | UUID | Execution run identifier |
| `event_type` | `string` | enum (10 values) | Lifecycle / tool / sandbox / fork event type |
| `node_id` | `string \| null` | optional | Originating DAG flow node identifier when inside a `FlowExecutor` run |
| `timestamp` | `string` | RFC 3339 date-time | UTC emission timestamp |
| `payload` | `object` | JSON object | Event-specific metadata (never contains API keys or `Authorization` headers) |

---

## 2. Event Types (`RuntimeEventType`)

1. **`TOOL_INVOKED`**: Emitted right before executing a workspace tool (`tool_name`, `tool_call_id`).
2. **`TOOL_COMPLETED`**: Emitted immediately after tool execution finishes (`tool_name`, `tool_call_id`, `is_error`).
3. **`APPROVAL_REQUIRED`**: Emitted when `ApprovalPolicy`, a workflow `ApprovalGate`, or a `write_file`/`run_command` policy check pauses execution for human review (`approval_id`, `tool_name`, `action_description`, `intent_kind`, `author_id`, and optional `path`/`program`/`args`).
4. **`APPROVAL_RESOLVED`**: Emitted when an approved or denied run resumes (`approval_id`, `decision`).
5. **`FLOW_STATE_CHANGED`**: Emitted on every atomic `WorkflowStepCheckpoint` transition in `FlowExecutor` (`node_id`, `sequence`, `status`, `next_node_id`).
6. **`SANDBOX_TERMINAL_OUTPUT`**: Streamed chunk from `RunCommandTool` or `CommandTask` running inside the OS-native sandbox (`program`, `stream`: `stdout` \| `stderr`, `chunk`).
7. **`SANDBOX_FILE_DIFF`**: Emitted by `WriteFileTool` after computing and committing an atomic file edit (`path`, `original`, `modified`, `diff` unified patch).
8. **`TRAJECTORY_FORKED`**: Emitted when `fork_trajectory` or `rewind_and_fork_flow` creates an isolated branch from an earlier checkpoint sequence (`parent_run_id`, `forked_run_id`, `fork_sequence`, plus `branch_id`/`branch_label` for agent trajectories or `node_id` for workflows).
9. **`RUN_COMPLETED`**: Emitted when an agent loop or DAG flow finishes successfully (`result` or `node_id` + `output`).
10. **`RUN_FAILED`**: Emitted on terminal execution failure, turn/step limit exhaustion, operator denial, or cancellation (`error`).

---

## 3. Desktop IPC Event Forwarding

In `src-tauri/src/commands.rs`, `build_tauri_event_callback` emits every `RuntimeEvent` on both:
- `alba://runtime-event` (consumed by the React 19 UI event timeline, Monaco diff pane, Xterm terminal, and Approval Card components)
- `<EVENT_TYPE>` (the exact uppercase event name such as `APPROVAL_REQUIRED` or `SANDBOX_FILE_DIFF`)
