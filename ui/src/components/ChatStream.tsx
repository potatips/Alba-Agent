import { useEffect, useRef, useState } from "react";
import {
  ArrowDown,
  Check,
  Columns2,
  Copy,
  MoreVertical,
  ThumbsDown,
  ThumbsUp,
} from "lucide-react";
import { ApprovalCard } from "./ApprovalCard";
import { MathFormula } from "./MathFormula";
import type { PendingApprovalDto } from "../types";

export interface ChatMessage {
  id: string;
  sender: "user" | "assistant";
  content: string;
  timestamp: string;
  showMath?: boolean;
}

interface ChatStreamProps {
  activeProject: string;
  activeTask: string;
  runStatus: string;
  messages: ChatMessage[];
  pendingApproval: PendingApprovalDto | null;
  approverId: string;
  busy: boolean;
  onChangeApproverId: (value: string) => void;
  onAllowOnce: () => void;
  onDeny: () => void;
  onRestartToUpdate: () => void;
  onToggleSplitPane?: () => void;
}

export function ChatStream({
  activeProject,
  activeTask,
  runStatus,
  messages,
  pendingApproval,
  approverId,
  busy,
  onChangeApproverId,
  onAllowOnce,
  onDeny,
  onRestartToUpdate,
  onToggleSplitPane,
}: ChatStreamProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [likedMap, setLikedMap] = useState<Record<string, "up" | "down">>({});

  const scrollToBottom = () => {
    if (containerRef.current) {
      containerRef.current.scrollTo({
        top: containerRef.current.scrollHeight,
        behavior: "smooth",
      });
    }
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages, pendingApproval]);

  const handleCopy = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const handleThumb = (id: string, dir: "up" | "down") => {
    setLikedMap((prev) => ({
      ...prev,
      [id]: prev[id] === dir ? undefined! : dir,
    }));
  };

  return (
    <div className="relative flex h-full flex-col bg-[#09090b] text-zinc-100">
      <header className="flex h-11 shrink-0 items-center justify-between border-b border-zinc-800/80 bg-[#0c0c0e]/95 px-4 backdrop-blur-md">
        <div className="flex items-center gap-2 truncate text-xs text-zinc-400">
          <span className="font-medium text-zinc-300">{activeProject}</span>
          <span>/</span>
          <span className="truncate text-zinc-200">{activeTask}</span>
        </div>

        <div className="flex items-center gap-2">
          {runStatus && runStatus !== "idle" && (
            <span className="rounded-full border border-blue-500/40 bg-blue-500/10 px-2 py-0.5 font-mono text-[10px] text-blue-300">
              {runStatus}
            </span>
          )}

          <button
            type="button"
            onClick={onRestartToUpdate}
            className="flex items-center gap-1.5 rounded-full bg-blue-600 px-3 py-1 text-xs font-medium text-white shadow-sm transition hover:bg-blue-500"
          >
            <span>Restart to Update</span>
            <span>→</span>
          </button>

          <button
            type="button"
            onClick={onToggleSplitPane}
            aria-label="Split pane"
            className="rounded p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <Columns2 className="h-4 w-4" />
          </button>
          <button
            type="button"
            aria-label="More options"
            className="rounded p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <MoreVertical className="h-4 w-4" />
          </button>
        </div>
      </header>

      <div
        ref={containerRef}
        className="min-h-0 flex-1 overflow-y-auto px-4 py-6 sm:px-8"
      >
        {messages.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center text-center">
            <div className="relative mb-6 flex h-24 w-24 items-center justify-center">
              <svg
                viewBox="0 0 100 100"
                className="h-24 w-24 text-zinc-600 drop-shadow-[0_0_25px_rgba(255,255,255,0.15)]"
                fill="none"
              >
                <path
                  d="M 25 80 C 25 35, 75 35, 75 80 C 65 80, 60 55, 50 55 C 40 55, 35 80, 25 80 Z"
                  fill="currentColor"
                />
              </svg>
            </div>
            <h1 className="text-xl font-medium tracking-tight text-zinc-100 sm:text-2xl">
              Good to see you, Phạm. What's next?
            </h1>
          </div>
        ) : (
          <div className="mx-auto flex max-w-2xl flex-col gap-6 pb-36">
            {messages.map((msg) => {
              if (msg.sender === "user") {
                return (
                  <div key={msg.id} className="flex justify-end">
                    <div className="max-w-[85%] rounded-2xl bg-zinc-800 px-4 py-2.5 text-sm text-zinc-100 shadow-sm">
                      <p className="whitespace-pre-wrap">{msg.content}</p>
                    </div>
                  </div>
                );
              }

              return (
                <div key={msg.id} className="flex flex-col gap-2">
                  <div className="space-y-3 text-sm leading-relaxed text-zinc-200">
                    {msg.showMath ? (
                      <div>
                        <h3 className="mb-2 text-sm font-semibold tracking-wide text-zinc-100">
                          Bản nâng cấp Toán học: GOR v2 (Smooth C<sup>∞</sup> Modulation)
                        </h3>
                        <p className="text-xs text-zinc-300 sm:text-sm">
                          Trong bản v1 trước, hàm <span className="font-serif italic">ϕ(ρ) = ReLU(ρ)</span> có một điểm yếu: Tại bước khởi tạo khi <span className="font-serif italic">ρ ≈ 0</span>, <span className="font-serif italic">ReLU(0) = 0</span> khiến nhánh song song <span className="font-serif italic">u<sup>∥</sup></span> bị cắt về 0, làm mô hình bị "chậm nhịp" ở 250 step đầu.
                        </p>
                        <p className="mt-2 text-xs text-zinc-300 sm:text-sm">
                          Ở GOR v2, công thức được nâng cấp thành hàm giải tích trơn vô hạn (<span className="font-serif italic">C<sup>∞</sup></span>):
                        </p>

                        <MathFormula formula="gor_update" />
                        <MathFormula formula="gor_phi" />

                        <p className="mt-2 text-xs text-zinc-300 sm:text-sm">
                          Đảm bảo gradient lan truyền trơn tru ngay từ step 0 mà không làm bùng nổ Loss trên GPU L4.
                        </p>
                      </div>
                    ) : (
                      <div className="whitespace-pre-wrap text-xs sm:text-sm">
                        {msg.content}
                      </div>
                    )}
                  </div>

                  <div className="mt-1 flex items-center gap-3 text-xs text-zinc-500">
                    <span>{msg.timestamp}</span>
                    <button
                      type="button"
                      onClick={() => handleCopy(msg.id, msg.content)}
                      aria-label="Copy message"
                      className="flex items-center gap-1 hover:text-zinc-300"
                    >
                      {copiedId === msg.id ? (
                        <Check className="h-3.5 w-3.5 text-emerald-400" />
                      ) : (
                        <Copy className="h-3.5 w-3.5" />
                      )}
                    </button>
                    <button
                      type="button"
                      onClick={() => handleThumb(msg.id, "up")}
                      aria-label="Thumbs up"
                      className={`hover:text-zinc-300 ${
                        likedMap[msg.id] === "up" ? "text-blue-400" : ""
                      }`}
                    >
                      <ThumbsUp className="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => handleThumb(msg.id, "down")}
                      aria-label="Thumbs down"
                      className={`hover:text-zinc-300 ${
                        likedMap[msg.id] === "down" ? "text-rose-400" : ""
                      }`}
                    >
                      <ThumbsDown className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              );
            })}

            {pendingApproval && (
              <div className="mt-2">
                <ApprovalCard
                  approval={pendingApproval}
                  approverId={approverId}
                  busy={busy}
                  onChangeApproverId={onChangeApproverId}
                  onAllowOnce={onAllowOnce}
                  onDeny={onDeny}
                />
              </div>
            )}
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={scrollToBottom}
        aria-label="Scroll to bottom"
        className="absolute bottom-32 right-8 z-20 flex h-8 w-8 items-center justify-center rounded-full border border-zinc-700 bg-zinc-900/90 text-zinc-300 shadow-lg backdrop-blur-md transition hover:bg-zinc-800 hover:text-white"
      >
        <ArrowDown className="h-4 w-4" />
      </button>
    </div>
  );
}
