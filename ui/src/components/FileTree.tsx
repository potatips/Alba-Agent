import { FileCode2, Folder, RefreshCw } from "lucide-react";
import type { WorkspaceEntryDto } from "../types";

interface FileTreeProps {
  entries: WorkspaceEntryDto[];
  selectedPath: string | null;
  loading: boolean;
  onSelectFile: (path: string) => void;
  onRefresh: () => void;
}

export function FileTree({
  entries,
  selectedPath,
  loading,
  onSelectFile,
  onRefresh,
}: FileTreeProps) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-slate-800 px-3 py-2">
        <span className="text-xs font-semibold tracking-wider text-slate-400 uppercase">
          Workspace Files
        </span>
        <button
          type="button"
          onClick={onRefresh}
          disabled={loading}
          aria-label="Refresh workspace files"
          className="rounded p-1 text-slate-400 transition hover:bg-slate-800 hover:text-slate-100 disabled:opacity-40"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto px-1.5 py-2">
        {entries.length === 0 ? (
          <p className="px-2 py-4 text-xs text-slate-500">No workspace files loaded.</p>
        ) : (
          <ul className="space-y-0.5">
            {entries.map((entry) => {
              const depth = entry.path.split("/").length - 1;
              const name = entry.path.split("/").pop() ?? entry.path;
              const isSelected = selectedPath === entry.path;
              const paddingLeft = 8 + depth * 12;

              if (entry.entry_type === "dir") {
                return (
                  <li
                    key={entry.path}
                    style={{ paddingLeft }}
                    className="flex items-center gap-1.5 rounded px-2 py-1 text-xs text-slate-400"
                  >
                    <Folder className="h-3.5 w-3.5 shrink-0 text-amber-400/80" />
                    <span className="truncate font-medium">{name}</span>
                  </li>
                );
              }

              return (
                <li key={entry.path}>
                  <button
                    type="button"
                    onClick={() => onSelectFile(entry.path)}
                    style={{ paddingLeft }}
                    className={`flex w-full items-center gap-1.5 rounded px-2 py-1 text-left text-xs transition ${
                      isSelected
                        ? "bg-sky-500/20 text-sky-200"
                        : "text-slate-300 hover:bg-slate-800/80 hover:text-white"
                    }`}
                  >
                    <FileCode2 className="h-3.5 w-3.5 shrink-0 text-sky-400/80" />
                    <span className="truncate">{name}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
