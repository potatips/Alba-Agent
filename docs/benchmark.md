# Alba Agent Engine & Desktop Benchmark Report

## 1. Scope & Comparison Disclaimer

- **What this measures:**
  - `scripts/benchmark.py` measures the headless CLI engine (`target/release/nexus-albaai`) on standalone process startup (`nexus-albaai --version`) and a 2-turn single-tool agent loop (`nexus-albaai run` executing one `read_file` invocation, SQLite WAL checkpoint persistence, and a local loopback mock OpenAI-compatible HTTP server, using `NEXUS_API_KEY` via process environment).
  - Release artifact footprint is measured via `stat` for both `target/release/nexus-albaai` (CLI engine) and `target/release/alba-agent` (full Tauri v2 desktop binary embedding `ui/dist`), alongside `du -sh ui/dist`.
- **Comparison with Python NexusAgent:** **not directly benchmarked side-by-side here**. Only the compiled `nexus-albaai` runtime is timed in this report.

---

## 2. Environment

- **Platform:** `Linux-6.12.116+deb13-arm64-aarch64-with-glibc2.41` (`aarch64`)
- **Compiler:** `rustc 1.99.1 (58cf8f9c3 2026-10-13)`
- **Optimization Profile:** `release` (`cargo build --release --workspace`)
- **Measurement Harness:** `scripts/benchmark.py` (Python `3.13.5`, `time.perf_counter()`, `os.wait4` `ru_maxrss`, `subprocess.Popen`)

---

## 3. Measured Results

| Metric | Repetitions | Median | Mean | Spread (Min – Max) | IQR | StdDev |
|---|---|---|---|---|---|---|
| **CLI Engine Release Binary (`nexus-albaai`)** | static artifact | `8,229,112` bytes (`7.85 MiB`) | — | — | — | — |
| **Desktop App Release Binary (`alba-agent`, Tauri v2 + embedded `ui/dist`)** | static artifact | `13,300,928` bytes (`12.68 MiB`) | — | — | — | — |
| **Built Frontend Bundle (`ui/dist`, Vite + React 19 + Monaco + Xterm)** | static directory | `14M` (`du -sh`) | — | — | — | — |
| **Cold CLI Startup Latency (`--version`)** | 20 | **60.35 ms** | 60.34 ms | 49.55 ms – 69.93 ms | 9.17 ms | 5.67 ms |
| **Cold CLI Startup Peak RSS (`--version`)** | 20 | **18,724.0 KB** (`18.29 MiB`) | 18,716 KB | 18,564 KB – 18,724 KB | 0.0 KB | 35.78 KB |
| **Single-Tool Agent Loop (`read_file` fixture)** | 20 | **113.67 ms** | 113.91 ms | 95.98 ms – 153.31 ms | 8.16 ms | 11.61 ms |
| **Single-Tool Loop Peak RSS (`read_file` fixture)** | 20 | **19,540.0 KB** (`19.08 MiB`) | 19,524.8 KB | 19,236 KB – 19,540 KB | 0.0 KB | 67.98 KB |

---

## 4. Workload Definition

Each single-tool fixture repetition spawns a fresh `target/release/nexus-albaai run` child process with `NEXUS_API_KEY` set in its environment, executing:
1. CLI argument parsing (`clap`) and tenant/workspace scope initialization (`ExecutionScope`).
2. SQLite connection (`0600`), WAL setup, and schema migration verification.
3. Initial `running` checkpoint and `run.started` audit record.
4. Model Turn 1: HTTP POST to the local mock OpenAI-compatible handler returning a `read_file("fixture.txt")` tool call.
5. Policy evaluation, workspace path canonicalization (`ReadFileTool`), and reading `fixture.txt`.
6. Turn 1 checkpoint persistence in SQLite.
7. Model Turn 2: HTTP POST with tool evidence returning a final text response.
8. Final `completed` checkpoint, `run.completed` audit record, and JSONL event output.

---

## 5. Reproducibility Commands

```bash
npm --prefix ui install
npm --prefix ui run build
cargo build --release --workspace
python3 scripts/benchmark.py
```
