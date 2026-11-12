#!/usr/bin/env python3

import http.server
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import threading
import time

class MockOpenAiHandler(http.server.BaseHTTPRequestHandler):
    turn_counter = 0

    def log_message(self, format, *args):
        pass

    def do_POST(self):
        if self.path == "/v1/chat/completions" or self.path == "/chat/completions":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            payload = json.loads(body.decode("utf-8"))

            MockOpenAiHandler.turn_counter += 1

            if MockOpenAiHandler.turn_counter % 2 == 1:
                response = {
                    "id": "chatcmpl-bench-1",
                    "object": "chat.completion",
                    "created": 1700000000,
                    "model": "gpt-4o-mini",
                    "choices": [
                        {
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": None,
                                "tool_calls": [
                                    {
                                        "id": "call_bench_1",
                                        "type": "function",
                                        "function": {
                                            "name": "read_file",
                                            "arguments": json.dumps({"path": "fixture.txt"})
                                        }
                                    }
                                ]
                            },
                            "finish_reason": "tool_calls"
                        }
                    ],
                    "usage": {
                        "prompt_tokens": 30,
                        "completion_tokens": 15,
                        "total_tokens": 45
                    }
                }
            else:
                response = {
                    "id": "chatcmpl-bench-2",
                    "object": "chat.completion",
                    "created": 1700000001,
                    "model": "gpt-4o-mini",
                    "choices": [
                        {
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": "The fixture file contains: benchmark data."
                            },
                            "finish_reason": "stop"
                        }
                    ],
                    "usage": {
                        "prompt_tokens": 55,
                        "completion_tokens": 12,
                        "total_tokens": 67
                    }
                }

            resp_bytes = json.dumps(response).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(resp_bytes)))
            self.end_headers()
            self.wfile.write(resp_bytes)
        else:
            self.send_response(404)
            self.end_headers()

def summarize(values):
    ordered = sorted(values)
    n = len(ordered)
    q1 = statistics.quantiles(ordered, n=4)[0] if n >= 4 else ordered[0]
    q3 = statistics.quantiles(ordered, n=4)[2] if n >= 4 else ordered[-1]
    return {
        "iterations": n,
        "median": round(statistics.median(ordered), 2),
        "mean": round(statistics.mean(ordered), 2),
        "min": round(ordered[0], 2),
        "max": round(ordered[-1], 2),
        "iqr": round(q3 - q1, 2),
        "stdev": round(statistics.stdev(ordered), 2) if n >= 2 else 0.0,
    }


def run_measured(cmd, env=None):
    t0 = time.perf_counter()
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    _, status, ru = os.wait4(proc.pid, 0)
    t1 = time.perf_counter()
    stdout = proc.stdout.read() if proc.stdout else b""
    stderr = proc.stderr.read() if proc.stderr else b""
    if proc.stdout:
        proc.stdout.close()
    if proc.stderr:
        proc.stderr.close()
    exit_code = os.waitstatus_to_exitcode(status)
    return exit_code, (t1 - t0) * 1000.0, ru.ru_maxrss, stdout, stderr


def main():
    binary_path = os.path.abspath(
        os.path.join(os.path.dirname(__file__), "..", "target", "release", "nexus-albaai")
    )
    if not os.path.exists(binary_path):
        print(f"Error: binary not found at {binary_path}", file=sys.stderr)
        sys.exit(1)

    binary_size_bytes = os.path.getsize(binary_path)
    rustc_ver = subprocess.check_output(["rustc", "--version"], text=True).strip()

    startup_latencies_ms = []
    startup_rss_kb = []
    for _ in range(20):
        code, elapsed_ms, rss_kb, _, err = run_measured([binary_path, "--version"])
        if code != 0:
            print(f"Error: --version failed: {err.decode(errors='replace')}", file=sys.stderr)
            sys.exit(1)
        startup_latencies_ms.append(elapsed_ms)
        startup_rss_kb.append(rss_kb)

    server = http.server.HTTPServer(("127.0.0.1", 0), MockOpenAiHandler)
    server_port = server.server_address[1]
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()

    with tempfile.TemporaryDirectory() as tmp_dir:
        fixture_file = os.path.join(tmp_dir, "fixture.txt")
        with open(fixture_file, "w") as f:
            f.write("benchmark data\n")

        db_path = os.path.join(tmp_dir, "bench.db")
        api_base = f"http://127.0.0.1:{server_port}/v1"

        fixture_latencies_ms = []
        peak_rss_kb_list = []

        bench_env = {**os.environ, "NEXUS_API_KEY": "test-bench-key"}
        for i in range(20):
            MockOpenAiHandler.turn_counter = 0
            code, elapsed_ms, rss_kb, _, err = run_measured(
                [
                    binary_path,
                    "run",
                    "--prompt",
                    "Read fixture.txt and summarize it",
                    "--workspace-root",
                    tmp_dir,
                    "--db-path",
                    db_path,
                    "--api-base",
                    api_base,
                    "--model",
                    "gpt-4o-mini",
                ],
                env=bench_env,
            )
            if code != 0:
                print(
                    f"Error during run {i}: {err.decode(errors='replace')}",
                    file=sys.stderr,
                )
                sys.exit(1)

            fixture_latencies_ms.append(elapsed_ms)
            peak_rss_kb_list.append(rss_kb)

    server.shutdown()

    results = {
        "environment": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "rustc": rustc_ver,
            "python": platform.python_version(),
        },
        "binary_size_bytes": binary_size_bytes,
        "binary_size_mib": round(binary_size_bytes / (1024 * 1024), 2),
        "startup_ms": summarize(startup_latencies_ms),
        "startup_peak_rss_kb": summarize(startup_rss_kb),
        "single_tool_loop_ms": summarize(fixture_latencies_ms),
        "single_tool_loop_peak_rss_kb": summarize(peak_rss_kb_list),
    }

    print(json.dumps(results, indent=2))

if __name__ == "__main__":
    main()
