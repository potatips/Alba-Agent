import { useEffect, useState } from "react";
import Editor, { DiffEditor } from "@monaco-editor/react";
import { Check, Code2, GitCompare, Save } from "lucide-react";
import { ensureMonacoConfigured, inferLanguageFromPath } from "../monacoSetup";
import { saveWorkspaceFile } from "../tauri";
import type { DiffSelection } from "../types";

interface CodeDiffPaneProps {
  selectedPath: string | null;
  fileContent: string;
  activeDiff: DiffSelection | null;
  viewMode: "code" | "diff";
  onChangeViewMode: (mode: "code" | "diff") => void;
  onFileSaved?: (path: string, newContent: string) => void;
}

ensureMonacoConfigured();

export function CodeDiffPane({
  selectedPath,
  fileContent,
  activeDiff,
  viewMode,
  onChangeViewMode,
  onFileSaved,
}: CodeDiffPaneProps) {
  const effectivePath = viewMode === "diff" && activeDiff ? activeDiff.path : selectedPath ?? "";
  const language = inferLanguageFromPath(effectivePath || "untitled.txt");

  const [currentText, setCurrentText] = useState(fileContent);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [savedSuccess, setSavedSuccess] = useState(false);

  useEffect(() => {
    setCurrentText(fileContent);
    setDirty(false);
  }, [fileContent, selectedPath]);

  const handleSave = async () => {
    if (!selectedPath || saving) return;
    setSaving(true);
    try {
      await saveWorkspaceFile(selectedPath, currentText);
      setDirty(false);
      setSavedSuccess(true);
      onFileSaved?.(selectedPath, currentText);
      setTimeout(() => setSavedSuccess(false), 2000);
    } catch (err) {
      alert(`Save error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setSaving(false);
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        handleSave();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedPath, currentText, saving]);

  return (
    <div className="flex h-full flex-col bg-[#09090b]">
      <div className="flex items-center justify-between border-b border-zinc-800 bg-[#0c0c0e] px-3 py-1.5">
        <div className="flex items-center gap-2 truncate text-xs">
          <span className="font-mono text-zinc-200 truncate">
            {effectivePath || "No file selected (select a file in the left Explorer)"}
          </span>
          {activeDiff && viewMode === "diff" && (
            <span className="rounded bg-amber-500/20 px-1.5 py-0.5 text-[10px] font-medium text-amber-300">
              Inline Diff
            </span>
          )}
          {dirty && (
            <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] text-blue-300">
              Unsaved changes
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          {selectedPath && viewMode === "code" && (
            <button
              type="button"
              disabled={saving || !dirty}
              onClick={handleSave}
              className={`flex items-center gap-1 rounded px-2.5 py-1 text-xs font-medium transition ${
                dirty
                  ? "bg-blue-600 text-white hover:bg-blue-500"
                  : "bg-zinc-800 text-zinc-400"
              }`}
            >
              {savedSuccess ? (
                <>
                  <Check className="h-3 w-3 text-emerald-400" />
                  <span>Saved</span>
                </>
              ) : (
                <>
                  <Save className="h-3 w-3" />
                  <span>{saving ? "Saving..." : "Save (Cmd+S)"}</span>
                </>
              )}
            </button>
          )}

          <div className="flex items-center gap-1 rounded-md border border-zinc-800 bg-zinc-950 p-0.5 text-[11px]">
            <button
              type="button"
              onClick={() => onChangeViewMode("code")}
              className={`flex items-center gap-1 rounded px-2 py-0.5 transition ${
                viewMode === "code"
                  ? "bg-zinc-800 text-white"
                  : "text-zinc-400 hover:text-zinc-200"
              }`}
            >
              <Code2 className="h-3 w-3" />
              Code
            </button>
            <button
              type="button"
              disabled={!activeDiff}
              onClick={() => onChangeViewMode("diff")}
              className={`flex items-center gap-1 rounded px-2 py-0.5 transition disabled:opacity-35 ${
                viewMode === "diff"
                  ? "bg-zinc-800 text-amber-200"
                  : "text-zinc-400 hover:text-zinc-200"
              }`}
            >
              <GitCompare className="h-3 w-3" />
              Diff
            </button>
          </div>
        </div>
      </div>

      <div className="min-h-0 flex-1">
        {viewMode === "diff" && activeDiff ? (
          <DiffEditor
            height="100%"
            theme="vs-dark"
            language={language}
            original={activeDiff.original}
            modified={activeDiff.modified}
            options={{
              readOnly: true,
              renderSideBySide: false,
              minimap: { enabled: false },
              fontSize: 12,
              scrollBeyondLastLine: false,
            }}
          />
        ) : (
          <Editor
            height="100%"
            theme="vs-dark"
            language={language}
            value={currentText}
            onChange={(val) => {
              setCurrentText(val ?? "");
              setDirty(true);
            }}
            options={{
              readOnly: !selectedPath,
              minimap: { enabled: false },
              fontSize: 12,
              wordWrap: "on",
              scrollBeyondLastLine: false,
            }}
          />
        )}
      </div>
    </div>
  );
}
