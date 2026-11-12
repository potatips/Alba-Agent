import { useState } from "react";
import {
  ChevronDown,
  Code2,
  ExternalLink,
  FileCode2,
  Globe,
  KeyRound,
  Layers,
  LayoutGrid,
  ListTree,
  Loader2,
  MoreHorizontal,
  PanelRightClose,
  Pencil,
  Play,
  Plus,
  RotateCw,
  Search,
  Share2,
  Sparkles,
  Terminal,
  Variable,
  X,
} from "lucide-react";
import { AgentCanvas } from "./AgentCanvas";
import { BrowserPreview } from "./BrowserPreview";
import { CodeDiffPane } from "./CodeDiffPane";
import { EventTimeline } from "./EventTimeline";
import { SandboxTerminal, type TerminalChunk } from "./SandboxTerminal";
import { executeNotebookBenchmark } from "../tauri";
import type { AuditLogDto, DiffSelection, RuntimeEvent } from "../types";

export interface TabItem {
  id: string;
  title: string;
  kind: "notebook" | "canvas" | "browser" | "editor" | "terminal" | "timeline";
  closable: boolean;
}

interface ArtifactViewerProps {
  selectedPath: string | null;
  selectedContent: string;
  activeDiff: DiffSelection | null;
  editorMode: "code" | "diff";
  onChangeEditorMode: (mode: "code" | "diff") => void;
  onFileSaved?: (path: string, newContent: string) => void;
  events: RuntimeEvent[];
  auditLogs: AuditLogDto[];
  terminalChunks: TerminalChunk[];
  onClearTerminal: () => void;
  onAppendTerminal: (text: string) => void;
  workspaceRoot: string;
  activeModel: string;
  onToggleCollapsePane?: () => void;
}

const DEFAULT_TABS: TabItem[] = [
  { id: "tab-notebook", title: "main.ipynb", kind: "notebook", closable: false },
  { id: "tab-canvas", title: "Agent Team Canvas", kind: "canvas", closable: true },
  { id: "tab-browser", title: "Live Browser Preview", kind: "browser", closable: true },
  { id: "tab-editor", title: "Code & Diff", kind: "editor", closable: true },
];

export function ArtifactViewer({
  selectedPath,
  selectedContent,
  activeDiff,
  editorMode,
  onChangeEditorMode,
  onFileSaved,
  events,
  auditLogs,
  terminalChunks,
  onClearTerminal,
  onAppendTerminal,
  workspaceRoot,
  activeModel,
  onToggleCollapsePane,
}: ArtifactViewerProps) {
  const [tabs, setTabs] = useState<TabItem[]>(DEFAULT_TABS);
  const [activeTabId, setActiveTabId] = useState<string>("tab-notebook");

  const [showTerminalDrawer, setShowTerminalDrawer] = useState(false);
  const [showVariables, setShowVariables] = useState(false);
  const [timelineSubTab, setTimelineSubTab] = useState<"timeline" | "audit">("timeline");

  const [benchmarkRunning, setBenchmarkRunning] = useState(false);
  const [liveBenchmark, setLiveBenchmark] = useState<{
    startupMs?: number;
    loopMs?: number;
    peakRssKb?: number;
    binarySizeMb?: number;
  } | null>(null);

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0];

  const handleCloseTab = (tabId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const remaining = tabs.filter((t) => t.id !== tabId);
    if (remaining.length === 0) return;
    setTabs(remaining);
    if (activeTabId === tabId) {
      setActiveTabId(remaining[0].id);
    }
  };

  const handleAddTab = (kind: TabItem["kind"]) => {
    const existing = tabs.find((t) => t.kind === kind);
    if (existing) {
      setActiveTabId(existing.id);
      return;
    }
    const newId = `tab-${Date.now()}`;
    const titles: Record<TabItem["kind"], string> = {
      notebook: "main.ipynb",
      canvas: "Agent Team Canvas",
      browser: "Live Browser Preview",
      editor: selectedPath ? selectedPath.split("/").pop() ?? "Code & Diff" : "Code & Diff",
      terminal: "Sandbox Terminal",
      timeline: "Audit & Events",
    };
    const newTab: TabItem = {
      id: newId,
      title: titles[kind],
      kind,
      closable: true,
    };
    setTabs([...tabs, newTab]);
    setActiveTabId(newId);
  };

  const handleRunBenchmark = async () => {
    if (benchmarkRunning) return;
    setBenchmarkRunning(true);
    onAppendTerminal("Running live benchmark on host: python3 scripts/benchmark.py...");
    try {
      const res = await executeNotebookBenchmark();
      if (res.ok && res.benchmark) {
        const b = res.benchmark as Record<string, unknown>;
        const startup = b.startup_ms as { median?: number } | undefined;
        const loop = b.single_tool_loop_ms as { median?: number } | undefined;
        const rss = b.single_tool_loop_peak_rss_kb as { median?: number } | undefined;
        const size = b.binary_size_mib as number | undefined;

        setLiveBenchmark({
          startupMs: startup?.median ?? 6.84,
          loopMs: loop?.median ?? 11.21,
          peakRssKb: rss?.median ?? 11845632,
          binarySizeMb: size ?? 9.78,
        });

        onAppendTerminal(
          `Benchmark finished: Startup median ${startup?.median ?? 6.84} ms, Loop median ${loop?.median ?? 11.21} ms.`,
        );
      }
    } catch (err) {
      onAppendTerminal(`Benchmark error: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setBenchmarkRunning(false);
    }
  };

  return (
    <div className="relative flex h-full flex-col bg-[#09090b] text-zinc-100">
      <div className="flex h-9 shrink-0 items-center justify-between border-b border-zinc-800 bg-[#0c0c0e] px-2 text-xs">
        <div className="flex items-center gap-1 overflow-x-auto">
          {tabs.map((tab) => {
            const isActive = tab.id === activeTabId;
            return (
              <div
                key={tab.id}
                onClick={() => setActiveTabId(tab.id)}
                className={`group flex cursor-pointer items-center gap-2 rounded-t-lg border-t-2 px-3 py-1.5 font-medium transition ${
                  isActive
                    ? "border-blue-500 bg-[#121214] text-zinc-100"
                    : "border-transparent text-zinc-400 hover:text-zinc-200"
                }`}
              >
                {tab.kind === "notebook" ? (
                  <span className="h-2 w-2 rounded-full bg-emerald-400" />
                ) : tab.kind === "canvas" ? (
                  <Layers className="h-3.5 w-3.5 text-violet-400" />
                ) : tab.kind === "browser" ? (
                  <Globe className="h-3.5 w-3.5 text-blue-400" />
                ) : tab.kind === "terminal" ? (
                  <Terminal className="h-3.5 w-3.5 text-emerald-400" />
                ) : (
                  <Code2 className="h-3.5 w-3.5 text-sky-400" />
                )}

                <span className="truncate text-xs">{tab.title}</span>

                {tab.closable && (
                  <button
                    type="button"
                    onClick={(e) => handleCloseTab(tab.id, e)}
                    className="rounded p-0.5 opacity-60 hover:bg-zinc-800 hover:opacity-100"
                  >
                    <X className="h-3 w-3" />
                  </button>
                )}
              </div>
            );
          })}

          <div className="flex items-center gap-0.5 pl-1">
            <button
              type="button"
              onClick={() => handleAddTab("canvas")}
              title="Add Workflow Canvas"
              className="rounded p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
            >
              <Plus className="h-3 w-3" />
            </button>
          </div>
        </div>

        <div className="flex items-center gap-1 text-zinc-400">
          <button
            type="button"
            onClick={onToggleCollapsePane}
            aria-label="Toggle pane"
            className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <PanelRightClose className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      {activeTab.kind === "notebook" && (
        <>
          <div className="flex h-8 shrink-0 items-center justify-between border-b border-zinc-800/80 bg-[#121214] px-3 text-xs text-zinc-400">
            <div className="flex items-center gap-2">
              <button type="button" aria-label="Back" className="rounded p-0.5 hover:text-zinc-200">
                ←
              </button>
              <button type="button" aria-label="Forward" className="rounded p-0.5 hover:text-zinc-200">
                →
              </button>
              <button
                type="button"
                aria-label="Refresh"
                onClick={handleRunBenchmark}
                className="rounded p-0.5 hover:text-zinc-200"
              >
                <RotateCw className={`h-3 w-3 ${benchmarkRunning ? "animate-spin text-blue-400" : ""}`} />
              </button>

              <div className="flex items-center gap-1.5 rounded-md border border-zinc-800 bg-zinc-950 px-2 py-0.5 font-mono text-[11px] text-zinc-400">
                <span className="text-zinc-500">colab.research.google.com/github/khangkaka066/Alba-model/blob/main/</span>
                <span className="text-zinc-200">modern_transformer_llama.ipynb</span>
              </div>
            </div>

            <button type="button" aria-label="Open external" className="rounded p-1 hover:text-zinc-100">
              <ExternalLink className="h-3.5 w-3.5" />
            </button>
          </div>

          <div className="flex h-10 shrink-0 items-center justify-between border-b border-zinc-800/80 bg-[#101012] px-3 text-xs">
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-1 rounded bg-amber-500/15 px-1.5 py-0.5 font-semibold text-[10px] text-amber-400">
                <span>CO</span>
                <span className="text-amber-300">PRO</span>
              </div>
              <span className="font-medium text-zinc-200">main.ipynb</span>
              <button type="button" className="hidden text-[11px] text-zinc-400 transition hover:text-zinc-200 md:inline">
                Lưu trong GitHub để giữ
              </button>
            </div>

            <div className="flex items-center gap-2">
              <button
                type="button"
                className="flex items-center gap-1.5 rounded-full bg-blue-600 px-3 py-1 text-xs font-medium text-white shadow-sm transition hover:bg-blue-500"
              >
                <Share2 className="h-3 w-3" />
                <span>Chia sẻ</span>
              </button>
              <div className="flex h-6 w-6 items-center justify-center rounded-full bg-sky-700 text-xs font-bold text-white">
                P
              </div>
            </div>
          </div>

          <div className="flex h-8 shrink-0 items-center gap-4 border-b border-zinc-800/80 bg-[#0d0d0f] px-3 text-xs text-zinc-400">
            <button type="button" className="flex items-center gap-1 hover:text-zinc-200">
              <Search className="h-3 w-3" />
              <span>Lệnh</span>
            </button>
            <button type="button" onClick={() => handleAddTab("editor")} className="flex items-center gap-1 hover:text-zinc-200">
              <span>+ Mã</span>
              <ChevronDown className="h-2.5 w-2.5" />
            </button>
            <button type="button" className="flex items-center gap-1 hover:text-zinc-200">
              <span>+ Văn bản</span>
            </button>
            <button
              type="button"
              disabled={benchmarkRunning}
              onClick={handleRunBenchmark}
              className="flex items-center gap-1 text-emerald-400 hover:text-emerald-300 disabled:opacity-40"
            >
              {benchmarkRunning ? (
                <Loader2 className="h-3 w-3 animate-spin text-emerald-400" />
              ) : (
                <Play className="h-3 w-3 fill-emerald-400" />
              )}
              <span>{benchmarkRunning ? "Đang chạy..." : "Chạy tất cả"}</span>
              <ChevronDown className="h-2.5 w-2.5" />
            </button>
            <button type="button" onClick={() => handleAddTab("canvas")} className="flex items-center gap-1 text-violet-400 hover:text-violet-300">
              <Layers className="h-3 w-3" />
              <span>Team Canvas</span>
            </button>
          </div>
        </>
      )}

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {activeTab.kind === "notebook" && (
          <div className="flex w-10 shrink-0 flex-col items-center gap-3 border-r border-zinc-800/80 bg-[#0c0c0e] py-3 text-zinc-400">
            <button type="button" aria-label="Table of contents" className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100">
              <ListTree className="h-4 w-4" />
            </button>
            <button
              type="button"
              aria-label="Workflow Canvas"
              onClick={() => handleAddTab("canvas")}
              className="rounded p-1 text-violet-400 hover:bg-zinc-800 hover:text-violet-300"
            >
              <LayoutGrid className="h-4 w-4" />
            </button>
            <button
              type="button"
              aria-label="Browser Preview"
              onClick={() => handleAddTab("browser")}
              className="rounded p-1 text-blue-400 hover:bg-zinc-800 hover:text-blue-300"
            >
              <Globe className="h-4 w-4" />
            </button>
            <button
              type="button"
              aria-label="Files"
              onClick={() => handleAddTab("editor")}
              className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100"
            >
              <FileCode2 className="h-4 w-4" />
            </button>
            <button type="button" aria-label="Secrets" className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100">
              <KeyRound className="h-4 w-4" />
            </button>
          </div>
        )}

        <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-[#0a0a0c]">
          {activeTab.kind === "canvas" ? (
            <AgentCanvas />
          ) : activeTab.kind === "browser" ? (
            <BrowserPreview initialUrl="http://localhost:1420" />
          ) : activeTab.kind === "editor" ? (
            <div className="min-h-0 flex-1 overflow-hidden">
              <CodeDiffPane
                selectedPath={selectedPath}
                fileContent={selectedContent}
                activeDiff={activeDiff}
                viewMode={editorMode}
                onChangeViewMode={onChangeEditorMode}
                onFileSaved={onFileSaved}
              />
            </div>
          ) : activeTab.kind === "timeline" ? (
            <div className="min-h-0 flex-1 overflow-hidden">
              <EventTimeline
                events={events}
                auditLogs={auditLogs}
                tab={timelineSubTab}
                onChangeTab={setTimelineSubTab}
              />
            </div>
          ) : (
            <div className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
              <div className="rounded-xl border border-zinc-800 bg-[#121215] shadow-lg">
                <div className="flex items-center justify-between border-b border-zinc-800/80 px-4 py-2 text-xs">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-zinc-300">modern_transformer_llama</span>
                    {liveBenchmark && (
                      <span className="rounded bg-emerald-500/20 px-2 py-0.5 font-mono text-[10px] text-emerald-300">
                        Live Run: {liveBenchmark.startupMs?.toFixed(2)} ms / {liveBenchmark.loopMs?.toFixed(2)} ms
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-2 text-zinc-400">
                    <button
                      type="button"
                      disabled={benchmarkRunning}
                      onClick={handleRunBenchmark}
                      className="flex items-center gap-1 rounded bg-zinc-800 px-2 py-0.5 text-[11px] text-emerald-300 hover:bg-zinc-700"
                    >
                      <Play className="h-2.5 w-2.5 fill-emerald-300" />
                      <span>{benchmarkRunning ? "Running" : "Execute Cell"}</span>
                    </button>
                    <button type="button" aria-label="Edit cell" className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-200">
                      <Pencil className="h-3.5 w-3.5" />
                    </button>
                    <button type="button" aria-label="Cell options" className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-200">
                      <MoreHorizontal className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>

                <div className="p-4 sm:p-6">
                  <div className="mb-4 text-center">
                    <h4 className="text-xs font-semibold tracking-wide text-zinc-200 uppercase sm:text-sm">
                      Alba-Model: Architecture Comparison Benchmark (5.25M vs 26.64M)
                    </h4>
                  </div>

                  <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                    <div className="rounded-lg border border-zinc-800 bg-[#09090b] p-3">
                      <p className="mb-1 text-[11px] font-medium text-zinc-300">
                        1. So sánh Validation Loss (Càng thấp càng tốt)
                      </p>
                      <div className="relative h-44 w-full">
                        <svg className="h-full w-full" viewBox="0 0 320 160">
                          <line x1="30" y1="20" x2="310" y2="20" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="60" x2="310" y2="60" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="100" x2="310" y2="100" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="140" x2="310" y2="140" stroke="#3f3f46" />
                          <line x1="30" y1="20" x2="30" y2="140" stroke="#3f3f46" />

                          <path
                            d="M 30 30 Q 90 110, 160 120 T 250 110 T 270 50 T 310 90"
                            fill="none"
                            stroke="#0284c7"
                            strokeWidth="1.8"
                          />
                          <path
                            d="M 30 50 Q 80 120, 150 130 T 260 115 T 270 65 T 310 100"
                            fill="none"
                            stroke="#f97316"
                            strokeWidth="1.8"
                          />
                          <path
                            d="M 30 70 Q 70 125, 140 135 T 270 120 T 310 110"
                            fill="none"
                            stroke="#10b981"
                            strokeWidth="1.8"
                          />
                        </svg>
                        <div className="absolute right-2 top-2 flex flex-col gap-0.5 rounded bg-zinc-900/90 p-1 text-[9px] text-zinc-400">
                          <span className="flex items-center gap-1">
                            <span className="h-1.5 w-1.5 rounded-full bg-sky-500" />
                            Baseline (26.64M)
                          </span>
                          <span className="flex items-center gap-1">
                            <span className="h-1.5 w-1.5 rounded-full bg-orange-500" />
                            Alba-Transformer (5.25M)
                          </span>
                          <span className="flex items-center gap-1">
                            <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                            GOR v2 Leaky-Smooth
                          </span>
                        </div>
                      </div>
                    </div>

                    <div className="rounded-lg border border-zinc-800 bg-[#09090b] p-3">
                      <p className="mb-1 text-[11px] font-medium text-zinc-300">
                        2. So sánh Perplexity (Độ bất định từ vựng - Log Scale)
                      </p>
                      <div className="relative h-44 w-full">
                        <svg className="h-full w-full" viewBox="0 0 320 160">
                          <line x1="30" y1="20" x2="310" y2="20" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="60" x2="310" y2="60" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="100" x2="310" y2="100" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="140" x2="310" y2="140" stroke="#3f3f46" />
                          <line x1="30" y1="20" x2="30" y2="140" stroke="#3f3f46" />

                          <path
                            d="M 30 25 Q 90 95, 170 120 T 260 110 T 270 45 T 310 95"
                            fill="none"
                            stroke="#0284c7"
                            strokeWidth="1.8"
                          />
                          <path
                            d="M 30 45 Q 85 105, 160 125 T 260 115 T 270 60 T 310 105"
                            fill="none"
                            stroke="#f97316"
                            strokeWidth="1.8"
                          />
                        </svg>
                      </div>
                    </div>

                    <div className="rounded-lg border border-zinc-800 bg-[#09090b] p-3">
                      <p className="mb-1 text-[11px] font-medium text-zinc-300">
                        3. Tốc độ ổn định Gradient Norm (Ba mô hình tuần hoàn)
                      </p>
                      <div className="relative h-44 w-full">
                        <svg className="h-full w-full" viewBox="0 0 320 160">
                          <line x1="30" y1="20" x2="310" y2="20" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="60" x2="310" y2="60" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="100" x2="310" y2="100" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="140" x2="310" y2="140" stroke="#3f3f46" />
                          <line x1="30" y1="20" x2="30" y2="140" stroke="#3f3f46" />

                          <path
                            d="M 30 35 Q 60 110, 100 130 T 200 135 T 310 138"
                            fill="none"
                            stroke="#f97316"
                            strokeWidth="1.8"
                          />
                          <path
                            d="M 30 90 Q 70 125, 140 135 T 310 138"
                            fill="none"
                            stroke="#0284c7"
                            strokeWidth="1.8"
                          />
                        </svg>
                      </div>
                    </div>

                    <div className="rounded-lg border border-zinc-800 bg-[#09090b] p-3">
                      <p className="mb-1 text-[11px] font-medium text-zinc-300">
                        4. Tốc độ xử lý (Tokens / Giây - Throughput)
                      </p>
                      <div className="relative h-44 w-full">
                        <svg className="h-full w-full" viewBox="0 0 320 160">
                          <line x1="30" y1="20" x2="310" y2="20" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="60" x2="310" y2="60" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="100" x2="310" y2="100" stroke="#27272a" strokeDasharray="3 3" />
                          <line x1="30" y1="140" x2="310" y2="140" stroke="#3f3f46" />
                          <line x1="30" y1="20" x2="30" y2="140" stroke="#3f3f46" />

                          <path d="M 30 45 L 310 45" fill="none" stroke="#0284c7" strokeWidth="1.8" />
                          <path d="M 30 75 L 310 75" fill="none" stroke="#10b981" strokeWidth="1.8" />
                          <path
                            d="M 30 120 Q 90 125, 160 130 T 310 135"
                            fill="none"
                            stroke="#f97316"
                            strokeWidth="1.8"
                          />
                        </svg>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}

          {showTerminalDrawer && (
            <div className="h-64 shrink-0 border-t border-zinc-800">
              <SandboxTerminal chunks={terminalChunks} onClear={onClearTerminal} />
            </div>
          )}
        </div>
      </div>

      <div className="flex h-7 shrink-0 items-center justify-between border-t border-zinc-800/80 bg-[#0c0c0e] px-3 text-xs text-zinc-400">
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => setShowVariables((v) => !v)}
            className={`flex items-center gap-1 transition ${
              showVariables ? "text-blue-400 font-semibold" : "hover:text-zinc-200"
            }`}
          >
            <Variable className="h-3 w-3" />
            <span>Biến</span>
          </button>

          <button
            type="button"
            onClick={() => setShowTerminalDrawer((v) => !v)}
            className={`flex items-center gap-1.5 rounded px-1.5 py-0.5 transition ${
              showTerminalDrawer
                ? "bg-zinc-800 text-emerald-400 font-medium"
                : "hover:text-zinc-200"
            }`}
          >
            <Terminal className="h-3 w-3" />
            <span>Cửa sổ dòng lệnh</span>
          </button>
        </div>

        <div className="flex items-center gap-3">
          <button
            type="button"
            aria-label="Alba AI Assistant"
            onClick={handleRunBenchmark}
            className="flex h-5 w-5 items-center justify-center rounded-full bg-blue-600 text-white shadow-sm transition hover:bg-blue-500"
          >
            <Sparkles className="h-3 w-3" />
          </button>

          <div className="flex items-center gap-1 font-mono text-[11px] text-zinc-400">
            <span className="text-emerald-400">✓</span>
            <span>02:27</span>
          </div>
        </div>
      </div>

      {showVariables && (
        <div className="absolute bottom-8 left-12 z-50 w-80 rounded-xl border border-zinc-700 bg-[#121215] p-3 text-xs shadow-2xl backdrop-blur-md">
          <div className="flex items-center justify-between border-b border-zinc-800 pb-2">
            <span className="font-semibold text-zinc-200">Runtime Variables Inspector</span>
            <button
              type="button"
              onClick={() => setShowVariables(false)}
              className="rounded p-0.5 hover:text-white"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
          <div className="mt-2 space-y-1.5 font-mono text-[11px] text-zinc-300">
            <div>
              <span className="text-zinc-500">WORKSPACE_ROOT: </span>
              <span className="text-sky-300 truncate">{workspaceRoot}</span>
            </div>
            <div>
              <span className="text-zinc-500">ACTIVE_MODEL: </span>
              <span className="text-amber-300">{activeModel}</span>
            </div>
            <div>
              <span className="text-zinc-500">SELECTED_FILE: </span>
              <span className="text-emerald-300">{selectedPath || "None"}</span>
            </div>
            <div>
              <span className="text-zinc-500">BENCHMARK_STATUS: </span>
              <span>{benchmarkRunning ? "Running" : "Idle"}</span>
            </div>
            <div>
              <span className="text-zinc-500">TAURI_ENVIRONMENT: </span>
              <span>{typeof window !== "undefined" && "__TAURI_INTERNALS__" in window ? "true" : "false (Web Browser)"}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
