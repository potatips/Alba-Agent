import { Check, ShieldAlert, X } from "lucide-react";
import type { PendingApprovalDto } from "../types";

interface ApprovalCardProps {
  approval: PendingApprovalDto;
  approverId: string;
  busy: boolean;
  onChangeApproverId: (value: string) => void;
  onAllowOnce: () => void;
  onDeny: () => void;
}

export function ApprovalCard({
  approval,
  approverId,
  busy,
  onChangeApproverId,
  onAllowOnce,
  onDeny,
}: ApprovalCardProps) {
  const selfApproval =
    Boolean(approval.author_id) && approval.author_id === approverId.trim();

  return (
    <div
      className="rounded-lg border border-amber-500/50 bg-amber-950/20 p-3 shadow-lg"
      role="region"
      aria-label="Human Approval Card"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2">
          <ShieldAlert className="h-4 w-4 shrink-0 text-amber-400" />
          <div>
            <p className="text-xs font-semibold text-amber-200">
              Approval Required: <span className="font-mono">{approval.tool_name}</span>
            </p>
            <p className="text-[11px] text-slate-300">{approval.action_description}</p>
          </div>
        </div>
        <span className="rounded bg-amber-500/20 px-1.5 py-0.5 font-mono text-[10px] text-amber-300 uppercase">
          {approval.status}
        </span>
      </div>

      <div className="mt-2 space-y-1 rounded bg-slate-950/90 p-2 font-mono text-[11px] text-slate-300">
        {approval.path && (
          <div>
            <span className="text-slate-500">path: </span>
            <span className="text-sky-300">{approval.path}</span>
          </div>
        )}
        {approval.program && (
          <div>
            <span className="text-slate-500">command: </span>
            <span className="text-emerald-300">
              {approval.program} {(approval.args ?? []).join(" ")}
            </span>
          </div>
        )}
        {approval.author_id && (
          <div>
            <span className="text-slate-500">author: </span>
            <span>{approval.author_id}</span>
          </div>
        )}
      </div>

      {approval.diff && (
        <pre className="mt-2 max-h-40 overflow-auto rounded border border-slate-800 bg-slate-950 p-2 font-mono text-[11px] leading-relaxed text-slate-200">
          {approval.diff}
        </pre>
      )}

      <div className="mt-2.5">
        <label className="mb-1 block text-[11px] text-slate-400" htmlFor="approver-id-input">
          Approver Identity (must differ from author)
        </label>
        <input
          id="approver-id-input"
          type="text"
          value={approverId}
          onChange={(e) => onChangeApproverId(e.target.value)}
          className="w-full rounded border border-slate-700 bg-slate-950 px-2.5 py-1 text-xs text-slate-100 focus:border-amber-400 focus:outline-none"
        />
        {selfApproval && (
          <p className="mt-1 text-[11px] text-rose-400">
            Separation of duties forbids approving your own run ({approval.author_id}).
          </p>
        )}
      </div>

      <div className="mt-3 flex gap-2">
        <button
          type="button"
          disabled={busy || selfApproval || !approverId.trim()}
          onClick={onAllowOnce}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-45"
        >
          <Check className="h-3.5 w-3.5" />
          Allow Once
        </button>
        <button
          type="button"
          disabled={busy || !approverId.trim()}
          onClick={onDeny}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-rose-600 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-rose-500 disabled:cursor-not-allowed disabled:opacity-45"
        >
          <X className="h-3.5 w-3.5" />
          Deny
        </button>
      </div>
    </div>
  );
}
