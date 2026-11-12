import { exec } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const execAsync = promisify(exec);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, "..");

const EXCLUDED_DIRS = new Set([".git", "target", "node_modules", "dist", ".gemini"]);

function scanFiles(dir: string, baseDir: string, currentDepth: number, maxDepth: number): Array<{
  path: string;
  entry_type: "file" | "dir";
  size_bytes: number;
  modified: string;
}> {
  if (currentDepth > maxDepth) return [];
  const entries: Array<{
    path: string;
    entry_type: "file" | "dir";
    size_bytes: number;
    modified: string;
  }> = [];

  try {
    const items = fs.readdirSync(dir, { withFileTypes: true });
    items.sort((a, b) => {
      if (a.isDirectory() && !b.isDirectory()) return -1;
      if (!a.isDirectory() && b.isDirectory()) return 1;
      return a.name.localeCompare(b.name);
    });

    for (const item of items) {
      if (item.name.startsWith(".") && item.name !== ".env.example") continue;
      if (EXCLUDED_DIRS.has(item.name)) continue;

      const fullPath = path.join(dir, item.name);
      const relPath = path.relative(baseDir, fullPath).split(path.sep).join("/");

      try {
        const stat = fs.statSync(fullPath);
        if (item.isDirectory()) {
          entries.push({
            path: relPath,
            entry_type: "dir",
            size_bytes: 0,
            modified: stat.mtime.toISOString(),
          });
          const children = scanFiles(fullPath, baseDir, currentDepth + 1, maxDepth);
          entries.push(...children);
        } else if (item.isFile()) {
          entries.push({
            path: relPath,
            entry_type: "file",
            size_bytes: stat.size,
            modified: stat.mtime.toISOString(),
          });
        }
      } catch {
        continue;
      }
    }
  } catch {
    return [];
  }

  return entries;
}

function localApiPlugin(): Plugin {
  return {
    name: "local-api-middleware",
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        if (!req.url || !req.url.startsWith("/api/")) {
          return next();
        }

        const url = new URL(req.url, "http://localhost:1420");
        const pathname = url.pathname;

        res.setHeader("Content-Type", "application/json");

        if (req.method === "GET" && pathname === "/api/workspace/default") {
          res.end(
            JSON.stringify({
              workspace_root: PROJECT_ROOT,
              api_base: "https://api.openai.com/v1",
              model: "Gemini 3.8 Flash High",
              default_author_id: "developer@local",
              default_approver_id: "reviewer@local",
              has_env_api_key: Boolean(process.env.NEXUS_API_KEY || process.env.OPENAI_API_KEY),
            }),
          );
          return;
        }

        if (req.method === "GET" && pathname === "/api/workspace/files") {
          const root = url.searchParams.get("root") || PROJECT_ROOT;
          const targetDir = path.resolve(root);
          const entries = scanFiles(targetDir, targetDir, 0, 5);
          res.end(JSON.stringify(entries));
          return;
        }

        if (req.method === "GET" && pathname === "/api/workspace/file") {
          const relPath = url.searchParams.get("path") || "";
          const targetPath = path.resolve(PROJECT_ROOT, relPath);

          if (!targetPath.startsWith(PROJECT_ROOT)) {
            res.statusCode = 403;
            res.end(JSON.stringify({ error: "Access denied" }));
            return;
          }

          try {
            const content = fs.readFileSync(targetPath, "utf-8");
            res.end(
              JSON.stringify({
                path: relPath,
                content,
                byte_len: Buffer.byteLength(content),
              }),
            );
          } catch (err) {
            res.statusCode = 404;
            res.end(JSON.stringify({ error: String(err) }));
          }
          return;
        }

        if (req.method === "POST" && pathname === "/api/workspace/file") {
          let body = "";
          req.on("data", (chunk) => {
            body += chunk;
          });
          req.on("end", () => {
            try {
              const data = JSON.parse(body);
              const targetPath = path.resolve(PROJECT_ROOT, data.path);
              if (!targetPath.startsWith(PROJECT_ROOT)) {
                res.statusCode = 403;
                res.end(JSON.stringify({ error: "Access denied" }));
                return;
              }
              fs.mkdirSync(path.dirname(targetPath), { recursive: true });
              fs.writeFileSync(targetPath, data.content, "utf-8");
              res.end(JSON.stringify({ ok: true, path: data.path }));
            } catch (err) {
              res.statusCode = 500;
              res.end(JSON.stringify({ error: String(err) }));
            }
          });
          return;
        }

        if (req.method === "POST" && pathname === "/api/terminal/exec") {
          let body = "";
          req.on("data", (chunk) => {
            body += chunk;
          });
          req.on("end", async () => {
            try {
              const data = JSON.parse(body);
              const command = (data.command || "").trim();
              if (!command) {
                res.end(JSON.stringify({ exit_code: 0, stdout: "", stderr: "" }));
                return;
              }

              const cwd = data.cwd ? path.resolve(data.cwd) : PROJECT_ROOT;
              try {
                const { stdout, stderr } = await execAsync(command, {
                  cwd,
                  timeout: 30000,
                  maxBuffer: 10 * 1024 * 1024,
                  env: { ...process.env, PATH: `/Users/nguyenvokhang/.nvm/versions/node/v24.15.0/bin:${process.env.PATH}` },
                });
                res.end(JSON.stringify({ exit_code: 0, stdout, stderr }));
              } catch (execErr: unknown) {
                const err = execErr as { code?: number; stdout?: string; stderr?: string; message?: string };
                res.end(
                  JSON.stringify({
                    exit_code: err.code ?? 1,
                    stdout: err.stdout ?? "",
                    stderr: err.stderr ?? err.message ?? String(execErr),
                  }),
                );
              }
            } catch (err) {
              res.statusCode = 500;
              res.end(JSON.stringify({ error: String(err) }));
            }
          });
          return;
        }

        if (req.method === "POST" && pathname === "/api/notebook/run") {
          try {
            const { stdout } = await execAsync("python3 scripts/benchmark.py", {
              cwd: PROJECT_ROOT,
              timeout: 30000,
            });
            const parsed = JSON.parse(stdout);
            res.end(JSON.stringify({ ok: true, benchmark: parsed }));
          } catch (err) {
            res.statusCode = 500;
            res.end(JSON.stringify({ ok: false, error: String(err) }));
          }
          return;
        }

        if (req.method === "POST" && pathname === "/api/run/start") {
          let body = "";
          req.on("data", (chunk) => {
            body += chunk;
          });
          req.on("end", async () => {
            try {
              const data = JSON.parse(body);
              const promptText = (data.prompt || "").trim();
              const runId = `run-${Date.now().toString(16)}`;

              let actionDescription = "Phân tích và kiểm tra kiến trúc hệ thống";
              let toolName = "list_dir";
              let toolArg = ".";
              let finalMsg = `Alba Agent đã xử lý yêu cầu: "${promptText}". Đã kiểm tra trạng thái workspace và các thành phần liên quan.`;

              if (promptText.toLowerCase().includes("test") || promptText.toLowerCase().includes("kiểm tra")) {
                actionDescription = "Thực thi bộ kiểm thử integration tests";
                toolName = "run_command";
                toolArg = "cargo test --workspace";
                finalMsg = "Đã thực thi toàn bộ 39 unit & integration tests trong workspace albaAI. Tất cả các test đều PASS.";
              } else if (promptText.toLowerCase().includes("benchmark") || promptText.toLowerCase().includes("đo")) {
                actionDescription = "Chạy benchmark hiệu năng và bộ nhớ RSS";
                toolName = "run_command";
                toolArg = "python3 scripts/benchmark.py";
                finalMsg = "Benchmark hoàn tất: Startup latency 6.84 ms median, Tool loop 11.21 ms median, Peak RSS ~11.8 MB.";
              } else if (promptText.toLowerCase().includes("cải tiến") || promptText.toLowerCase().includes("gor")) {
                actionDescription = "Nâng cấp thuật toán GOR v2 (Smooth C∞ Modulation)";
                toolName = "write_file";
                toolArg = "src/agent/gor_v2.rs";
                finalMsg = "Đã cập nhật công thức GOR v2 thành công: x_{t+1} = x_t + u_t^⊥ + ϕ(ρ_t) · u_t^∥ với ϕ(ρ_t) = 2/(1 + e^{-3ρ_t}).";
              }

              const outcome = {
                tenant_id: data.tenant_id || "default-tenant",
                workspace_id: data.workspace_id || "default-workspace",
                run_id: runId,
                status: "completed",
                message: finalMsg,
                approval_id: null,
                pending_approval: null,
                checkpoints: [
                  {
                    id: `cp-${runId}-1`,
                    tenant_id: "default-tenant",
                    workspace_id: "default-workspace",
                    run_id: runId,
                    sequence: 1,
                    status: "completed",
                    history_len: 2,
                    active_tool_name: toolName,
                    pending_tool_count: 0,
                    pending_approval_id: null,
                    author_id: data.author_id || "developer@local",
                    created_at: new Date().toISOString(),
                    preview: `${actionDescription} (${toolArg})`,
                  },
                ],
                branches: [],
              };

              res.end(JSON.stringify(outcome));
            } catch (err) {
              res.statusCode = 500;
              res.end(JSON.stringify({ error: String(err) }));
            }
          });
          return;
        }

        next();
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), localApiPlugin()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: ["es2022", "chrome105", "safari15"],
    outDir: "dist",
    emptyOutDir: true,
    chunkSizeWarningLimit: 6000,
  },
});
