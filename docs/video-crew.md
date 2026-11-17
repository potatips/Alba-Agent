# Video Crew: Multi-Camera Assembly and Automated Quality Control

## Overview
Video Crew is an autonomous multi-camera video production workflow orchestrated by the Alba Agent engine. It takes multiple camera angles and a master audio reference, aligns audio streams via cross-correlation, compiles an Edit Decision List (EDL), pauses for mandatory human approval (with separated author and approver roles), and renders the final video with automated quality control checks.

---

## Architecture and Tools

### 1. Tools (`alba/src/tools/video/`)
- `ffprobe_info`: Inspects media streams, extracting duration, FPS, codecs, resolution, and audio channels. All paths are validated against directory traversal and symlink escapes via `path_guard`.
- `ffmpeg_sync_audio`: Computes millisecond time offsets between camera audio streams and a master audio reference using normalized cross-correlation. Returns `{camera, offset_ms, confidence}`. Rejects low-confidence matches (`confidence < min_confidence`) with a typed error without guessing.
- `ffmpeg_render`: Validates and renders an Edit Decision List (EDL) into a unified MP4 video using ffmpeg filter graphs. Enforces:
  - Non-overlapping cuts on the timeline
  - Source ranges within clip duration
  - Existing camera sources within workspace boundaries
  - Execution timeout (`max_render_seconds`)
  - Output file size limits (`max_output_bytes`)
  - Concurrency limits via `tokio::sync::Semaphore`
- `ffmpeg_qc`: Runs automated technical quality control on the rendered video:
  - Verifies duration matches expected EDL timeline within configured tolerance
  - Detects prolonged black frames via ffmpeg `blackdetect`
  - Asserts audio-video synchronization and stream presence

### 2. Agent Crew
- **Director**: Analyzes scene notes and camera coverage using `ffprobe_info` to design cut transitions and pacing.
- **Editor**: Measures audio sync offsets via `ffmpeg_sync_audio` and generates the Edit Decision List (EDL).
- **Approval Gate**: Human review gate requiring an explicit approval decision. Strictly enforces separation of duties (the author who initiated the workflow cannot approve their own EDL).
- **Render Engineer**: Executes `ffmpeg_render` on the approved EDL to produce the final master MP4.
- **QC Specialist**: Runs `ffmpeg_qc` and `ffprobe_info` to verify broadcast specifications.

### 3. Crash Safety & Idempotency
All tool calls are journaled in the SQLite checkpoint store (`checkpoints.db`). In the event of a system crash or restart, completed tool invocations (`consumed`) return cached outputs without re-executing expensive ffmpeg render jobs.

---

## Manual Execution with Real Footage

When real multi-camera footage is available, place the media files in `data/footage/`.

### Directory Layout
```
data/footage/
├── cam1.mp4
├── cam2.mp4
├── cam3.mp4
└── master.wav
```

### Step 1: Start Video Production Run
```bash
cargo run --bin alba -- run \
  --workflow video-crew-workflow.json \
  --workspace-root data/footage \
  --db-path data/footage/checkpoints.db
```

The workflow will execute Director planning and Editor audio synchronization, generate the EDL, and then pause at the approval gate:
```
Workflow run paused for human approval.
Node: gate_edl_approval
Approval ID: <APPROVAL_ID>
```

### Step 2: Review and Approve EDL
A user with an approver role (different from the run author) approves the gate:
```bash
cargo run --bin alba -- approve \
  --run-id <RUN_ID> \
  --approval-id <APPROVAL_ID> \
  --approver producer_lead \
  --decision allow_once \
  --db-path data/footage/checkpoints.db
```

### Step 3: Resume Workflow to Render and QC
```bash
cargo run --bin alba -- resume \
  --run-id <RUN_ID> \
  --workflow video-crew-workflow.json \
  --workspace-root data/footage \
  --db-path data/footage/checkpoints.db
```

The workflow resumes, executes `ffmpeg_render` to produce `data/footage/final_output.mp4`, runs automated `ffmpeg_qc`, and transitions to `Completed`.

---

## Dependencies & Licensing
- `ffmpeg` & `ffprobe`: External system CLI tools.
- License: LGPL v2.1+ or GPL v2+ depending on build flags.
- Invocation: Executed strictly as isolated subprocesses with explicit argument vectors. No static or dynamic C/FFI linking is used, guaranteeing that GPL copyleft does not attach to Alba application binaries.
