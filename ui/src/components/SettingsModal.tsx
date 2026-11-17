import { useState } from "react";
import {
  Briefcase,
  ChevronRight,
  Folder,
  MessageSquare,
  Wrench,
  X,
} from "lucide-react";

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  workspaceRoot: string;
  onChangeWorkspaceRoot: (value: string) => void;
  apiBase: string;
  onChangeApiBase: (value: string) => void;
  apiKey: string;
  onChangeApiKey: (value: string) => void;
  hasEnvApiKey: boolean;
  model: string;
  onChangeModel: (value: string) => void;
  authorId: string;
  onChangeAuthorId: (value: string) => void;
  approverId: string;
  onChangeApproverId: (value: string) => void;
  requireApprovalReads: boolean;
  onChangeRequireApprovalReads: (value: boolean) => void;
}

export function SettingsModal({
  open,
  onClose,
  workspaceRoot,
  onChangeWorkspaceRoot,
  apiBase,
  onChangeApiBase,
  apiKey,
  onChangeApiKey,
  hasEnvApiKey,
  model,
  onChangeModel,
  authorId,
  onChangeAuthorId,
  approverId,
  onChangeApproverId,
  requireApprovalReads,
  onChangeRequireApprovalReads,
}: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState<
    "general" | "application" | "appearance" | "shortcuts" | "notifications" | "models" | "browser"
  >("general");

  const [layoutMode, setLayoutMode] = useState<"knowledge" | "software">("software");
  const [localPermission, setLocalPermission] = useState<"Turbo" | "Ask for Approval" | "Read Only">("Turbo");
  const [remotePermission, setRemotePermission] = useState<"Turbo" | "Ask for Approval" | "Disabled">("Turbo");
  const [planReviewPolicy, setPlanReviewPolicy] = useState<"Always Proceed" | "Ask for Approval" | "Strict Review">("Always Proceed");
  const [queuedMessages, setQueuedMessages] = useState<"Queue" | "Send Immediately">("Send Immediately");

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-md"
    >
      <div className="flex h-[600px] w-full max-w-4xl overflow-hidden rounded-2xl border border-zinc-800 bg-[#0d0d10] text-zinc-100 shadow-2xl">
        <div className="flex w-52 shrink-0 flex-col border-r border-zinc-800/80 bg-[#0a0a0c] p-3 text-xs text-zinc-400">
          <div className="mb-1 px-2 py-1 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">
            Settings
          </div>
          <nav className="space-y-0.5">
            {[
              { id: "general", label: "General" },
              { id: "application", label: "Application" },
              { id: "appearance", label: "Appearance" },
              { id: "shortcuts", label: "Shortcuts" },
              { id: "notifications", label: "Notifications" },
              { id: "models", label: "Models" },
              { id: "browser", label: "Browser" },
            ].map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setActiveTab(item.id as typeof activeTab)}
                className={`flex w-full items-center rounded-lg px-2.5 py-1.5 text-left font-medium transition ${
                  activeTab === item.id
                    ? "bg-zinc-800/90 text-white"
                    : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
                }`}
              >
                {item.label}
              </button>
            ))}
          </nav>

          <div className="mt-4 border-t border-zinc-800/80 pt-3">
            <div className="mb-1 px-2 py-1 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">
              Projects
            </div>
            <div className="space-y-0.5">
              {["Create-LLM", "NexusAgent", "Website_Nexus", "Show All"].map((p) => (
                <button
                  key={p}
                  type="button"
                  className="flex w-full items-center gap-1.5 rounded-lg px-2.5 py-1 text-left text-zinc-400 transition hover:bg-zinc-900 hover:text-zinc-200"
                >
                  <Folder className="h-3 w-3 text-zinc-500" />
                  <span>{p}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="mt-4 border-t border-zinc-800/80 pt-3">
            <div className="mb-1 px-2 py-1 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">
              Not in Project
            </div>
            <button
              type="button"
              className="flex w-full items-center gap-1.5 rounded-lg px-2.5 py-1 text-left text-zinc-400 transition hover:bg-zinc-900 hover:text-zinc-200"
            >
              <MessageSquare className="h-3 w-3 text-zinc-500" />
              <span>Conversations</span>
            </button>
          </div>

          <div className="mt-auto border-t border-zinc-800/80 pt-3">
            <button
              type="button"
              className="mb-2 block w-full px-2 text-left text-[11px] text-zinc-500 transition hover:text-zinc-300"
            >
              Give Feedback
            </button>
            <div className="flex items-center gap-2 rounded-xl border border-zinc-800 bg-[#121215] p-2">
              <div className="flex h-7 w-7 items-center justify-center rounded-full bg-sky-700 text-xs font-bold text-white">
                P
              </div>
              <div className="truncate">
                <p className="truncate text-xs font-medium text-zinc-200">Phạm Khánh Như</p>
                <p className="truncate text-[10px] text-zinc-500">nhukhanh276@gmail.com</p>
              </div>
            </div>
          </div>
        </div>

        <div className="flex min-w-0 flex-1 flex-col overflow-y-auto p-6">
          <div className="flex items-start justify-between border-b border-zinc-800 pb-3">
            <div>
              <h2 className="text-base font-semibold capitalize text-zinc-100">
                {activeTab}
              </h2>
              <p className="mt-0.5 text-xs text-zinc-400">
                {activeTab === "general"
                  ? "Configure agent execution, queued message delivery, and permissions."
                  : activeTab === "models"
                    ? "Manage provider credentials, model selection and token budgets."
                    : activeTab === "browser"
                      ? "Control Alba browser automation, DevTools proxy and preview."
                      : "Configure environment settings and developer tools."}
              </p>
            </div>
            <button
              type="button"
              onClick={onClose}
              aria-label="Close Settings"
              className="rounded-lg p-1 text-zinc-400 hover:bg-zinc-800 hover:text-white"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {activeTab === "general" && (
            <div className="mt-5 space-y-6 text-xs">
              <div>
                <h3 className="mb-2 font-medium text-zinc-300">Interface Layout</h3>
                <div className="grid grid-cols-2 gap-3">
                  <div
                    onClick={() => setLayoutMode("knowledge")}
                    className={`cursor-pointer rounded-xl border p-3.5 transition ${
                      layoutMode === "knowledge"
                        ? "border-blue-500 bg-blue-950/20 ring-1 ring-blue-500"
                        : "border-zinc-800 bg-[#121215] hover:border-zinc-700"
                    }`}
                  >
                    <Briefcase className="h-4 w-4 text-zinc-400" />
                    <p className="mt-2 font-medium text-zinc-200">Knowledge Work</p>
                    <p className="mt-1 text-[11px] text-zinc-500">
                      Simplified interface for research, writing, and everyday tasks.
                    </p>
                  </div>

                  <div
                    onClick={() => setLayoutMode("software")}
                    className={`cursor-pointer rounded-xl border p-3.5 transition ${
                      layoutMode === "software"
                        ? "border-blue-500 bg-blue-950/20 ring-1 ring-blue-500"
                        : "border-zinc-800 bg-[#121215] hover:border-zinc-700"
                    }`}
                  >
                    <Wrench className="h-4 w-4 text-blue-400" />
                    <p className="mt-2 font-medium text-zinc-200">Software Engineering</p>
                    <p className="mt-1 text-[11px] text-zinc-500">
                      Full interface for coding, developer tools, and managing agents.
                    </p>
                  </div>
                </div>
              </div>

              <div className="border-t border-zinc-800/80 pt-5">
                <div className="mb-3">
                  <h3 className="font-medium text-zinc-300">Permissions</h3>
                  <p className="text-[11px] text-zinc-500">
                    Configure what the agent can do in local and remote conversations.{" "}
                    <span className="text-blue-400 underline">Learn more.</span>
                  </p>
                </div>

                <div className="space-y-3 rounded-xl border border-zinc-800 bg-[#121215] p-3.5">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="font-medium text-zinc-200">This Computer</p>
                      <p className="text-[11px] text-zinc-500">
                        Controls the actions the agent can take.
                      </p>
                    </div>
                    <select
                      value={localPermission}
                      onChange={(e) =>
                        setLocalPermission(e.target.value as typeof localPermission)
                      }
                      className="rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-1 text-xs text-zinc-200 focus:outline-none"
                    >
                      <option value="Turbo">Turbo</option>
                      <option value="Ask for Approval">Ask for Approval</option>
                      <option value="Read Only">Read Only</option>
                    </select>
                  </div>

                  <div className="flex items-center justify-between border-t border-zinc-800/60 pt-3">
                    <div>
                      <p className="font-medium text-zinc-200">Remote</p>
                      <p className="text-[11px] text-zinc-500">
                        Controls permissions for remote conversations.
                      </p>
                    </div>
                    <select
                      value={remotePermission}
                      onChange={(e) =>
                        setRemotePermission(e.target.value as typeof remotePermission)
                      }
                      className="rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-1 text-xs text-zinc-200 focus:outline-none"
                    >
                      <option value="Turbo">Turbo</option>
                      <option value="Ask for Approval">Ask for Approval</option>
                      <option value="Disabled">Disabled</option>
                    </select>
                  </div>

                  <div className="border-t border-zinc-800/60 pt-2">
                    <button
                      type="button"
                      className="flex items-center gap-1 text-[11px] font-medium text-zinc-400 hover:text-zinc-200"
                    >
                      <span>Advanced</span>
                      <ChevronRight className="h-3 w-3" />
                    </button>
                  </div>
                </div>
              </div>

              <div className="border-t border-zinc-800/80 pt-5">
                <h3 className="mb-2 font-medium text-zinc-300">Agent Behavior</h3>
                <div className="rounded-xl border border-zinc-800 bg-[#121215] p-3.5">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="font-medium text-zinc-200">Plan Review Policy</p>
                      <p className="text-[11px] text-zinc-500">
                        Whether the agent asks you to review its documents. Modified in swarm/swarm_board.
                      </p>
                      <p className="mt-1 font-mono text-[10px] text-zinc-400">
                        Type / and select plan to have the agent generate a plan.
                      </p>
                    </div>
                    <select
                      value={planReviewPolicy}
                      onChange={(e) =>
                        setPlanReviewPolicy(e.target.value as typeof planReviewPolicy)
                      }
                      className="rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-1 text-xs text-zinc-200 focus:outline-none"
                    >
                      <option value="Always Proceed">Always Proceed</option>
                      <option value="Ask for Approval">Ask for Approval</option>
                      <option value="Strict Review">Strict Review</option>
                    </select>
                  </div>
                </div>
              </div>

              <div className="border-t border-zinc-800/80 pt-5">
                <h3 className="mb-2 font-medium text-zinc-300">Execution</h3>
                <div className="flex items-center justify-between rounded-xl border border-zinc-800 bg-[#121215] p-3.5">
                  <div>
                    <p className="font-medium text-zinc-200">Queued Messages</p>
                    <p className="text-[11px] text-zinc-500">
                      Configure when follow-up messages are sent.
                    </p>
                  </div>
                  <div className="flex rounded-lg border border-zinc-700 bg-zinc-900 p-0.5">
                    <button
                      type="button"
                      onClick={() => setQueuedMessages("Queue")}
                      className={`rounded-md px-3 py-1 font-medium transition ${
                        queuedMessages === "Queue"
                          ? "bg-zinc-800 text-white"
                          : "text-zinc-400 hover:text-zinc-200"
                      }`}
                    >
                      Queue
                    </button>
                    <button
                      type="button"
                      onClick={() => setQueuedMessages("Send Immediately")}
                      className={`rounded-md px-3 py-1 font-medium transition ${
                        queuedMessages === "Send Immediately"
                          ? "bg-zinc-800 text-white"
                          : "text-zinc-400 hover:text-zinc-200"
                      }`}
                    >
                      Send Immediately
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {activeTab === "models" && (
            <div className="mt-5 space-y-4 text-xs">
              <div>
                <label className="mb-1 block font-medium text-zinc-300" htmlFor="cfg-model-picker">
                  Active Model
                </label>
                <select
                  id="cfg-model-picker"
                  value={model}
                  onChange={(e) => onChangeModel(e.target.value)}
                  className="w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 text-zinc-100 focus:outline-none"
                >
                  <option value="Gemini 3.8 Flash High">Gemini 3.8 Flash High</option>
                  <option value="gpt-4o-mini">gpt-4o-mini</option>
                  <option value="gpt-4o">gpt-4o</option>
                  <option value="claude-3-5-sonnet">claude-3-5-sonnet</option>
                  <option value="deepseek-r1">deepseek-r1</option>
                </select>
              </div>

              <div>
                <label className="mb-1 block font-medium text-zinc-300" htmlFor="cfg-api-base-input">
                  OpenAI-Compatible API Base
                </label>
                <input
                  id="cfg-api-base-input"
                  type="text"
                  value={apiBase}
                  onChange={(e) => onChangeApiBase(e.target.value)}
                  className="w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 font-mono text-zinc-100 focus:outline-none"
                />
              </div>

              <div>
                <label className="mb-1 block font-medium text-zinc-300" htmlFor="cfg-api-key-input">
                  API Key {hasEnvApiKey ? "(NEXUS_API_KEY detected in env)" : ""}
                </label>
                <input
                  id="cfg-api-key-input"
                  type="password"
                  value={apiKey}
                  onChange={(e) => onChangeApiKey(e.target.value)}
                  placeholder={hasEnvApiKey ? "Using NEXUS_API_KEY from environment" : "sk-..."}
                  className="w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 font-mono text-zinc-100 focus:outline-none"
                />
              </div>
            </div>
          )}

          {activeTab === "browser" && (
            <div className="mt-5 space-y-4 text-xs">
              <div className="rounded-xl border border-zinc-800 bg-[#121215] p-3.5">
                <p className="font-medium text-zinc-200">Alba Browser Automation</p>
                <p className="mt-1 text-[11px] text-zinc-400">
                  Enables live DevTools browser control, responsive layout inspections, and local server page interactions.
                </p>
                <div className="mt-3 flex items-center justify-between border-t border-zinc-800 pt-3">
                  <span>Remote Debugging Port</span>
                  <span className="font-mono text-blue-400">9222</span>
                </div>
                <div className="mt-2 flex items-center justify-between border-t border-zinc-800 pt-2">
                  <span>Auto-Open Dev Server</span>
                  <span className="font-mono text-emerald-400">Enabled (port 1420)</span>
                </div>
              </div>
            </div>
          )}

          {activeTab === "application" && (
            <div className="mt-5 space-y-4 text-xs">
              <div>
                <label className="mb-1 block font-medium text-zinc-300" htmlFor="cfg-ws-root-input">
                  Workspace Root
                </label>
                <input
                  id="cfg-ws-root-input"
                  type="text"
                  value={workspaceRoot}
                  onChange={(e) => onChangeWorkspaceRoot(e.target.value)}
                  className="w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 font-mono text-zinc-100 focus:outline-none"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="mb-1 block font-medium text-zinc-300" htmlFor="cfg-author-input">
                    Author ID
                  </label>
                  <input
                    id="cfg-author-input"
                    type="text"
                    value={authorId}
                    onChange={(e) => onChangeAuthorId(e.target.value)}
                    className="w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 text-zinc-100 focus:outline-none"
                  />
                </div>
                <div>
                  <label className="mb-1 block font-medium text-zinc-300" htmlFor="cfg-approver-input">
                    Approver ID (Separation of Duties)
                  </label>
                  <input
                    id="cfg-approver-input"
                    type="text"
                    value={approverId}
                    onChange={(e) => onChangeApproverId(e.target.value)}
                    className="w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 text-zinc-100 focus:outline-none"
                  />
                </div>
              </div>

              <label className="flex items-center gap-2 pt-2 text-zinc-300">
                <input
                  type="checkbox"
                  checked={requireApprovalReads}
                  onChange={(e) => onChangeRequireApprovalReads(e.target.checked)}
                />
                Require human approval on read_file operations
              </label>
            </div>
          )}

          <div className="mt-auto flex justify-end border-t border-zinc-800 pt-4">
            <button
              type="button"
              onClick={onClose}
              className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-semibold text-white transition hover:bg-blue-500"
            >
              Done
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
