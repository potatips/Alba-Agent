"use client";

import { useEffect, useRef, useState } from "react";
import { useLocale } from "@/components/locale-provider";
import { useWorkspace } from "@/components/workspace/workspace-context";
import { Icon } from "./icons";
import { getDesktop, type FolderInfo } from "@/lib/desktop";

export function CreateProjectModal({ onClose }: { onClose: () => void }) {
  const { t } = useLocale();
  const workspace = useWorkspace();
  const [tab, setTab] = useState<"local" | "remote">("local");
  const [selectedFolders, setSelectedFolders] = useState<FolderInfo[]>([]);
  const [remoteUrl, setRemoteUrl] = useState("");
  const panelRef = useRef<HTMLElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const previousFocus = useRef<Element | null>(null);

  useEffect(() => {
    previousFocus.current = document.activeElement;
    panelRef.current?.focus();
    return () => {
      if (previousFocus.current instanceof HTMLElement && previousFocus.current.isConnected) {
        previousFocus.current.focus();
      }
    };
  }, []);

  const handleAddFolder = async () => {
    const desktop = getDesktop();
    if (desktop?.openFolder) {
      try {
        const next = await desktop.openFolder();
        if (next) {
          setSelectedFolders((prev) => [...prev.filter((f) => f.root !== next.root), next]);
        }
      } catch {
        fileInputRef.current?.click();
      }
    } else if (typeof window !== "undefined" && "showDirectoryPicker" in window) {
      try {
        const picker = (window as unknown as { showDirectoryPicker: () => Promise<{ name: string }> }).showDirectoryPicker;
        const dirHandle = await picker();
        const next: FolderInfo = {
          root: `/${dirHandle.name}`,
          name: dirHandle.name,
          files: [],
          truncated: false,
        };
        setSelectedFolders((prev) => [...prev.filter((f) => f.root !== next.root), next]);
      } catch {
        fileInputRef.current?.click();
      }
    } else {
      fileInputRef.current?.click();
    }
  };

  const handleFileInputChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const files = event.target.files;
    if (files && files.length > 0) {
      const first = files[0];
      const relPath = first.webkitRelativePath || first.name;
      const folderName = relPath.split("/")[0] || "project";
      const next: FolderInfo = {
        root: `/${folderName}`,
        name: folderName,
        files: Array.from(files).map((f) => ({ path: f.webkitRelativePath || f.name, size: f.size })),
        truncated: false,
      };
      setSelectedFolders((prev) => [...prev.filter((f) => f.root !== next.root), next]);
    }
  };

  const handleCreate = async () => {
    if (tab === "local" && selectedFolders.length > 0) {
      try {
        await workspace.selectProject(selectedFolders[0].root);
      } catch {
        void 0;
      }
      onClose();
    } else if (tab === "remote" && remoteUrl.trim()) {
      const name = remoteUrl.split("/").pop()?.replace(/\.git$/, "") || "remote-project";
      try {
        await workspace.selectProject(`remote://${name}`);
      } catch {
        void 0;
      }
      onClose();
    } else {
      onClose();
    }
  };

  const hasSelection = (tab === "local" && selectedFolders.length > 0) || (tab === "remote" && remoteUrl.trim().length > 0);

  return (
    <div className="ag-modal" role="presentation" onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <section
        className="nx-create-project-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("shell.createProject")}
        tabIndex={-1}
        ref={panelRef}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onClose();
          } else if (event.key === "Tab") {
            const nodes = [...event.currentTarget.querySelectorAll<HTMLElement>(
              'button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href],[tabindex="0"]'
            )].filter((node) => node.getClientRects().length);
            const first = nodes[0];
            const last = nodes.at(-1);
            if (event.shiftKey && (document.activeElement === first || document.activeElement === panelRef.current)) {
              event.preventDefault();
              last?.focus();
            } else if (!event.shiftKey && document.activeElement === last) {
              event.preventDefault();
              first?.focus();
            }
          }
        }}
      >
        <div className="nx-create-project-head">
          <h2>{t("shell.createProject")}</h2>
          <button
            type="button"
            className="nx-create-project-close"
            aria-label={t("common.close")}
            onClick={onClose}
          >
            <Icon name="close" size={16} />
          </button>
        </div>

        <div className="nx-create-project-tabs" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={tab === "local"}
            className={`nx-create-project-tab ${tab === "local" ? "is-active" : ""}`}
            onClick={() => setTab("local")}
          >
            <Icon name="computer" size={15} />
            <span>{t("shell.thisComputer")}</span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "remote"}
            className={`nx-create-project-tab ${tab === "remote" ? "is-active" : ""}`}
            onClick={() => setTab("remote")}
          >
            <Icon name="cloud" size={15} />
            <span>{t("shell.remote")}</span>
          </button>
        </div>

        {tab === "local" ? (
          <div className="nx-create-project-content">
            <div className="nx-create-project-section-label">
              {t("shell.selectFolders")}
            </div>

            {selectedFolders.length > 0 && (
              <div className="nx-create-project-selected-list">
                {selectedFolders.map((folder, index) => (
                  <div key={folder.root + index} className="nx-create-project-selected-item">
                    <div className="nx-create-project-selected-info">
                      <Icon name="folder" size={15} />
                      <span>{folder.name}</span>
                    </div>
                    <button
                      type="button"
                      className="nx-create-project-close"
                      aria-label={t("common.close")}
                      onClick={() => setSelectedFolders((prev) => prev.filter((_, i) => i !== index))}
                    >
                      <Icon name="close" size={14} />
                    </button>
                  </div>
                ))}
              </div>
            )}

            <button
              type="button"
              className="nx-create-project-add-btn"
              onClick={() => void handleAddFolder()}
            >
              <Icon name="plus" size={15} />
              <span>{t("shell.addFolder")}</span>
            </button>
            <input
              type="file"
              ref={fileInputRef}
              style={{ display: "none" }}
              onChange={handleFileInputChange}
              {...({ webkitdirectory: "", directory: "" } as Record<string, string>)}
            />
          </div>
        ) : (
          <div className="nx-create-project-content">
            <div className="nx-create-project-section-label">
              {t("shell.remote")}
            </div>
            <input
              type="text"
              className="nx-create-project-remote-input"
              placeholder={t("shell.remotePlaceholder")}
              value={remoteUrl}
              onChange={(e) => setRemoteUrl(e.target.value)}
            />
          </div>
        )}

        <div className="nx-create-project-foot">
          {hasSelection && (
            <button
              type="button"
              className="nx-create-project-primary-btn"
              onClick={() => void handleCreate()}
            >
              {t("shell.createProject")}
            </button>
          )}
          <button
            type="button"
            className="nx-create-project-skip-btn"
            onClick={onClose}
          >
            {t("shell.skip")}
          </button>
        </div>
      </section>
    </div>
  );
}
