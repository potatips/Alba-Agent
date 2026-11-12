import { Activity, ShieldCheck } from "lucide-react";
import type { AuditLogDto, RuntimeEvent } from "../types";

interface EventTimelineProps {
  events: RuntimeEvent[];
  auditLogs: AuditLogDto[];
  tab: "timeline" | "audit";
  onChangeTab: (tab: "timeline" | "audit") => void;
}

const badgeColor = (eventType: RuntimeEvent["type"]): string => {
  switch (eventType) {
    case "APPROVAL_REQUIRED":
      return "bg-amber-500/20 text-amber-300";
    case "APPROVAL_RESOLVED":
      return "bg-emerald-500/20 text-emerald-300";
    case "RUN_FAILED":
      return "bg-rose-500/20 text-rose-300";
    case "RUN_COMPLETED":
      return "bg-sky-500/20 text-sky-300";
    case "TRAJECTORY_FORKED":
      return "bg-violet-500/20 text-violet-300";
    case "SANDBOX_FILE_DIFF":
      return "bg-amber-500/15 text-amber-200";
    default:
      return "bg-slate-800 text-slate-300";
  }
};

export function EventTimeline({
  events,
  auditLogs,
  tab,
  onChangeTab,
}: EventTimelineProps) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between border-b border-slate-800 px-3 py-1.5">
        <div className="flex gap-1 text-[11px]">
          <button
            type="button"
            onClick={() => onChangeTab("timeline")}
            className={`flex items-center gap-1 rounded px-2 py-1 font-medium transition ${
              tab === "timeline"
                ? "bg-slate-800 text-white"
                : "text-slate-400 hover:text-slate-200"
            }`}
          >
            <Activity className="h-3.5 w-3.5 text-sky-400" />
            Timeline ({events.length})
          </button>
          <button
            type="button"
            onClick={() => onChangeTab("audit")}
            className={`flex items-center gap-1 rounded px-2 py-1 font-medium transition ${
              tab === "audit"
                ? "bg-slate-800 text-white"
                : "text-slate-400 hover:text-slate-200"
            }`}
          >
            <ShieldCheck className="h-3.5 w-3.5 text-emerald-400" />
            Audit ({auditLogs.length})
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {tab === "timeline" ? (
          events.length === 0 ? (
            <p className="text-xs text-slate-500">
              Runtime events stream here during execution.
            </p>
          ) : (
            <ul className="space-y-2">
              {events.map((ev) => (
                <li
                  key={ev.id}
                  className="rounded-md border border-slate-800 bg-slate-900/60 p-2 text-xs"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span
                      className={`rounded px-1.5 py-0.5 font-mono text-[10px] font-semibold ${badgeColor(ev.type)}`}
                    >
                      {ev.type}
                    </span>
                    <span className="text-[10px] text-slate-500">
                      {new Date(ev.occurred_at).toLocaleTimeString()}
                    </span>
                  </div>
                  <pre className="mt-1.5 max-h-24 overflow-auto font-mono text-[10px] text-slate-300">
                    {JSON.stringify(ev.payload, null, 2)}
                  </pre>
                </li>
              ))}
            </ul>
          )
        ) : auditLogs.length === 0 ? (
          <p className="text-xs text-slate-500">No immutable audit records for this run yet.</p>
        ) : (
          <ul className="space-y-2">
            {auditLogs.map((entry) => (
              <li
                key={entry.id}
                className="rounded-md border border-slate-800 bg-slate-900/60 p-2 text-xs"
              >
                <div className="flex items-center justify-between">
                  <span className="font-mono text-[11px] font-semibold text-emerald-300">
                    {entry.event_type}
                  </span>
                  <span className="text-[10px] text-slate-400">{entry.actor ?? "system"}</span>
                </div>
                <pre className="mt-1 overflow-auto font-mono text-[10px] text-slate-300">
                  {entry.details_json}
                </pre>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
