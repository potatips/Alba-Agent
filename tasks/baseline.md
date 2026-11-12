# NexusAgent Baseline Measurements (Phase G0)

Date: 2026-11-12
Host: macOS (Apple Silicon arm64)
Reference Location: `/Users/nguyenvokhang/Downloads/v3/NexusAgent` (Read-only)

---

## 1. Summary of Baseline Metrics

| Metric | Value | Measurement Method | Command / Evidence Source | Status |
| :--- | :--- | :--- | :--- | :--- |
| **1. Install Size (Lean)** | ~1.15 GB | README specification | `NexusAgent/apps/desktop/README.md:38` | **documented (README), not measured** |
| **1b. Install Size (Full)** | ~1.50 GB | README specification | `NexusAgent/apps/desktop/README.md:51` | **documented (README), not measured** |
| **1c. Source Code Size** | 25.9 MB (core) / 952 MB (with dev deps) | Disk usage command | `du -sh NexusAgent` (core apps: 23MB, packages: 2.9MB, dev node_modules: 924MB) | **measured** |
| **2. Idle RAM (Default Lean Desktop)** | ~274 MB – 403 MB | Live proxy RSS via `ps` | `ps -eo pid,ppid,rss,command`: Next.js web dev (22.6 MB) + empty Electron test app (196.1 MB) + bare uvicorn (50.5 MB); upper bound includes unmeasured 180 MB API figure | **proxy measurement, not NexusAgent actual** |
| **2b. Idle RAM (Postgres Full Stack)** | ~850 MB – 1,150 MB | Multi-process sum (6 services + Electron) | Architecture audit: Postgres (~55MB) + Redis (~20MB) + Qdrant (~105MB) + Celery (~320MB) + API (~180MB) + Web (~56MB) + Electron (~350MB) | **documented (architecture), not measured** |
| **3. Startup Time (Warm)** | Median: **11.56s** (Min: 7.00s, Max: 122.06s) | Complete parsing of all 32 runs in startup log | `~/Library/Application Support/@nexus/desktop/logs/startup.log` | **measured** |
| **3b. Startup Time (Cold)** | ~60s ("about a minute") | README documentation | `NexusAgent/apps/desktop/README.md:19` (`initdb` took 27.08s, first run 11.11s) | **documented (README), not verified** (by single 60s log line) |
| **4. Sample Run Time** | **27.22s** | Celery worker task log | `~/Library/Application Support/@nexus/desktop/logs/worker.log` | **measured** |

---

## 2. Detailed Metric Evidence

### Metric 1: Install Size
- Command:
  ```bash
  du -sh /Users/nguyenvokhang/Downloads/v3/NexusAgent/apps
  du -sh /Users/nguyenvokhang/Downloads/v3/NexusAgent/packages
  ```
- Raw Output:
  ```text
  23M	/Users/nguyenvokhang/Downloads/v3/NexusAgent/apps
  2.9M	/Users/nguyenvokhang/Downloads/v3/NexusAgent/packages
  ```
- App Bundle Status:
  No pre-built `.app` bundle or pre-packaged `runtime/mac-arm64` directory is present on disk in this checkout.
- Label:
  - 1.15 GB lean / 1.50 GB full bundle: **documented (README), not measured**.
  - 25.9 MB source code: **measured**.

---

### Metric 2: Idle RAM
The NexusAgent desktop application operates in two distinct execution modes defined in `apps/desktop/src/stack.ts`:
1. **Default Mode (Single-User)**: `NEXUS_DESKTOP_USE_POSTGRES != "true"` (default). Starts SQLite (`aiosqlite`), in-process Redis memory mock (`MemoryRedis`), embedded Qdrant, in-process task queue (`AsyncioDispatcher`), Next.js standalone web server, and Electron. Spawns only: API, Web, and Electron.
2. **Postgres Full Stack Mode**: `NEXUS_DESKTOP_USE_POSTGRES == "true"` or `--full` build. Spawns 6 external processes: `postgres`, `redis-server`, `qdrant`, `uvicorn` (API), `celery` (worker), `next-server` (Web), plus Electron.

#### A. Proxy Measurement of Default Lean Desktop Mode (Proxy measurement, not NexusAgent actual)
- Context & Disclaimer:
  Because the complete precompiled `runtime/mac-arm64` bundle containing `crewai`, standalone Python wheels, and full NexusAgent services is not present in this repository checkout, this is a proxy measurement using the actual available runtimes on host: Next.js dev server, an empty Electron test app loading `/studio`, and a bare Python FastAPI uvicorn server. It does not measure the full NexusAgent API runtime with all embedded modules loaded.
- Execution Command:
  ```bash
  ps -eo pid,ppid,rss,command | grep -E "node.*next dev|Electron.*test_app|Electron Helper"
  ```
- Raw Output:
  ```text
  48167 48163  22576 node ./node_modules/.bin/../next/dist/bin/next dev -p 3099
  48182 48163  75760 .../Electron.app/Contents/MacOS/Electron /tmp/test_app
  48190 48182  43728 .../Electron Helper (GPU).app/Contents/MacOS/Electron Helper (GPU)
  48191 48182  27936 .../Electron Helper.app/Contents/MacOS/Electron Helper --type=utility
  48196 48182  53424 .../Electron Helper (Renderer).app/Contents/MacOS/Electron Helper (Renderer)
  ```
- Component Breakdown:
  - Next.js Web Dev Server: 22,576 KB (~22.1 MB)
  - Electron Main Process (empty test app): 75,760 KB (~74.0 MB)
  - Electron GPU Helper: 43,728 KB (~42.7 MB)
  - Electron Network Utility Helper: 27,936 KB (~27.3 MB)
  - Electron Renderer Helper (`/studio` view): 53,424 KB (~52.2 MB)
  - Combined Empty Electron Total: 200,848 KB (~196.1 MB)
  - Bare Python FastAPI / Uvicorn Server: 51,680 KB (~50.5 MB) (measured via `ps -o pid,ppid,rss,command -p $API_PID`)
- **Total Proxy Default Desktop Idle RAM: ~274 MB – 403 MB** (The 403 MB upper bound includes an unmeasured 180 MB API estimate from full-stack architecture).
- Status: **proxy measurement, not NexusAgent actual**.

#### B. Full Multi-Process Postgres Mode (Documented / Architecture)
- Component Breakdown (6 background processes + Electron):
  - Postgres: ~55 MB
  - Redis: ~20 MB
  - Qdrant: ~105 MB
  - Python Celery Worker: ~320 MB
  - Python FastAPI API: ~180 MB
  - Next.js Web: ~56 MB
  - Electron (Main + Renderer + GPU): ~350 MB
- Total Full Stack Mode: **~850 MB – 1,150 MB** (Status: **documented (architecture), not measured**).

---

### Metric 3: Startup Time
Source Log: `~/Library/Application Support/@nexus/desktop/logs/startup.log`

#### Complete Run Distribution (All 32 Starts in Log)
Duration computed from first `Starting the database…` to `Ready`:
- Run 1 (2026-11-07 17:48:39 -> 17:48:50): 11.11s
- Run 2 (2026-11-07 17:56:50 -> 17:57:01): 11.28s
- Run 3 (2026-11-07 18:04:54 -> 18:05:03): 8.88s
- Run 4 (2026-11-07 19:23:42 -> 19:23:52): 10.15s
- Run 5 (2026-11-07 19:35:33 -> 19:35:44): 10.78s
- Run 6 (2026-11-07 19:46:36 -> 19:46:48): 11.56s
- Run 7 (2026-11-08 07:07:05 -> 07:07:12): 7.26s
- Run 8 (2026-11-08 07:19:30 -> 07:19:43): 12.46s
- Run 9 (2026-11-08 07:25:21 -> 07:25:33): 11.56s
- Run 10 (2026-11-08 07:31:22 -> 07:31:37): 15.90s
- Run 11 (2026-11-08 07:49:03 -> 07:49:11): 7.94s
- Run 12 (2026-11-08 13:05:36 -> 13:05:43): 7.15s
- Run 13 (2026-11-08 13:14:37 -> 13:14:44): 7.74s
- Run 14 (2026-11-08 13:19:24 -> 13:19:34): 10.13s
- Run 15 (2026-11-08 13:57:43 -> 13:57:50): 7.36s
- Run 16 (2026-11-08 14:19:51 -> 14:20:06): 15.27s
- Run 17 (2026-11-08 14:40:19 -> 14:40:31): 11.86s
- Run 18 (2026-11-08 14:46:32 -> 14:48:15): 103.17s
- Run 19 (2026-11-08 18:11:01 -> 18:11:11): 10.63s
- Run 20 (2026-11-08 18:17:32 -> 18:17:44): 11.69s
- Run 21 (2026-11-08 18:48:33 -> 18:49:58): 84.66s
- Run 22 (2026-11-08 18:58:40 -> 18:59:38): 57.37s
- Run 23 (2026-11-08 19:09:16 -> 19:09:39): 23.07s
- Run 24 (2026-11-09 03:57:00 -> 03:59:02): 122.06s
- Run 25 (2026-11-09 04:32:18 -> 04:32:25): 7.10s
- Run 26 (2026-11-09 04:39:53 -> 04:40:00): 7.00s
- Run 27 (2026-11-09 05:08:02 -> 05:08:24): 22.72s
- Run 28 (2026-11-09 05:18:53 -> 05:19:38): 45.03s
- Run 29 (2026-11-09 05:24:42 -> 05:24:56): 13.97s
- Run 30 (2026-11-09 07:13:56 -> 07:14:17): 20.81s
- Run 31 (2026-11-09 08:06:27 -> 08:06:37): 10.42s
- Run 32 (2026-11-09 13:24:32 -> 13:24:51): 19.24s

#### Summary Statistics
- Minimum: **7.00s** (Run 26)
- Median: **11.56s** (Run 6, Run 9)
- Mean: **23.04s**
- Maximum: **122.06s** (Run 24)
- 2026-11-09 13:24 start: **19.24s** (13:24:32.213Z -> 13:24:51.453Z)
- Status: **measured**.

#### Cold Start Analysis
- `initdb.log`:
  ```text
  2026-11-07T08:47:45.816Z The files belonging to this database system will be owned by user "nguyenvokhang".
  ...
  2026-11-07T08:48:12.892Z Success. You can now start the database server using...
  ```
  Initial cluster creation: **27.08s**. First startup turn: **11.11s**. Total initial cold sequence: **~38.19s**.
- The "60s" figure: Stated in `apps/desktop/README.md:19` ("takes about a minute").
- Status: **documented (README), not verified** (by a single 60.0s log line).

---

### Metric 4: Sample Run Execution Time
- Value: **27.22s**
- Evidence Source: `~/Library/Application Support/@nexus/desktop/logs/worker.log`:
  ```text
  [2026-11-09 20:25:31,452: INFO/ForkPoolWorker-1] Task nexus.run_workflow[db8bfd05-5a8d-4e37-aa23-bf6cff32633c] succeeded in 27.221581999998307s
  ```
- Status: **measured**.

---

## 3. Verification Command Logs on NexusAgent

### A. `pnpm lint`
```text
> alba-agent@ lint /Users/nguyenvokhang/Downloads/v3/NexusAgent
> pnpm --dir apps/web lint

> @nexus/web@1.0.0 lint /Users/nguyenvokhang/Downloads/v3/NexusAgent/apps/web
> eslint src --max-warnings=0

Exit code: 0
```

### B. `pnpm check`
```text
> alba-agent@ check /Users/nguyenvokhang/Downloads/v3/NexusAgent
> pnpm --dir apps/web check

> @nexus/web@1.0.0 check /Users/nguyenvokhang/Downloads/v3/NexusAgent/apps/web
> tsc --noEmit

Exit code: 0
```

### C. `pnpm test`
```text
> alba-agent@ test /Users/nguyenvokhang/Downloads/v3/NexusAgent
> pnpm --dir apps/web test

> @nexus/web@1.0.0 test /Users/nguyenvokhang/Downloads/v3/NexusAgent/apps/web
> node --test

ℹ tests 89
ℹ suites 0
ℹ pass 89
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 791.932416

Exit code: 0
```

### D. `pytest packages/`
Run with `PYTHONDONTWRITEBYTECODE=1` and `pytest -p no:cacheprovider`:
```text
pytest packages/ -q -p no:cacheprovider
...................................s........s........................... [ 79%]
...................                                                      [100%]
89 passed, 2 skipped in 0.57s

Exit code: 0
```
Note on `apps/api/tests`: Full API suite requires runtime external dependencies (`crewai`) that are packaged only inside precompiled runtime images. The pure packages test suite passes 100% (89 passed, 2 skipped).
