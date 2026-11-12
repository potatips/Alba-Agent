import { useState } from "react";
import {
  Bot,
  Layers,
  Play,
  ShieldAlert,
  Terminal,
  User,
} from "lucide-react";

export interface AgentNode {
  id: string;
  name: string;
  role: string;
  model: string;
  status: "idle" | "running" | "completed" | "waiting_approval";
  x: number;
  y: number;
  temperature: number;
  sop: string;
  lastThought?: string;
}

const INITIAL_NODES: AgentNode[] = [
  {
    id: "node-trigger",
    name: "User Trigger",
    role: "Task Input & Event Trigger",
    model: "NexusFlow v2",
    status: "completed",
    x: 40,
    y: 120,
    temperature: 0.0,
    sop: "Receives natural language requirement, extracts constraints and triggers the pipeline.",
    lastThought: "Triggered from user session with prompt: 'Fix failing tests & verify GOR v2'",
  },
  {
    id: "node-architect",
    name: "Meta-Architect Agent",
    role: "Pillar 6: Orchestration & SOP",
    model: "Gemini 3.8 Flash High",
    status: "completed",
    x: 280,
    y: 50,
    temperature: 0.2,
    sop: "Decomposes requirements into parallel subtasks and selects optimal models.",
    lastThought: "Decomposed task into (1) Code generation, (2) Security audit, (3) Sandbox verification.",
  },
  {
    id: "node-coder",
    name: "Coder Agent",
    role: "Code Implementation & Patching",
    model: "claude-3-5-sonnet",
    status: "running",
    x: 520,
    y: 40,
    temperature: 0.3,
    sop: "Produces strict typed Rust and TypeScript implementations without placeholder stubs.",
    lastThought: "Implementing Smooth C∞ GOR v2 formulation in src/agent/gor_v2.rs...",
  },
  {
    id: "node-reviewer",
    name: "Reviewer & Auditor Agent",
    role: "Security & Licensing Guard",
    model: "gpt-4o",
    status: "idle",
    x: 520,
    y: 220,
    temperature: 0.1,
    sop: "Validates license allowlists, checks zero comments rule, and prevents prompt injection.",
    lastThought: "Waiting for Coder Agent diff output to scan licenses and no-comments compliance.",
  },
  {
    id: "node-sandbox",
    name: "Isolated Sandbox Executor",
    role: "Seatbelt / Docker Sandbox",
    model: "Local Seatbelt Runner",
    status: "idle",
    x: 760,
    y: 60,
    temperature: 0.0,
    sop: "Executes test suites and benchmarks inside an unshared filesystem container.",
    lastThought: "Pre-warmed Seatbelt profile ready for `cargo test --workspace` invocation.",
  },
  {
    id: "node-approval",
    name: "Human Approval Gate",
    role: "Separation of Duties Gate",
    model: "Author != Approver Gate",
    status: "waiting_approval",
    x: 760,
    y: 220,
    temperature: 0.0,
    sop: "Enforces single-use atomic allow_once token before committing changes to disk.",
    lastThought: "Pending approval for write_file operation on tests/phase1_phase2_tests.rs.",
  },
];

interface Edge {
  from: string;
  to: string;
}

const EDGES: Edge[] = [
  { from: "node-trigger", to: "node-architect" },
  { from: "node-architect", to: "node-coder" },
  { from: "node-architect", to: "node-reviewer" },
  { from: "node-coder", to: "node-sandbox" },
  { from: "node-reviewer", to: "node-approval" },
  { from: "node-sandbox", to: "node-approval" },
];

export function AgentCanvas() {
  const [nodes, setNodes] = useState<AgentNode[]>(INITIAL_NODES);
  const [selectedNodeId, setSelectedNodeId] = useState<string>("node-coder");
  const [isSimulating, setIsSimulating] = useState(false);
  const [zoom, setZoom] = useState(1);

  const selectedNode = nodes.find((n) => n.id === selectedNodeId) ?? nodes[0];

  const handleRunSimulation = () => {
    setIsSimulating(true);
    setNodes((prev) =>
      prev.map((n) =>
        n.id === "node-trigger"
          ? { ...n, status: "completed" }
          : n.id === "node-architect"
            ? { ...n, status: "running" }
            : { ...n, status: "idle" },
      ),
    );

    setTimeout(() => {
      setNodes((prev) =>
        prev.map((n) =>
          n.id === "node-architect"
            ? { ...n, status: "completed" }
            : n.id === "node-coder"
              ? { ...n, status: "running" }
              : n,
        ),
      );
    }, 1200);

    setTimeout(() => {
      setNodes((prev) =>
        prev.map((n) =>
          n.id === "node-coder"
            ? { ...n, status: "completed" }
            : n.id === "node-sandbox" || n.id === "node-reviewer"
              ? { ...n, status: "running" }
              : n,
        ),
      );
    }, 2500);

    setTimeout(() => {
      setNodes((prev) =>
        prev.map((n) =>
          n.id === "node-sandbox" || n.id === "node-reviewer"
            ? { ...n, status: "completed" }
            : n.id === "node-approval"
              ? { ...n, status: "waiting_approval" }
              : n,
        ),
      );
      setIsSimulating(false);
    }, 3800);
  };

  const handleUpdateSelectedNode = (patch: Partial<AgentNode>) => {
    setNodes((prev) =>
      prev.map((n) => (n.id === selectedNodeId ? { ...n, ...patch } : n)),
    );
  };

  return (
    <div className="flex h-full w-full flex-col bg-[#08080a] text-zinc-100">
      <div className="flex h-10 shrink-0 items-center justify-between border-b border-zinc-800 bg-[#0c0c0e] px-4 text-xs">
        <div className="flex items-center gap-2">
          <Layers className="h-4 w-4 text-violet-400" />
          <span className="font-semibold tracking-wide text-zinc-200">
            NexusAgent Workflow Studio (ComfyUI Graph)
          </span>
          <span className="rounded bg-violet-500/20 px-1.5 py-0.5 font-mono text-[10px] text-violet-300">
            6 Nodes / 6 Edges
          </span>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setZoom((z) => Math.max(0.6, z - 0.1))}
            className="rounded bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300 hover:text-white"
          >
            -
          </button>
          <span className="font-mono text-[11px] text-zinc-400">{Math.round(zoom * 100)}%</span>
          <button
            type="button"
            onClick={() => setZoom((z) => Math.min(1.4, z + 0.1))}
            className="rounded bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300 hover:text-white"
          >
            +
          </button>
          <button
            type="button"
            onClick={() => setZoom(1)}
            className="rounded bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300 hover:text-white"
          >
            Reset
          </button>

          <button
            type="button"
            disabled={isSimulating}
            onClick={handleRunSimulation}
            className="flex items-center gap-1.5 rounded-full bg-violet-600 px-3 py-1 text-xs font-semibold text-white shadow-lg transition hover:bg-violet-500 disabled:opacity-50"
          >
            <Play className="h-3 w-3 fill-white" />
            <span>{isSimulating ? "Running DAG..." : "Execute Swarm Flow"}</span>
          </button>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 overflow-hidden">
        <div className="relative min-h-0 flex-1 overflow-auto p-6 select-none">
          <div
            style={{
              transform: `scale(${zoom})`,
              transformOrigin: "top left",
              width: "1050px",
              height: "450px",
            }}
            className="relative"
          >
            <svg className="pointer-events-none absolute inset-0 h-full w-full">
              <defs>
                <linearGradient id="edge-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
                  <stop offset="0%" stopColor="#8b5cf6" stopOpacity="0.8" />
                  <stop offset="100%" stopColor="#3b82f6" stopOpacity="0.8" />
                </linearGradient>
              </defs>
              {EDGES.map((edge, idx) => {
                const source = nodes.find((n) => n.id === edge.from);
                const target = nodes.find((n) => n.id === edge.to);
                if (!source || !target) return null;

                const startX = source.x + 190;
                const startY = source.y + 40;
                const endX = target.x;
                const endY = target.y + 40;
                const cpX1 = startX + (endX - startX) * 0.5;
                const cpY1 = startY;
                const cpX2 = startX + (endX - startX) * 0.5;
                const cpY2 = endY;

                return (
                  <g key={`${edge.from}-${edge.to}-${idx}`}>
                    <path
                      d={`M ${startX} ${startY} C ${cpX1} ${cpY1}, ${cpX2} ${cpY2}, ${endX} ${endY}`}
                      fill="none"
                      stroke="url(#edge-gradient)"
                      strokeWidth="2"
                      strokeDasharray="4 2"
                      className="animate-pulse"
                    />
                    <circle cx={startX} cy={startY} r="3" fill="#8b5cf6" />
                    <circle cx={endX} cy={endY} r="3" fill="#3b82f6" />
                  </g>
                );
              })}
            </svg>

            {nodes.map((node) => {
              const isSelected = selectedNodeId === node.id;
              const statusColor =
                node.status === "running"
                  ? "border-emerald-500 shadow-emerald-500/20"
                  : node.status === "waiting_approval"
                    ? "border-amber-500 shadow-amber-500/20"
                    : node.status === "completed"
                      ? "border-blue-500/60 shadow-blue-500/10"
                      : "border-zinc-800";

              return (
                <div
                  key={node.id}
                  onClick={() => setSelectedNodeId(node.id)}
                  style={{ left: node.x, top: node.y }}
                  className={`absolute w-48 cursor-pointer rounded-xl border bg-[#121215] p-3 shadow-xl transition-all hover:scale-105 ${statusColor} ${
                    isSelected ? "ring-2 ring-violet-400" : ""
                  }`}
                >
                  <div className="flex items-center justify-between border-b border-zinc-800 pb-1.5">
                    <div className="flex items-center gap-1.5 truncate">
                      {node.id === "node-trigger" ? (
                        <User className="h-3.5 w-3.5 text-sky-400" />
                      ) : node.id === "node-approval" ? (
                        <ShieldAlert className="h-3.5 w-3.5 text-amber-400" />
                      ) : node.id === "node-sandbox" ? (
                        <Terminal className="h-3.5 w-3.5 text-emerald-400" />
                      ) : (
                        <Bot className="h-3.5 w-3.5 text-violet-400" />
                      )}
                      <span className="truncate text-xs font-semibold text-zinc-100">
                        {node.name}
                      </span>
                    </div>

                    <span
                      className={`h-2 w-2 rounded-full ${
                        node.status === "running"
                          ? "animate-ping bg-emerald-400"
                          : node.status === "waiting_approval"
                            ? "bg-amber-400"
                            : node.status === "completed"
                              ? "bg-blue-400"
                              : "bg-zinc-600"
                      }`}
                    />
                  </div>

                  <p className="mt-1.5 truncate text-[10px] text-zinc-400">{node.role}</p>

                  <div className="mt-2 flex items-center justify-between text-[9px]">
                    <span className="rounded bg-zinc-800 px-1.5 py-0.5 font-mono text-zinc-300">
                      {node.model}
                    </span>
                    <span
                      className={`font-semibold uppercase ${
                        node.status === "running"
                          ? "text-emerald-400"
                          : node.status === "waiting_approval"
                            ? "text-amber-400"
                            : "text-zinc-500"
                      }`}
                    >
                      {node.status}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        <div className="w-80 shrink-0 border-l border-zinc-800 bg-[#0c0c0e] p-4 text-xs">
          <div className="border-b border-zinc-800 pb-3">
            <div className="flex items-center gap-2">
              <Bot className="h-4 w-4 text-violet-400" />
              <h3 className="font-semibold text-zinc-100">{selectedNode.name}</h3>
            </div>
            <p className="mt-1 text-[11px] text-zinc-400">{selectedNode.role}</p>
          </div>

          <div className="mt-4 space-y-3.5">
            <div>
              <label className="mb-1 block font-medium text-zinc-300" htmlFor="node-model-select">
                Assigned Model
              </label>
              <select
                id="node-model-select"
                value={selectedNode.model}
                onChange={(e) => handleUpdateSelectedNode({ model: e.target.value })}
                className="w-full rounded-lg border border-zinc-700 bg-zinc-900 px-2.5 py-1.5 text-xs text-zinc-200 focus:outline-none"
              >
                <option value="Gemini 3.8 Flash High">Gemini 3.8 Flash High</option>
                <option value="claude-3-5-sonnet">claude-3-5-sonnet</option>
                <option value="gpt-4o">gpt-4o</option>
                <option value="gpt-4o-mini">gpt-4o-mini</option>
                <option value="Local Seatbelt Runner">Local Seatbelt Runner</option>
                <option value="Author != Approver Gate">Author != Approver Gate</option>
              </select>
            </div>

            <div>
              <div className="mb-1 flex items-center justify-between">
                <label className="font-medium text-zinc-300" htmlFor="node-temp-slider">
                  Temperature
                </label>
                <span className="font-mono text-zinc-400">{selectedNode.temperature}</span>
              </div>
              <input
                id="node-temp-slider"
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={selectedNode.temperature}
                onChange={(e) =>
                  handleUpdateSelectedNode({ temperature: parseFloat(e.target.value) })
                }
                className="w-full accent-violet-500"
              />
            </div>

            <div>
              <label className="mb-1 block font-medium text-zinc-300" htmlFor="node-sop-text">
                Role SOP & Constraints
              </label>
              <textarea
                id="node-sop-text"
                rows={3}
                value={selectedNode.sop}
                onChange={(e) => handleUpdateSelectedNode({ sop: e.target.value })}
                className="w-full resize-none rounded-lg border border-zinc-700 bg-zinc-900 p-2 text-[11px] text-zinc-200 focus:outline-none"
              />
            </div>

            <div>
              <span className="mb-1 block font-medium text-zinc-300">Live Agent Thought</span>
              <div className="rounded-lg border border-zinc-800 bg-[#08080a] p-2.5 font-mono text-[10px] leading-relaxed text-zinc-300">
                {selectedNode.lastThought}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
