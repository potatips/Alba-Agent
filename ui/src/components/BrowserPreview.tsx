import { useState } from "react";
import {
  ExternalLink,
  Globe,
  Laptop,
  RefreshCw,
  Smartphone,
  Tablet,
} from "lucide-react";

interface BrowserPreviewProps {
  initialUrl?: string;
  onOpenExternal?: (url: string) => void;
}

export function BrowserPreview({
  initialUrl = "http://localhost:1420",
  onOpenExternal,
}: BrowserPreviewProps) {
  const [url, setUrl] = useState(initialUrl);
  const [inputUrl, setInputUrl] = useState(initialUrl);
  const [deviceMode, setDeviceMode] = useState<"desktop" | "tablet" | "mobile">("desktop");
  const [refreshKey, setRefreshKey] = useState(0);

  const handleNavigate = (e: React.FormEvent) => {
    e.preventDefault();
    let dest = inputUrl.trim();
    if (!dest.startsWith("http://") && !dest.startsWith("https://")) {
      dest = `https://${dest}`;
    }
    setUrl(dest);
    setInputUrl(dest);
  };

  const reload = () => {
    setRefreshKey((k) => k + 1);
  };

  const getContainerWidth = () => {
    switch (deviceMode) {
      case "mobile":
        return "max-w-[390px] h-[844px]";
      case "tablet":
        return "max-w-[768px] h-[1024px]";
      default:
        return "w-full h-full";
    }
  };

  return (
    <div className="flex h-full flex-col bg-[#09090b] text-zinc-100">
      <div className="flex h-10 shrink-0 items-center justify-between border-b border-zinc-800 bg-[#0c0c0e] px-3 text-xs">
        <form onSubmit={handleNavigate} className="flex flex-1 items-center gap-2">
          <Globe className="h-4 w-4 shrink-0 text-blue-400" />
          <input
            type="text"
            value={inputUrl}
            onChange={(e) => setInputUrl(e.target.value)}
            placeholder="Enter URL to preview..."
            className="w-full max-w-lg rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-1 font-mono text-[11px] text-zinc-200 focus:border-blue-500 focus:outline-none"
          />
          <button
            type="button"
            onClick={reload}
            aria-label="Reload preview"
            className="rounded p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </button>
        </form>

        <div className="flex items-center gap-1 text-zinc-400">
          <button
            type="button"
            onClick={() => setDeviceMode("desktop")}
            aria-label="Desktop viewport"
            className={`rounded p-1 transition ${
              deviceMode === "desktop"
                ? "bg-zinc-800 text-white"
                : "hover:bg-zinc-850 hover:text-zinc-200"
            }`}
          >
            <Laptop className="h-4 w-4" />
          </button>
          <button
            type="button"
            onClick={() => setDeviceMode("tablet")}
            aria-label="Tablet viewport"
            className={`rounded p-1 transition ${
              deviceMode === "tablet"
                ? "bg-zinc-800 text-white"
                : "hover:bg-zinc-850 hover:text-zinc-200"
            }`}
          >
            <Tablet className="h-4 w-4" />
          </button>
          <button
            type="button"
            onClick={() => setDeviceMode("mobile")}
            aria-label="Mobile viewport"
            className={`rounded p-1 transition ${
              deviceMode === "mobile"
                ? "bg-zinc-800 text-white"
                : "hover:bg-zinc-850 hover:text-zinc-200"
            }`}
          >
            <Smartphone className="h-4 w-4" />
          </button>
          <div className="mx-1 h-3.5 w-[1px] bg-zinc-800" />
          <button
            type="button"
            onClick={() => onOpenExternal?.(url)}
            aria-label="Open in external browser"
            className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <ExternalLink className="h-4 w-4" />
          </button>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto bg-[#050507] p-3">
        <div
          className={`overflow-hidden rounded-xl border border-zinc-800 bg-white shadow-2xl transition-all ${getContainerWidth()}`}
        >
          <iframe
            key={refreshKey}
            src={url}
            title="Antigravity Browser Preview"
            className="h-full w-full border-0 bg-white"
            sandbox="allow-same-origin allow-scripts allow-forms allow-popups"
          />
        </div>
      </div>
    </div>
  );
}
