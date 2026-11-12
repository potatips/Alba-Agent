import { type FormEvent, useEffect, useRef, useState } from "react";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal as XTerm } from "@xterm/xterm";
import { Play, TerminalSquare, Trash2 } from "lucide-react";
import { executeTerminalCommand } from "../tauri";

export interface TerminalChunk {
  id: string;
  stream: string;
  text: string;
}

interface SandboxTerminalProps {
  chunks: TerminalChunk[];
  onClear: () => void;
}

export function SandboxTerminal({ chunks, onClear }: SandboxTerminalProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const writtenCountRef = useRef<number>(0);

  const [inputCmd, setInputCmd] = useState("");
  const [running, setRunning] = useState(false);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const term = new XTerm({
      theme: {
        background: "#09090b",
        foreground: "#e2e8f0",
        cursor: "#38bdf8",
      },
      fontSize: 12,
      fontFamily: '"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace',
      convertEol: true,
      disableStdin: true,
      scrollback: 2000,
    });
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(container);
    fitAddon.fit();
    term.writeln("\x1b[36mAlba Agent Native Sandbox Terminal ready. Type a command below.\x1b[0m");

    termRef.current = term;
    fitRef.current = fitAddon;
    writtenCountRef.current = 0;

    const observer = new ResizeObserver(() => {
      fitAddon.fit();
    });
    observer.observe(container);

    return () => {
      observer.disconnect();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, []);

  useEffect(() => {
    const term = termRef.current;
    if (!term) return;

    if (chunks.length < writtenCountRef.current) {
      term.clear();
      writtenCountRef.current = 0;
    }

    for (let i = writtenCountRef.current; i < chunks.length; i++) {
      const chunk = chunks[i];
      if (chunk.stream === "stderr") {
        term.write(`\x1b[31m${chunk.text}\x1b[0m`);
      } else if (chunk.stream === "system") {
        term.writeln(`\x1b[33m${chunk.text}\x1b[0m`);
      } else {
        term.write(chunk.text);
      }
    }
    writtenCountRef.current = chunks.length;
  }, [chunks]);

  const runCommand = async (cmdToRun: string) => {
    const trimmed = cmdToRun.trim();
    if (!trimmed || running) return;

    const term = termRef.current;
    if (term) {
      term.writeln(`\r\n\x1b[32m$ ${trimmed}\x1b[0m`);
    }

    setRunning(true);
    setInputCmd("");
    try {
      const res = await executeTerminalCommand(trimmed);
      if (term) {
        if (res.stdout) {
          term.writeln(res.stdout.replace(/\n/g, "\r\n"));
        }
        if (res.stderr) {
          term.writeln(`\x1b[31m${res.stderr.replace(/\n/g, "\r\n")}\x1b[0m`);
        }
        if (res.exit_code === 0) {
          term.writeln("\x1b[32mProcess completed with exit code 0\x1b[0m");
        } else {
          term.writeln(`\x1b[31mProcess exited with code ${res.exit_code}\x1b[0m`);
        }
      }
    } catch (err) {
      if (term) {
        term.writeln(`\x1b[31mError: ${err instanceof Error ? err.message : String(err)}\x1b[0m`);
      }
    } finally {
      setRunning(false);
    }
  };

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    runCommand(inputCmd);
  };

  return (
    <div className="flex h-full flex-col border-t border-zinc-800 bg-[#09090b]">
      <div className="flex items-center justify-between border-b border-zinc-800 bg-[#0c0c0e] px-3 py-1">
        <div className="flex items-center gap-2">
          <span className="flex items-center gap-1.5 text-[11px] font-semibold tracking-wider text-zinc-300 uppercase">
            <TerminalSquare className="h-3.5 w-3.5 text-emerald-400" />
            Sandbox Terminal
          </span>
          <div className="hidden items-center gap-1 text-[10px] text-zinc-400 sm:flex">
            <button
              type="button"
              onClick={() => runCommand("git status -s")}
              className="rounded bg-zinc-800 px-1.5 py-0.5 hover:bg-zinc-700 hover:text-white"
            >
              git status
            </button>
            <button
              type="button"
              onClick={() => runCommand("cargo test --workspace -j 2")}
              className="rounded bg-zinc-800 px-1.5 py-0.5 hover:bg-zinc-700 hover:text-white"
            >
              cargo test
            </button>
            <button
              type="button"
              onClick={() => runCommand("python3 scripts/benchmark.py")}
              className="rounded bg-zinc-800 px-1.5 py-0.5 hover:bg-zinc-700 hover:text-white"
            >
              benchmark
            </button>
          </div>
        </div>

        <button
          type="button"
          onClick={onClear}
          className="flex items-center gap-1 rounded px-2 py-0.5 text-[11px] text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-100"
        >
          <Trash2 className="h-3 w-3" />
          Clear
        </button>
      </div>

      <div ref={containerRef} className="min-h-0 flex-1 p-2" />

      <form
        onSubmit={handleSubmit}
        className="flex items-center gap-2 border-t border-zinc-800 bg-[#0c0c0e] px-3 py-1.5"
      >
        <span className="font-mono text-xs font-bold text-emerald-400">$</span>
        <input
          type="text"
          value={inputCmd}
          onChange={(e) => setInputCmd(e.target.value)}
          placeholder="Enter command (e.g. ls, cargo test, git status)..."
          disabled={running}
          className="flex-1 bg-transparent font-mono text-xs text-zinc-100 placeholder-zinc-600 focus:outline-none"
        />
        <button
          type="submit"
          disabled={running || !inputCmd.trim()}
          className="flex items-center gap-1 rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-200 transition hover:bg-zinc-700 disabled:opacity-30"
        >
          <Play className="h-3 w-3 fill-zinc-200" />
          <span>Run</span>
        </button>
      </form>
    </div>
  );
}
