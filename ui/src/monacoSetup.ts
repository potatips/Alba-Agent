import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

interface MonacoGlobal {
  MonacoEnvironment?: {
    getWorker: (_moduleId: string, label: string) => Worker;
  };
}

let initialized = false;

export function ensureMonacoConfigured(): void {
  if (initialized) {
    return;
  }
  initialized = true;

  const target = self as unknown as MonacoGlobal;
  target.MonacoEnvironment = {
    getWorker(_moduleId: string, label: string): Worker {
      if (label === "json") {
        return new jsonWorker();
      }
      if (label === "css" || label === "scss" || label === "less") {
        return new cssWorker();
      }
      if (label === "html" || label === "handlebars" || label === "razor") {
        return new htmlWorker();
      }
      if (label === "typescript" || label === "javascript") {
        return new tsWorker();
      }
      return new editorWorker();
    },
  };

  loader.config({ monaco });
}

export function inferLanguageFromPath(filePath: string): string {
  const lower = filePath.toLowerCase();
  if (lower.endsWith(".rs")) return "rust";
  if (lower.endsWith(".ts") || lower.endsWith(".tsx")) return "typescript";
  if (lower.endsWith(".js") || lower.endsWith(".jsx") || lower.endsWith(".mjs")) {
    return "javascript";
  }
  if (lower.endsWith(".json")) return "json";
  if (lower.endsWith(".toml") || lower.endsWith(".ini")) return "ini";
  if (lower.endsWith(".yaml") || lower.endsWith(".yml")) return "yaml";
  if (lower.endsWith(".md")) return "markdown";
  if (lower.endsWith(".css")) return "css";
  if (lower.endsWith(".html")) return "html";
  if (lower.endsWith(".sh") || lower.endsWith(".bash")) return "shell";
  if (lower.endsWith(".py")) return "python";
  if (lower.endsWith(".sql")) return "sql";
  return "plaintext";
}
