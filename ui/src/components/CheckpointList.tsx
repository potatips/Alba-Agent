import { GitBranch, GitFork, History } from "lucide-react";
import type { CheckpointDto, TrajectoryBranchDto } from "../types";

interface CheckpointListProps {
  activeRunId: string | null;
  checkpoints: CheckpointDto[];
  branches: TrajectoryBranchDto[];
  onSwitchRun: (runId: string) => void;
  onRequestFork: (checkpoint: CheckpointDto) => void;
}

export function CheckpointList({
  activeRunId,
  checkpoints,
  branches,
  onSwitchRun,
  onRequestFork,
}: CheckpointListProps) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-slate-800 px-3 py-2">
        <span className="flex items-center gap-1.5 text-xs font-semibold tracking-wider text-slate-400 uppercase">
          <History className="h-3.5 w-3.5 text-violet-400" />
          Time-Travel Checkpoints
        </span>
      </div>

      {branches.length > 0 && (
        <div className="border-b border-slate-800 px-3 py-2">
          <p className="mb-1.5 text-[11px] font-medium text-slate-400">Trajectory Branches</p>
          <div className="space-y-1">
            {branches.map((branch) => {
              const active = activeRunId === branch.branch_run_id;
              return (
                <button
                  key={branch.id}
                  type="button"
                  onClick={() => onSwitchRun(branch.branch_run_id)}
                  className={`flex w-full items-center justify-between rounded border px-2 py-1 text-left text-xs transition ${
                    active
                      ? "border-violet-500/60 bg-violet-500/15 text-violet-200"
                      : "border-slate-800 bg-slate-900/60 text-slate-300 hover:border-slate-700"
                  }`}
                >
                  <span className="flex items-center gap-1.5 truncate">
                    <GitBranch className="h-3 w-3 shrink-0 text-violet-400" />
                    <span className="truncate font-medium">{branch.branch_label}</span>
                  </span>
                  <span className="ml-2 shrink-0 text-[10px] text-slate-500">
                    seq {branch.parent_sequence}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-2 py-2">
        {checkpoints.length === 0 ? (
          <p className="px-2 py-4 text-xs text-slate-500">
            Start a run to capture atomic SQLite checkpoints.
          </p>
        ) : (
          <ol className="space-y-2">
            {checkpoints.map((cp) => (
              <li
                key={cp.id}
                className="rounded-md border border-slate-800 bg-slate-900/70 p-2.5 text-xs"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono font-semibold text-slate-200">
                    Seq #{cp.sequence}
                  </span>
                  <span className="rounded bg-slate-800 px-1.5 py-0.5 text-[10px] font-medium text-sky-300 uppercase">
                    {cp.status}
                  </span>
                </div>
                <p className="mt-1 line-clamp-2 text-[11px] text-slate-400">{cp.preview}</p>
                <div className="mt-2 flex items-center justify-between text-[10px] text-slate-500">
                  <span>{cp.history_len} msgs</span>
                  {cp.active_tool_name && (
                    <span className="font-mono text-amber-300">{cp.active_tool_name}</span>
                  )}
                </div>
                <button
                  type="button"
                  onClick={() => onRequestFork(cp)}
                  className="mt-2 flex w-full items-center justify-center gap-1.5 rounded border border-violet-500/40 bg-violet-500/10 px-2 py-1 text-[11px] font-medium text-violet-200 transition hover:bg-violet-500/20"
                >
                  <GitFork className="h-3 w-3" />
                  Fork from Seq #{cp.sequence}
                </button>
              </li>
            ))}
          </ol>
        )}
      </div>
    </div>
  );
}
