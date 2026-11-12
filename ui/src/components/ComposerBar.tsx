import {
  type FormEvent,
  type KeyboardEvent,
  type RefObject,
  useState,
} from "react";
import {
  ArrowUp,
  AudioWaveform,
  Check,
  ChevronDown,
  Folder,
  GitBranch,
  Mic,
  RotateCcw,
  Sliders,
  Sparkles,
  Zap,
} from "lucide-react";

interface ComposerBarProps {
  prompt: string;
  onChangePrompt: (value: string) => void;
  promptRef: RefObject<HTMLTextAreaElement | null>;
  busy: boolean;
  canResume: boolean;
  model: string;
  onChangeModel: (value: string) => void;
  activeProject?: string;
  onOpenSettings: () => void;
  onStartRun: () => void;
  onResumeRun: () => void;
}

const MODEL_OPTIONS = [
  { id: "Gemini 3.8 Flash High", label: "Gemini 3.8 Flash High", badge: "Default" },
  { id: "gpt-4o-mini", label: "gpt-4o-mini", badge: "Fast" },
  { id: "gpt-4o", label: "gpt-4o", badge: "Pro" },
  { id: "claude-3-5-sonnet", label: "claude-3-5-sonnet", badge: "Frontier" },
  { id: "deepseek-r1", label: "deepseek-r1", badge: "Reasoning" },
];

export function ComposerBar({
  prompt,
  onChangePrompt,
  promptRef,
  busy,
  canResume,
  model,
  onChangeModel,
  activeProject = "Create-LLM",
  onOpenSettings,
  onStartRun,
  onResumeRun,
}: ComposerBarProps) {
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false);
  const [speedMode, setSpeedMode] = useState<"Turbo" | "Standard">("Turbo");
  const [voiceActive, setVoiceActive] = useState(false);

  const handleSubmit = (event?: FormEvent) => {
    event?.preventDefault();
    if (!prompt.trim() || busy) return;
    onStartRun();
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const selectModel = (newModel: string) => {
    onChangeModel(newModel);
    setModelDropdownOpen(false);
  };

  return (
    <div className="pointer-events-none absolute bottom-4 left-0 right-0 z-30 px-4 sm:px-8">
      <div className="pointer-events-auto mx-auto max-w-2xl rounded-2xl border border-zinc-800 bg-[#141416]/95 p-3 shadow-2xl backdrop-blur-md">
        <div className="mb-2 flex items-center gap-2 border-b border-zinc-800/60 pb-2 text-xs text-zinc-400">
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-lg bg-zinc-900 px-2 py-1 text-[11px] font-medium text-zinc-300 transition hover:bg-zinc-800"
          >
            <Folder className="h-3 w-3 text-zinc-400" />
            <span>{activeProject}</span>
            <ChevronDown className="h-2.5 w-2.5 text-zinc-500" />
          </button>

          <button
            type="button"
            className="flex items-center gap-1.5 rounded-lg bg-zinc-900 px-2 py-1 text-[11px] font-medium text-zinc-300 transition hover:bg-zinc-800"
          >
            <GitBranch className="h-3 w-3 text-zinc-400" />
            <span>Local Checkout</span>
            <ChevronDown className="h-2.5 w-2.5 text-zinc-500" />
          </button>
        </div>

        <textarea
          ref={promptRef}
          rows={2}
          value={prompt}
          onChange={(e) => onChangePrompt(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask anything, @ to mention, / for actions"
          className="w-full resize-none bg-transparent text-xs text-zinc-100 placeholder-zinc-500 focus:outline-none sm:text-sm"
        />

        <div className="mt-2 flex items-center justify-between border-t border-zinc-800/80 pt-2 text-xs">
          <div className="relative flex items-center gap-2">
            <button
              type="button"
              onClick={() => setModelDropdownOpen((v) => !v)}
              className="flex items-center gap-1.5 rounded-lg bg-zinc-800/90 px-2.5 py-1 text-xs font-medium text-zinc-200 transition hover:bg-zinc-700/80"
            >
              <Sparkles className="h-3 w-3 text-blue-400" />
              <span>{model || "Gemini 3.8 Flash High"}</span>
              <ChevronDown className="h-3 w-3 text-zinc-400" />
            </button>

            {modelDropdownOpen && (
              <div className="absolute bottom-full left-0 mb-2 w-56 rounded-xl border border-zinc-700 bg-[#121214] p-1 shadow-2xl">
                <div className="px-2 py-1 text-[10px] font-semibold tracking-wider text-zinc-400 uppercase">
                  Select Model
                </div>
                {MODEL_OPTIONS.map((opt) => (
                  <button
                    key={opt.id}
                    type="button"
                    onClick={() => selectModel(opt.id)}
                    className="flex w-full items-center justify-between rounded-lg px-2 py-1.5 text-left text-xs text-zinc-200 hover:bg-zinc-800"
                  >
                    <span className="truncate">{opt.label}</span>
                    <div className="flex items-center gap-1">
                      <span className="rounded bg-zinc-800 px-1 py-0.5 text-[9px] text-zinc-400">
                        {opt.badge}
                      </span>
                      {model === opt.id && <Check className="h-3 w-3 text-blue-400" />}
                    </div>
                  </button>
                ))}
              </div>
            )}

            <button
              type="button"
              onClick={() =>
                setSpeedMode((prev) => (prev === "Turbo" ? "Standard" : "Turbo"))
              }
              className="flex items-center gap-1 rounded-lg px-2 py-1 text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-200"
            >
              <Zap className="h-3 w-3 text-amber-400" />
              <span>{speedMode}</span>
              <ChevronDown className="h-3 w-3 text-zinc-500" />
            </button>
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onOpenSettings}
              aria-label="Engine settings"
              className="rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
            >
              <Sliders className="h-4 w-4" />
            </button>

            <button
              type="button"
              aria-label="Dictate message"
              className="rounded-lg p-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
            >
              <Mic className="h-4 w-4" />
            </button>

            <button
              type="button"
              onClick={() => setVoiceActive((v) => !v)}
              aria-label="Voice conversation mode"
              className={`flex items-center gap-1 rounded-full px-2 py-1 transition ${
                voiceActive
                  ? "bg-blue-500/20 text-blue-400"
                  : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
              }`}
            >
              <AudioWaveform className="h-4 w-4" />
            </button>

            {canResume && (
              <button
                type="button"
                disabled={busy}
                onClick={onResumeRun}
                aria-label="Resume run"
                className="flex items-center gap-1 rounded-lg border border-sky-500/40 bg-sky-500/10 px-2.5 py-1 text-xs text-sky-300 hover:bg-sky-500/20"
              >
                <RotateCcw className="h-3.5 w-3.5" />
                <span>Resume</span>
              </button>
            )}

            <button
              type="button"
              disabled={busy || !prompt.trim()}
              onClick={() => handleSubmit()}
              aria-label="Send message"
              className="flex h-7 w-7 items-center justify-center rounded-full bg-zinc-200 text-zinc-950 transition hover:bg-white disabled:opacity-30"
            >
              <ArrowUp className="h-4 w-4" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
