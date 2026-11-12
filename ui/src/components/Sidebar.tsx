import { useState } from "react";
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Clock,
  Files,
  Filter,
  Folder,
  History,
  Lock,
  Plus,
  Search,
  Settings,
  SlidersHorizontal,
} from "lucide-react";

interface SidebarProps {
  activeProject: string;
  activeTask: string;
  onSelectTask: (project: string, task: string) => void;
  onNewConversation: () => void;
  onOpenSettings: () => void;
  sidebarTab: "projects" | "files" | "checkpoints";
  onChangeSidebarTab: (tab: "projects" | "files" | "checkpoints") => void;
  filesContent: React.ReactNode;
  checkpointsContent: React.ReactNode;
}

interface ProjectFolder {
  id: string;
  name: string;
  tasks: Array<{
    id: string;
    title: string;
    timeAgo: string;
    unread?: boolean;
    updatedAt?: string;
  }>;
}

const DEFAULT_PROJECTS: ProjectFolder[] = [
  {
    id: "create-llm",
    name: "Create-LLM",
    tasks: [
      {
        id: "task-create-folder",
        title: "Creating Folder LLM Model",
        timeAgo: "6h",
      },
    ],
  },
  {
    id: "nexus-agent",
    name: "NexusAgent",
    tasks: [
      {
        id: "task-failing-tests",
        title: "Fixing Failing API Tests",
        timeAgo: "1h",
        unread: true,
        updatedAt: "10 thg 11, 23:55",
      },
      {
        id: "task-continuous-cost",
        title: "Chi Phí Kiểm Tra Liên Tục",
        timeAgo: "5d",
      },
      {
        id: "task-ui-ux-prompt",
        title: "Prompt Thiết Kế Giao Diện UX/UI",
        timeAgo: "6d",
        unread: true,
      },
      {
        id: "task-video-prompt",
        title: "Tạo Prompt Viết Video Robot B...",
        timeAgo: "8d",
      },
    ],
  },
  {
    id: "website-nexus",
    name: "Website_Nexus",
    tasks: [
      {
        id: "task-web-preview",
        title: "Running Website Interface Previ...",
        timeAgo: "1d",
      },
      {
        id: "task-multi-google",
        title: "Sử Dụng Nhiều Tài Khoản Googl...",
        timeAgo: "1d",
      },
    ],
  },
  {
    id: "lab-ai",
    name: "Lab_AI",
    tasks: [
      {
        id: "task-math-footnote",
        title: "Adding Mathematical Footnot...",
        timeAgo: "10d",
      },
    ],
  },
  {
    id: "nexus-studio",
    name: "NexusStudio",
    tasks: [
      {
        id: "task-markdown-check",
        title: "Checking Markdown File Struc...",
        timeAgo: "12d",
      },
    ],
  },
  {
    id: "ds",
    name: "ds",
    tasks: [],
  },
];

const DEFAULT_CONVERSATIONS = [
  {
    id: "conv-vinfast",
    title: "VinFast AI Engineer Job Description",
    timeAgo: "3d",
  },
  {
    id: "conv-latin",
    title: "Latin Words For Enlightenment And...",
    timeAgo: "3d",
  },
];

export function Sidebar({
  activeProject,
  activeTask,
  onSelectTask,
  onNewConversation,
  onOpenSettings,
  sidebarTab,
  onChangeSidebarTab,
  filesContent,
  checkpointsContent,
}: SidebarProps) {
  const [openFolders, setOpenFolders] = useState<Record<string, boolean>>({
    "create-llm": true,
    "nexus-agent": true,
    "website-nexus": false,
    "lab-ai": false,
    "nexus-studio": false,
    "ds": false,
  });

  const [hoveredTask, setHoveredTask] = useState<string | null>(null);

  const toggleFolder = (folderId: string) => {
    setOpenFolders((prev) => ({
      ...prev,
      [folderId]: !prev[folderId],
    }));
  };

  return (
    <aside className="flex h-full w-64 shrink-0 flex-col border-r border-zinc-800 bg-[#0c0c0e] text-zinc-300">
      <div className="flex h-11 items-center justify-between border-b border-zinc-800/80 px-3">
        <div className="flex items-center gap-1 text-zinc-400">
          <button
            type="button"
            aria-label="Previous"
            className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <ChevronLeft className="h-4 w-4" />
          </button>
          <button
            type="button"
            aria-label="Next"
            className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <ChevronRight className="h-4 w-4" />
          </button>
          <button
            type="button"
            aria-label="Search"
            className="rounded p-1 hover:bg-zinc-800 hover:text-zinc-100"
          >
            <Search className="h-3.5 w-3.5" />
          </button>
        </div>

        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => onChangeSidebarTab("projects")}
            aria-label="Projects view"
            className={`rounded p-1 text-xs transition ${
              sidebarTab === "projects"
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            <Folder className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onChangeSidebarTab("files")}
            aria-label="Files view"
            className={`rounded p-1 text-xs transition ${
              sidebarTab === "files"
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            <Files className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onChangeSidebarTab("checkpoints")}
            aria-label="Checkpoints view"
            className={`rounded p-1 text-xs transition ${
              sidebarTab === "checkpoints"
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            <History className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <div className="p-3">
        <button
          type="button"
          onClick={onNewConversation}
          className="flex w-full items-center justify-center gap-2 rounded-xl border border-zinc-800 bg-[#18181b] px-3 py-2 text-xs font-medium text-zinc-200 shadow-sm transition hover:border-zinc-700 hover:bg-zinc-800"
        >
          <Plus className="h-3.5 w-3.5 text-zinc-400" />
          <span>New Conversation</span>
        </button>
      </div>

      {sidebarTab === "files" ? (
        <div className="min-h-0 flex-1 overflow-y-auto">{filesContent}</div>
      ) : sidebarTab === "checkpoints" ? (
        <div className="min-h-0 flex-1 overflow-y-auto">{checkpointsContent}</div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-2 py-1">
          <div className="space-y-0.5 text-xs text-zinc-400">
            <button
              type="button"
              className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 transition hover:bg-zinc-900 hover:text-zinc-100"
            >
              <Clock className="h-3.5 w-3.5 text-zinc-400" />
              <span>Automations</span>
            </button>
            <button
              type="button"
              className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 transition hover:bg-zinc-900 hover:text-zinc-100"
            >
              <SlidersHorizontal className="h-3.5 w-3.5 text-zinc-400" />
              <span>Customizations</span>
            </button>
          </div>

          <div className="mt-4 border-t border-zinc-800/80 pt-3">
            <div className="mb-1 flex items-center justify-between px-2 text-[11px] font-medium text-zinc-400">
              <span>Projects</span>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  aria-label="Filter projects"
                  className="rounded p-0.5 hover:text-zinc-200"
                >
                  <Filter className="h-3 w-3" />
                </button>
                <button
                  type="button"
                  aria-label="Add project"
                  className="rounded p-0.5 hover:text-zinc-200"
                >
                  <Plus className="h-3 w-3" />
                </button>
              </div>
            </div>

            <div className="space-y-1">
              {DEFAULT_PROJECTS.map((project) => {
                const isOpen = openFolders[project.id] ?? false;
                const isCurrentProject = activeProject === project.name;

                return (
                  <div key={project.id} className="text-xs">
                    <div className="group flex items-center justify-between rounded-lg px-2 py-1 text-zinc-300 hover:bg-zinc-900">
                      <button
                        type="button"
                        onClick={() => toggleFolder(project.id)}
                        className="flex flex-1 items-center gap-2 truncate text-left"
                      >
                        <Folder className="h-3.5 w-3.5 shrink-0 text-zinc-400" />
                        <span className="truncate font-medium">{project.name}</span>
                      </button>

                      <div className="flex items-center gap-1">
                        {isCurrentProject && (
                          <Lock className="h-3 w-3 text-zinc-500" />
                        )}
                        <button
                          type="button"
                          onClick={() => toggleFolder(project.id)}
                          aria-label={`Toggle ${project.name}`}
                          className="rounded p-0.5 text-zinc-500 hover:text-zinc-200"
                        >
                          {isOpen ? (
                            <ChevronUp className="h-3 w-3" />
                          ) : (
                            <ChevronDown className="h-3 w-3" />
                          )}
                        </button>
                      </div>
                    </div>

                    {isOpen && project.tasks.length > 0 && (
                      <div className="mt-0.5 ml-4 space-y-0.5 border-l border-zinc-800 pl-1.5">
                        {project.tasks.map((task) => {
                          const isSelected = activeTask === task.title;

                          return (
                            <div
                              key={task.id}
                              className="relative"
                              onMouseEnter={() => setHoveredTask(task.id)}
                              onMouseLeave={() => setHoveredTask(null)}
                            >
                              <button
                                type="button"
                                onClick={() => onSelectTask(project.name, task.title)}
                                className={`group flex w-full items-center justify-between rounded-lg px-2 py-1 text-left text-xs transition ${
                                  isSelected
                                    ? "bg-zinc-800 font-medium text-white"
                                    : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200"
                                }`}
                              >
                                <span className="truncate pr-1">{task.title}</span>
                                <div className="flex shrink-0 items-center gap-1.5">
                                  {task.unread && (
                                    <span className="h-1.5 w-1.5 rounded-full bg-blue-500" />
                                  )}
                                  <span className="text-[10px] text-zinc-500">{task.timeAgo}</span>
                                </div>
                              </button>

                              {hoveredTask === task.id && (
                                <div className="pointer-events-none absolute left-full top-0 z-40 ml-2 w-52 rounded-xl border border-zinc-700 bg-[#18181b] p-3 text-xs shadow-2xl">
                                  <p className="font-semibold text-zinc-100">{task.title}</p>
                                  <div className="mt-1 flex items-center gap-1 text-[11px] text-zinc-400">
                                    <Folder className="h-3 w-3" />
                                    <span>{project.name}</span>
                                  </div>
                                  <div className="mt-1 flex items-center gap-1 text-[11px] text-zinc-400">
                                    <Clock className="h-3 w-3" />
                                    <span>Updated {task.updatedAt ?? "recently"}</span>
                                  </div>
                                  {task.unread && (
                                    <p className="mt-1.5 flex items-center gap-1 text-[11px] font-medium text-blue-400">
                                      <span className="h-1.5 w-1.5 rounded-full bg-blue-500" />
                                      Unread
                                    </p>
                                  )}
                                </div>
                              )}
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          <div className="mt-4 border-t border-zinc-800/80 pt-3">
            <div className="mb-1 flex items-center justify-between px-2 text-[11px] font-medium text-zinc-400">
              <span>Conversations</span>
              <button
                type="button"
                onClick={onNewConversation}
                aria-label="New conversation"
                className="rounded p-0.5 hover:text-zinc-200"
              >
                <Plus className="h-3 w-3" />
              </button>
            </div>

            <div className="space-y-0.5">
              {DEFAULT_CONVERSATIONS.map((conv) => (
                <button
                  key={conv.id}
                  type="button"
                  onClick={() => onSelectTask("General", conv.title)}
                  className="flex w-full items-center justify-between rounded-lg px-2 py-1 text-left text-xs text-zinc-400 transition hover:bg-zinc-900 hover:text-zinc-200"
                >
                  <span className="truncate pr-1">{conv.title}</span>
                  <span className="shrink-0 text-[10px] text-zinc-500">{conv.timeAgo}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      <div className="border-t border-zinc-800/80 p-2">
        <button
          type="button"
          onClick={onOpenSettings}
          className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-xs text-zinc-400 transition hover:bg-zinc-900 hover:text-zinc-100"
        >
          <Settings className="h-4 w-4 text-zinc-400" />
          <span>Settings</span>
        </button>
      </div>
    </aside>
  );
}
