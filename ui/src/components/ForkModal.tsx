import { useState, type FormEvent } from "react";
import { GitFork, X } from "lucide-react";
import type { CheckpointDto } from "../types";

interface ForkModalProps {
  checkpoint: CheckpointDto;
  defaultModel: string;
  busy: boolean;
  onCancel: () => void;
  onSubmit: (params: {
    branchLabel: string;
    overridePrompt?: string;
    overrideModel?: string;
    autoResume: boolean;
  }) => void;
}

export function ForkModal({
  checkpoint,
  defaultModel,
  busy,
  onCancel,
  onSubmit,
}: ForkModalProps) {
  const [branchLabel, setBranchLabel] = useState(`fork-seq-${checkpoint.sequence}`);
  const [overridePrompt, setOverridePrompt] = useState("");
  const [overrideModel, setOverrideModel] = useState(defaultModel);
  const [autoResume, setAutoResume] = useState(true);

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (!branchLabel.trim()) return;
    onSubmit({
      branchLabel: branchLabel.trim(),
      overridePrompt: overridePrompt.trim() ? overridePrompt.trim() : undefined,
      overrideModel:
        overrideModel.trim() && overrideModel.trim() !== defaultModel
          ? overrideModel.trim()
          : undefined,
      autoResume,
    });
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/80 p-4 backdrop-blur-xs"
      role="dialog"
      aria-modal="true"
      aria-label="Fork Trajectory"
    >
      <form
        onSubmit={handleSubmit}
        className="w-full max-w-lg rounded-xl border border-slate-700 bg-slate-900 p-5 shadow-2xl"
      >
        <div className="flex items-center justify-between border-b border-slate-800 pb-3">
          <h2 className="flex items-center gap-2 text-sm font-semibold text-white">
            <GitFork className="h-4 w-4 text-violet-400" />
            Time-Travel A/B Trajectory Fork (Seq #{checkpoint.sequence})
          </h2>
          <button
            type="button"
            onClick={onCancel}
            className="rounded p-1 text-slate-400 hover:bg-slate-800 hover:text-white"
            aria-label="Close fork modal"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="mt-4 space-y-3 text-xs">
          <div>
            <label className="mb-1 block font-medium text-slate-300" htmlFor="fork-label">
              Branch Label
            </label>
            <input
              id="fork-label"
              type="text"
              required
              value={branchLabel}
              onChange={(e) => setBranchLabel(e.target.value)}
              className="w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-slate-100 focus:border-violet-500 focus:outline-none"
            />
          </div>

          <div>
            <label className="mb-1 block font-medium text-slate-300" htmlFor="fork-prompt">
              Override Prompt (optional counterfactual input)
            </label>
            <textarea
              id="fork-prompt"
              rows={3}
              value={overridePrompt}
              onChange={(e) => setOverridePrompt(e.target.value)}
              placeholder="Leave empty to replay cloned checkpoint state as-is, or inject a new steering instruction..."
              className="w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-slate-100 focus:border-violet-500 focus:outline-none"
            />
          </div>

          <div>
            <label className="mb-1 block font-medium text-slate-300" htmlFor="fork-model">
              Override Model
            </label>
            <input
              id="fork-model"
              type="text"
              value={overrideModel}
              onChange={(e) => setOverrideModel(e.target.value)}
              className="w-full rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-slate-100 focus:border-violet-500 focus:outline-none"
            />
          </div>

          <label className="flex items-center gap-2 pt-1 text-slate-300">
            <input
              type="checkbox"
              checked={autoResume}
              onChange={(e) => setAutoResume(e.target.checked)}
              className="h-3.5 w-3.5 rounded border-slate-600 bg-slate-950 text-violet-500"
            />
            Immediately execute branched trajectory after forking
          </label>
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md border border-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-800"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={busy}
            className="rounded-md bg-violet-600 px-4 py-1.5 text-xs font-semibold text-white transition hover:bg-violet-500 disabled:opacity-50"
          >
            {busy ? "Forking..." : "Create Branch"}
          </button>
        </div>
      </form>
    </div>
  );
}
