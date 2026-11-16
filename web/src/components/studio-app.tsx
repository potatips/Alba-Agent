"use client";

import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { TeammateWorkspace } from "@/components/teammates/teammate-workspace";
import { ScheduledTasks } from "@/components/teammates/scheduled-tasks";
import { Icon } from "@/components/shell/icons";
import { LearningTransferRadar } from "@/components/learning-transfer-radar";
import { FinOpsDistillationPanel } from "@/components/finops-distillation-panel";
import { CustomizePage } from "@/components/integrations/customize-page";
import { modelLabel } from "@/lib/models.mjs";
import { ModelProvider, useModels } from "@/components/model-context";
import { AppearanceSettings } from "@/components/settings/appearance-settings";
import { ApplicationSettings } from "@/components/settings/application-settings";
import { NotificationSettings } from "@/components/settings/notification-settings";
import { usePreferences } from "@/components/settings/preferences-provider";
import { GeneralSettings } from "@/components/settings/general-settings";
import { useLocale } from "@/components/locale-provider";
import { ShortcutSettings } from "@/components/providers/shortcut-settings";
import { ProviderSettings } from "@/components/providers/provider-settings";
import { TeamEvolutionOverview } from "@/components/team-evolution-overview";
import { WorkspaceApp } from "@/components/workspace/workspace-app";
import { WorkspaceProvider, useWorkspace } from "@/components/workspace/workspace-context";
import { apiRequest, readError, tokenExpiry } from "@/lib/api";
import { getDesktop } from "@/lib/desktop";
import { actionFor, formatCombo, loadBindings, saveBindings, type Bindings } from "@/lib/shortcuts.mjs";
import { INITIAL_DOCK, dockAfter, loadProjects, type DockTab } from "@/lib/shell.mjs";
import { Sidebar } from "@/components/shell/sidebar";
import { PanelResize, usePanelSize } from "@/components/shell/panel-resize";
import { SettingsModal } from "@/components/shell/settings-modal";
import { ShellContext, type NavPage, type ShellValue } from "@/components/shell/shell-context";
import { TitleBar } from "@/components/shell/title-bar";
import { useConversations } from "@/components/workspace/use-conversations";
import { useReview } from "@/components/workspace/use-review";

type WorkspaceScope = { tenant_id: string; workspace_id: string };
export type { NavPage } from "@/components/shell/shell-context";

const emptyScope = { tenant_id: "", workspace_id: "" };

export type AuthUser = {
  sub: string;
  email: string | null;
  name: string | null;
  picture: string | null;
  tenant_id: string;
  workspace_id: string;
};

export type AuthSession = {
  authenticated: boolean;
  user: AuthUser | null;
  has_unclaimed_legacy_data: boolean;
};

function AccountDialog({
  onSessionUpdated,
  onClose,
}: {
  onSessionUpdated: () => Promise<void>;
  onClose: () => void;
}) {
  const { t } = useLocale();
  const [busy, setBusy] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState("");
  const pollRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current);
      }
    };
  }, []);

  const startGoogleSignIn = async () => {
    setBusy(true);
    setError("");
    try {
      const start = await apiRequest<{ auth_url: string; state: string; handoff_id: string }>(
        "/api/v1/auth/google/start",
        "",
        { method: "POST" }
      );
      if (typeof window !== "undefined") {
        window.open(start.auth_url, "_blank");
      }
      setWaiting(true);

      if (pollRef.current !== null) {
        window.clearInterval(pollRef.current);
      }
      pollRef.current = window.setInterval(async () => {
        try {
          const res = await apiRequest<{ status: string }>(
            "/api/v1/auth/google/handoff",
            "",
            { method: "POST", body: JSON.stringify({ handoff_id: start.handoff_id }) }
          );
          if (res && res.status === "ready") {
            if (pollRef.current !== null) {
              window.clearInterval(pollRef.current);
              pollRef.current = null;
            }
            await onSessionUpdated();
            onClose();
          }
        } catch {
          // Keep polling while waiting for callback
        }
      }, 1500);
    } catch (reason) {
      setError(readError(reason));
      setBusy(false);
      setWaiting(false);
    }
  };

  return (
    <Dialog title={t("account.open")} onClose={onClose}>
      <p className="dialog-copy">{t("account.googleDescription")}</p>
      <div style={{ margin: "16px 0 20px" }}>
        <button
          type="button"
          className="button button-primary"
          disabled={busy}
          onClick={() => void startGoogleSignIn()}
          style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: "10px", width: "100%", padding: "10px 16px" }}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
            <path fill="#EA4335" d="M12 5c1.6 0 3 .6 4.1 1.6l3.1-3.1C17.3 1.7 14.8 1 12 1 7.5 1 3.7 3.6 1.9 7.3l3.7 2.9C6.5 7.4 9 5 12 5z"/>
            <path fill="#4285F4" d="M23.5 12.3c0-.8-.1-1.7-.2-2.3H12v4.6h6.5c-.3 1.5-1.1 2.8-2.4 3.7l3.7 2.9c2.2-2 3.7-5 3.7-8.9z"/>
            <path fill="#FBBC05" d="M5.6 14.8c-.2-.7-.4-1.5-.4-2.8s.2-2.1.4-2.8L1.9 6.3C.7 8.7 0 10.3 0 12s.7 3.3 1.9 5.7l3.7-2.9z"/>
            <path fill="#34A853" d="M12 23c3.2 0 6-1.1 8-3l-3.7-2.9c-1.1.7-2.5 1.2-4.3 1.2-3 0-5.5-2-6.4-4.8L1.9 16.4C3.7 20.4 7.5 23 12 23z"/>
          </svg>
          <span style={{ fontWeight: 600 }}>{t("account.signInWithGoogle")}</span>
        </button>
        {waiting && (
          <p style={{ fontSize: "12px", color: "var(--color-fg-muted, #5f6368)", margin: "12px 0 0", textAlign: "center" }}>
            {t("account.waitingGoogle")}
          </p>
        )}
      </div>
      {error && <p className="inline-error" role="alert">{error}</p>}
      <div className="dialog-actions">
        <button className="button button-quiet" onClick={onClose}>{t("common.close")}</button>
      </div>
    </Dialog>
  );
}

function Dialog({ title, children, onClose, onSubmit }: { title: string; children: React.ReactNode; onClose: () => void; onSubmit?: () => void }) {
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}><section className="modal-card" role="dialog" aria-modal="true" aria-label={title} onKeyDown={(event) => { if (onSubmit && event.key === "Enter" && event.target instanceof HTMLInputElement) { event.preventDefault(); onSubmit(); } }}><header className="modal-head"><div><span className="eyebrow">Alba Agent</span><h2>{title}</h2></div><button className="icon-button" onClick={onClose} aria-label="Close dialog">×</button></header>{children}</section></div>;
}

function IntegrationDialog({ kind, token, onClose }: { kind: "api-key" | "widget"; token: string; onClose: () => void }) {
  const [key, setKey] = useState(""); const [error, setError] = useState(""); const [busy, setBusy] = useState(false); const [copied, setCopied] = useState(false);
  const [origin, setOrigin] = useState("https://studio.example.com");
  useEffect(() => { setOrigin(window.location.origin); }, []);
  const code = kind === "widget" ? `<iframe\n  src="${origin}/embed/workflow-id"\n  title="Alba Agent workflow"\n  width="100%"\n  height="640"\n></iframe>` : "";
  const generate = async () => {
    setBusy(true); setError("");
    try { const result = await apiRequest<{ key: string }>("/api/v1/auth/keys", token, { method: "POST", body: JSON.stringify({ scopes: ["read", "write", "run"] }) }); setKey(result.key); }
    catch (reason) { setError(readError(reason)); }
    finally { setBusy(false); }
  };
  const copy = async (value: string) => { await navigator.clipboard.writeText(value); setCopied(true); window.setTimeout(() => setCopied(false), 1800); };
  return <Dialog title={kind === "api-key" ? "BaaS API access" : "Embed a workflow"} onClose={onClose}>{kind === "api-key" ? <><p className="dialog-copy">Create a scoped key for server-side workflow access. This secret is shown once; keep it on a trusted server.</p>{key ? <><div className="key-reveal"><code>{key}</code><button className="button button-small" onClick={() => void copy(key)}>{copied ? "Copied" : "Copy key"}</button></div><pre className="code-sample">{`curl -X POST "$NEXUS_API_URL/api/v1/workflows/$WORKFLOW_ID/runs" \\\n  -H "X-Nexus-API-Key: $NEXUS_SERVICE_KEY" \\\n  -H "X-Nexus-Tenant-ID: $NEXUS_TENANT_ID" \\\n  -H "X-Nexus-Workspace-ID: $NEXUS_WORKSPACE_ID" \\\n  -H "Content-Type: application/json" \\\n  -d '{"input_data":{"request":"Summarize this account"}}'`}</pre></> : <button className="button button-primary" disabled={!token || busy} onClick={() => void generate()}>{busy ? "Creating key…" : "Create service key"}</button>}{error && <p className="inline-error">{error}</p>}</> : <><p className="dialog-copy">Use this iframe in an authenticated workspace surface. Your host application should establish the user session before embedding.</p><pre className="code-sample">{code}</pre><div className="dialog-actions"><span className="muted-copy">Replace workflow-id with a published workflow.</span><button className="button button-primary" onClick={() => void copy(code)}>{copied ? "Copied" : "Copy embed code"}</button></div></>}</Dialog>;
}


const pagePaths: Record<NavPage, string> = {
  studio: "/studio",
  general: "/settings",
  application: "/settings/application",
  appearance: "/settings/appearance",
  shortcuts: "/settings/shortcuts",
  notifications: "/settings/notifications",
  integrations: "/settings/integrations",
  settings: "/settings/providers",
  teammates: "/teammates",
  intelligence: "/intelligence",
};


// The rail starts with New chat, Automations and Customize; the profile footer owns settings.
// Integrations (channels, MCP, skills) open from Customize.
const NAV_TARGETS: Record<string, NavPage> = { "nav-studio": "studio", "nav-integrations": "integrations", "nav-settings": "general" };
// Settings is a panel with its own navigation: General, Models and Intelligence (learning metrics).
const SETTINGS_FAMILY: NavPage[] = ["general", "application", "appearance", "shortcuts", "notifications", "settings", "intelligence"]; // shown as a panel over the work; Teammates (Automations) is a full page

type NavIcon = "studio" | "newSession" | "customize" | "general" | "models" | "teammates" | "intelligence" | "back" | "shortcuts" | "logout";

function NavGlyph({ name }: { name: NavIcon }) {
  const paths: Record<typeof name, React.ReactNode> = {
    studio: <><path d="M4 5.5h16v10.5H9l-4.5 3.5V5.5z" /><path d="M8.5 10h7" /></>,
    general: <><path d="M10.54 5.15L10.69 2.69 L13.31 2.69 L13.46 5.15A7 7 0 0 1 15.81 6.13L17.66 4.49 L19.51 6.34 L17.87 8.19A7 7 0 0 1 18.85 10.54L21.31 10.69 L21.31 13.31 L18.85 13.46A7 7 0 0 1 17.87 15.81L19.51 17.66 L17.66 19.51 L15.81 17.87A7 7 0 0 1 13.46 18.85L13.31 21.31 L10.69 21.31 L10.54 18.85A7 7 0 0 1 8.19 17.87L6.34 19.51 L4.49 17.66 L6.13 15.81A7 7 0 0 1 5.15 13.46L2.69 13.31 L2.69 10.69 L5.15 10.54A7 7 0 0 1 6.13 8.19L4.49 6.34 L6.34 4.49 L8.19 6.13A7 7 0 0 1 10.54 5.15Z" /><circle cx="12" cy="12" r="3" /></>,
    models: <path d="M12 3 20 7.5v9L12 21 4 16.5v-9zM12 12l8-4.5M12 12 4 7.5M12 12v9" />,
    teammates: <><circle cx="9" cy="8.5" r="3" /><path d="M3.5 19c.5-3 3-4.8 5.5-4.8s5 1.8 5.5 4.8M16 6a2.6 2.6 0 0 1 0 5M17.5 14.4c1.9.5 3 2.1 3.3 4.6" /></>,
    intelligence: <path d="M4 19V5M4 19h16M8 15v-4M12 15V8M16 15v-6" />,
    back: <path d="M19 12H5M11 6l-6 6 6 6" />,
    shortcuts: <><rect x="3.5" y="6" width="17" height="12" rx="2.5" /><path d="M7.5 10h.01M12 10h.01M16.5 10h.01M8 14h8" /></>,
    logout: <path d="M10 4.5H6.5a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2H10M9 12h11M16 8l4 4-4 4" />,
    newSession: <path d="M4 12 20 4l-4 16-4.5-6.5z" />,
    customize: <><rect x="4" y="4" width="6.5" height="6.5" rx="1.5" /><rect x="4" y="13.5" width="6.5" height="6.5" rx="1.5" /><rect x="13.5" y="13.5" width="6.5" height="6.5" rx="1.5" /><path d="M16.75 4v6.5M13.5 7.25H20" /></>,
  };
  return <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

function ShortcutsDialog({ bindings, onChange, mac, onClose }: { bindings: Bindings; onChange: (next: Bindings) => void; mac: boolean; onClose: () => void }) {
  const { t } = useLocale();
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <section className="modal-card help-modal" role="dialog" aria-modal="true" aria-label={t("shell.help")}>
      <header className="modal-head"><div><span className="eyebrow">{t("shell.helpEyebrow")}</span><h2>{t("shell.shortcutsTitle")}</h2></div><button className="icon-button" aria-label={t("common.close")} onClick={onClose}>×</button></header>
      <ShortcutSettings bindings={bindings} onChange={onChange} mac={mac} />
    </section>
  </div>;
}

function CommandPalette({ onClose, onNavigate, hint, onConnect, extra }: { onClose: () => void; onNavigate: (page: NavPage) => void; hint: (action: string) => string; onConnect: (() => void) | null; extra: Array<{ label: string; hint: string; run: () => void }> }) {
  const { t } = useLocale();
  const commands: Array<{ label: string; hint: string; run: () => void }> = [
    { label: t("palette.work"), hint: hint("nav-studio"), run: () => onNavigate("studio") },
    { label: t("page.integrations"), hint: hint("nav-integrations"), run: () => onNavigate("integrations") },
    { label: t("page.settings"), hint: hint("nav-settings"), run: () => onNavigate("general") },
    { label: t("shell.automations"), hint: t("palette.advanced"), run: () => onNavigate("teammates") },
    { label: t("palette.intelligence"), hint: t("palette.advanced"), run: () => onNavigate("intelligence") },
    ...(onConnect ? [{ label: t("palette.signIn"), hint: t("palette.account"), run: onConnect }] : []),
    ...extra,
  ];
  const [query, setQuery] = useState("");
  const filtered = commands.filter((command) => command.label.toLowerCase().includes(query.toLowerCase())).slice(0, 14);
  return <div className="command-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}><section className="command-palette" role="dialog" aria-modal="true" aria-label={t("shell.search")}><label className="command-search"><span aria-hidden="true">⌕</span><input autoFocus value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("palette.placeholder")} /></label><div className="command-list">{filtered.map((command) => <button key={command.label} onClick={() => { command.run(); onClose(); }}><span>{command.label}</span><kbd>{command.hint}</kbd></button>)}{filtered.length === 0 && <p>{t("palette.empty")}</p>}</div><footer><span>{t("palette.footer")}</span><kbd>ESC</kbd></footer></section></div>;
}

function EvolutionHost({ token, onOpened }: { token: string; onOpened: () => void }) {
  const workspace = useWorkspace();
  return <TeamEvolutionOverview token={token} onOpen={(id) => { workspace.openWorkflow(id); onOpened(); }} />;
}

function TeammatesHost({ token, scope, onConnect }: { token: string; scope: WorkspaceScope; onConnect: () => void }) {
  const workspace = useWorkspace();
  const { t } = useLocale();
  const [manage, setManage] = useState(false);
  return manage ? <div className="nx-agent-settings"><button type="button" className="button button-quiet" onClick={() => setManage(false)}><Icon name="back" size={14} />{t("interface.scheduledTasks")}</button><TeammateWorkspace token={token} scope={scope} incomingApprovals={workspace.run.approvals} onApprovalResolved={workspace.dismissApproval} onConnect={onConnect} /></div> : <ScheduledTasks token={token} scope={scope} onManageAgents={() => setManage(true)} />;
}

/** One profile entry owns account, model, language and application settings. */
function AccountRow({
  session,
  onRefreshSession,
  onSettings,
  onProviders,
  onTools,
  onShortcuts,
  onLogout,
  onSwitch,
}: {
  session: AuthSession | null;
  onRefreshSession: () => Promise<void>;
  onSettings: () => void;
  onProviders: () => void;
  onTools: () => void;
  onShortcuts: () => void;
  onLogout: (() => void) | null;
  onSwitch: (() => void) | null;
}) {
  const { t, locale, setLocale } = useLocale();
  const { selected } = useModels();
  const [open, setOpen] = useState(false);
  const [claiming, setClaiming] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const away = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        trigger.current?.focus();
      }
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  const authenticated = Boolean(session?.authenticated && session?.user);
  const user = session?.user ?? null;
  const label = authenticated
    ? user?.name || user?.email || t("account.googleAccount")
    : t("account.signedOut");
  const choose = (run: () => void) => () => {
    setOpen(false);
    run();
  };
  const item = (icon: NavIcon, text: string, run: () => void) => (
    <button type="button" className="account-menu__item" onClick={choose(run)}>
      <NavGlyph name={icon} />
      <span>{text}</span>
    </button>
  );

  const claimLegacy = async (claim: boolean) => {
    setClaiming(true);
    try {
      await apiRequest("/api/v1/auth/legacy-data/claim", "", {
        method: "POST",
        body: JSON.stringify({ claim }),
      });
      await onRefreshSession();
    } catch {
      // ignore
    } finally {
      setClaiming(false);
    }
  };

  return (
    <div className="account-wrap" ref={root}>
      {open && (
        <div className="account-menu ag-profile-menu" role="dialog" aria-label={t("profile.menu")}>
          <header>
            <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
              <strong>{label}</strong>
              {authenticated && user?.email && user.email !== label && (
                <span style={{ fontSize: "11px", color: "var(--color-fg-muted, #70757a)" }}>{user.email}</span>
              )}
            </div>
            <small>{t("profile.activeAI")}: {selected ? modelLabel(selected) : t("profile.noModel")}</small>
          </header>
          {session?.has_unclaimed_legacy_data && authenticated && (
            <div style={{ padding: "8px 10px", margin: "4px 8px 8px", background: "rgba(66, 133, 244, 0.08)", borderRadius: "6px", fontSize: "11px" }}>
              <p style={{ margin: "0 0 6px" }}>{t("account.legacyPrompt")}</p>
              <div style={{ display: "flex", gap: "6px" }}>
                <button
                  type="button"
                  className="button button-primary"
                  style={{ fontSize: "11px", padding: "3px 8px" }}
                  disabled={claiming}
                  onClick={() => void claimLegacy(true)}
                >
                  {claiming ? t("account.claimingLegacy") : t("account.claimLegacy")}
                </button>
                <button
                  type="button"
                  className="button button-quiet"
                  style={{ fontSize: "11px", padding: "3px 8px" }}
                  disabled={claiming}
                  onClick={() => void claimLegacy(false)}
                >
                  {t("account.deferLegacy")}
                </button>
              </div>
            </div>
          )}
          {authenticated ? (
            <>
              {item("models", t("profile.providers"), onProviders)}
              {item("customize", t("profile.tools"), onTools)}
              <div className="ag-profile-language">
                <span>{t("profile.language")}</span>
                <div role="group" aria-label={t("profile.language")}>
                  <button type="button" aria-pressed={locale === "vi"} onClick={() => setLocale("vi")}>Tiếng Việt</button>
                  <button type="button" aria-pressed={locale === "en"} onClick={() => setLocale("en")}>English</button>
                </div>
              </div>
              <div className="account-menu__rule" />
              {item("shortcuts", t("shell.shortcutsTitle"), onShortcuts)}
              {item("general", t("profile.general"), onSettings)}
              {onSwitch && item("teammates", t("profile.switchAccount"), onSwitch)}
              {onLogout && item("logout", t("account.menu.logout"), onLogout)}
            </>
          ) : (
            <>
              {onSwitch && (
                <div style={{ padding: "8px 12px" }}>
                  <button
                    type="button"
                    className="button button-primary"
                    style={{ width: "100%", justifyContent: "center" }}
                    onClick={choose(onSwitch)}
                  >
                    {t("account.signInWithGoogle")}
                  </button>
                </div>
              )}
              <div className="account-menu__rule" />
              <div className="ag-profile-language">
                <span>{t("profile.language")}</span>
                <div role="group" aria-label={t("profile.language")}>
                  <button type="button" aria-pressed={locale === "vi"} onClick={() => setLocale("vi")}>Tiếng Việt</button>
                  <button type="button" aria-pressed={locale === "en"} onClick={() => setLocale("en")}>English</button>
                </div>
              </div>
              {item("shortcuts", t("shell.shortcutsTitle"), onShortcuts)}
              {item("general", t("profile.general"), onSettings)}
            </>
          )}
        </div>
      )}
      <button
        ref={trigger}
        type="button"
        className={`account-row ${open ? "is-active" : ""}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        title={t("profile.menu")}
        aria-label={t("profile.menu")}
        onClick={() => setOpen(!open)}
      >
        <span className="account-avatar" aria-hidden="true" style={{ overflow: "hidden", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
          {authenticated && user?.picture ? (
            <span style={{ width: "100%", height: "100%", borderRadius: "50%", backgroundImage: `url(${user.picture})`, backgroundSize: "cover", backgroundPosition: "center", display: "inline-block" }} />
          ) : (
            authenticated ? label.charAt(0).toUpperCase() : "G"
          )}
        </span>
        <span className="ag-profile-identity">
          <strong>{label}</strong>
          <small>
            <span className={`run-dot run-dot--${authenticated ? "completed" : "idle"}`} />
            {authenticated ? t("account.googleAccount") : t("account.signedOut")}
          </small>
        </span>
      </button>
    </div>
  );
}

type LocalSession = { access_token: string; tenant_id: string; workspace_id: string; expires_in: number };

export function StudioApp({ initialPage = "studio" }: { initialPage?: NavPage }) {
  const { t } = useLocale();
  const [page, setPage] = useState<NavPage>(initialPage);
  const [workPage, setWorkPage] = useState<NavPage>(SETTINGS_FAMILY.includes(initialPage) ? "studio" : initialPage);
  useEffect(() => { document.title = `${t(`page.${page}`)} · Alba Agent`; }, [page, t]);
  const [token, setToken] = useState("");
  const [scope, setScope] = useState<WorkspaceScope>(emptyScope);
  const [localMode, setLocalMode] = useState<boolean | null>(null); // null while the server is asked
  const [accountOpen, setAccountOpen] = useState(false);
  const [session, setSession] = useState<AuthSession | null>(null);
  const [modal, setModal] = useState<"api-key" | "widget" | null>(null);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const { preferences, update: updatePreferences } = usePreferences();
  const theme = preferences.theme;
  const setTheme = useCallback((theme: "dark" | "light" | "system") => updatePreferences({ theme }), [updatePreferences]);
  const [bindings, setBindings] = useState<Bindings>(() => loadBindings({ getItem: () => null }));
  const [mac, setMac] = useState(false);

  const navigate = useCallback((next: NavPage) => {
    setPage(next);
    if (!SETTINGS_FAMILY.includes(next)) setWorkPage(next);
    setMobileNavOpen(false);
    if (typeof window !== "undefined" && window.location.pathname !== pagePaths[next]) window.history.pushState({}, "", pagePaths[next]);
  }, []);

  const saveToken = useCallback((value: string) => { setToken(value); window.localStorage.setItem("nexus_access_token", value); }, []);
  const saveScope = useCallback((value: WorkspaceScope) => { setScope(value); window.localStorage.setItem("nexus_tenant_id", value.tenant_id); window.localStorage.setItem("nexus_workspace_id", value.workspace_id); }, []);

  const refreshSession = useCallback(async () => {
    try {
      const data = await apiRequest<AuthSession>("/api/v1/auth/session", token);
      setSession(data);
      if (data.authenticated && data.user) {
        saveScope({ tenant_id: data.user.tenant_id, workspace_id: data.user.workspace_id });
        setAccountOpen(false);
      } else {
        setAccountOpen(true);
      }
    } catch {
      setSession({ authenticated: false, user: null, has_unclaimed_legacy_data: false });
      setAccountOpen(true);
    }
  }, [token, saveScope]);

  useEffect(() => {
    void refreshSession();
  }, [refreshSession]);

  useEffect(() => {
    setPage(initialPage);
    if (!SETTINGS_FAMILY.includes(initialPage)) setWorkPage(initialPage);
    setBindings(loadBindings(window.localStorage));
    try { setCollapsed(window.localStorage.getItem("nexus_sidebar_collapsed") === "1"); } catch { /* storage can be blocked; the sidebar then starts open */ }
    setMac(/Mac|iPhone|iPad/.test(window.navigator.platform));
    const desktop = getDesktop();
    if (desktop) document.documentElement.dataset.desktop = desktop.platform; // lets the CSS leave room for native window controls
    const modalEvent = (event: Event) => setModal((event as CustomEvent<"api-key" | "widget">).detail);
    const back = () => {
      const match = (Object.entries(pagePaths) as Array<[NavPage, string]>).find(([, href]) => href === window.location.pathname);
      if (match) { setPage(match[0]); if (!SETTINGS_FAMILY.includes(match[0])) setWorkPage(match[0]); }
    };
    window.addEventListener("nexus:modal", modalEvent);
    window.addEventListener("popstate", back);
    return () => { window.removeEventListener("nexus:modal", modalEvent); window.removeEventListener("popstate", back); };
  }, [initialPage]);

  // Local mode: the server hands out the built-in owner session, so nothing ever asks the user to sign in.
  const mintRef = useRef<(() => Promise<void>) | null>(null);
  useEffect(() => {
    let stopped = false; let timer: number | undefined;
    const isDesktop = Boolean(getDesktop());
    if (isDesktop) {
      setLocalMode(false);
      const storedToken = window.localStorage.getItem("nexus_access_token") ?? "";
      const tenant = window.localStorage.getItem("nexus_tenant_id") ?? "";
      const workspace = window.localStorage.getItem("nexus_workspace_id") ?? "";
      const expiry = storedToken ? tokenExpiry(storedToken) : null;
      if (storedToken && (expiry === null || expiry > Date.now())) {
        setToken(storedToken);
        if (tenant || workspace) setScope({ tenant_id: tenant, workspace_id: workspace });
      } else {
        saveToken("");
        setToken("");
        setAccountOpen(true);
      }
      return;
    }
    const mint = async () => {
      const session = await apiRequest<LocalSession>("/api/v1/auth/local-session", "", { method: "POST" });
      if (stopped) return;
      saveToken(session.access_token); saveScope({ tenant_id: session.tenant_id, workspace_id: session.workspace_id });
      window.clearTimeout(timer);
      timer = window.setTimeout(() => void mint().catch(() => undefined), Math.max(60, session.expires_in - 90) * 1000);
    };
    mintRef.current = mint;
    void (async () => {
      try {
        const mode = await apiRequest<{ enabled: boolean }>("/api/v1/auth/local-mode", "");
        if (stopped) return;
        setLocalMode(mode.enabled);
        if (mode.enabled) await mint();
        else {
          const storedToken = window.localStorage.getItem("nexus_access_token") ?? "";
          const tenant = window.localStorage.getItem("nexus_tenant_id") ?? "";
          const workspace = window.localStorage.getItem("nexus_workspace_id") ?? "";
          const expiry = storedToken ? tokenExpiry(storedToken) : null;
          if (storedToken && (expiry === null || expiry > Date.now())) {
            setToken(storedToken);
            if (tenant || workspace) setScope({ tenant_id: tenant, workspace_id: workspace });
          } else {
            saveToken("");
            setToken("");
            setAccountOpen(true);
          }
        }
      } catch { if (!stopped) setLocalMode(false); }
    })();
    return () => { stopped = true; window.clearTimeout(timer); mintRef.current = null; };
  }, [saveToken, saveScope]);

  // The API client renews a lost local session on its own and announces it; adopt it so every screen uses the new token.
  useEffect(() => {
    const adopt = (event: Event) => {
      if (getDesktop()) return; // Desktop maintains authenticated user session, not anonymous local owner
      const session = (event as CustomEvent<LocalSession>).detail;
      saveToken(session.access_token); saveScope({ tenant_id: session.tenant_id, workspace_id: session.workspace_id });
    };
    window.addEventListener("nexus:session-refreshed", adopt);
    return () => window.removeEventListener("nexus:session-refreshed", adopt);
  }, [saveToken, saveScope]);

  useEffect(() => {
    if (localMode === false && !token) setAccountOpen(true); // a shared server still needs a real sign-in
  }, [localMode, token]);
  useEffect(() => {
    const expire = () => { if (mintRef.current) void mintRef.current().catch(() => undefined); else { saveToken(""); setAccountOpen(true); } };
    window.addEventListener("nexus:session-expired", expire);
    const expiry = token && localMode === false ? tokenExpiry(token) : null;
    const timer = expiry === null ? undefined : window.setTimeout(expire, Math.max(0, Math.min(expiry - Date.now(), 2_000_000_000)));
    return () => { window.removeEventListener("nexus:session-expired", expire); if (timer !== undefined) window.clearTimeout(timer); };
  }, [token, localMode, saveToken]);

  const toggleSidebar = useCallback(() => {
    if (window.innerWidth <= 800) { setMobileNavOpen((value) => !value); return; } // narrow windows use the sidebar as a drawer
    setCollapsed((value) => {
      try { window.localStorage.setItem("nexus_sidebar_collapsed", value ? "0" : "1"); } catch { /* keep the in-memory choice */ }
      return !value;
    });
  }, []);

  const [dock, dispatchDock] = useReducer(dockAfter, INITIAL_DOCK);
  const [inspector, setInspector] = useState(false);
  const showDock = useCallback((tab: DockTab) => { if (tab === "canvas") window.dispatchEvent(new CustomEvent("nexus:set-mode", { detail: "mode-canvas" })); else dispatchDock({ type: "show", tab }); }, []);
  const toggleInspector = useCallback(() => { setInspector((value) => !value); window.dispatchEvent(new CustomEvent("nexus:set-mode", { detail: "mode-canvas" })); }, []);
  const closeSettings = useCallback(() => navigate(workPage), [navigate, workPage]);

  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if (event.key === "Escape") { setCommandOpen(false); setMobileNavOpen(false); setAccountOpen(false); setHelpOpen(false); return; }
      if (event.target instanceof HTMLElement && event.target.dataset.shortcutCapture) return;
      const action = actionFor(bindings, event);
      if (!action) return;
      event.preventDefault();
      if (action === "palette") setCommandOpen(true);
      else if (action === "new-conversation") window.dispatchEvent(new Event("nexus:new-conversation"));
      else if (action === "open-terminal") window.dispatchEvent(new Event("nexus:open-terminal"));
      else if (action === "toggle-sidebar") toggleSidebar();
      else if (action === "run") window.dispatchEvent(new CustomEvent("nexus:run"));
      else if (action === "toggle-canvas") window.dispatchEvent(new CustomEvent("nexus:toggle-canvas"));
      else if (action === "toggle-dock") dispatchDock({ type: "toggle" });
      else if (action === "toggle-inspector") toggleInspector();
      else if (action === "focus-composer") { if (page !== "studio") navigate("studio"); window.setTimeout(() => window.dispatchEvent(new CustomEvent("nexus:focus-composer")), 0); }
      else if (action.startsWith("mode-") || action === "toggle-workbench") window.dispatchEvent(new CustomEvent("nexus:set-mode", { detail: action }));
      else if (NAV_TARGETS[action]) navigate(NAV_TARGETS[action]);
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  }, [bindings, navigate, page, toggleSidebar, toggleInspector]);

  const changeBindings = useCallback((next: Bindings) => { setBindings(next); saveBindings(window.localStorage, next); }, []);
  const hint = useCallback((action: string) => formatCombo(bindings[action] ?? "", mac), [bindings, mac]);
  const currentScope = useMemo(() => scope, [scope]);
  const connect = useCallback(async () => {
    if (session?.authenticated) {
      try { await apiRequest("/api/v1/auth/sign-out", "", { method: "POST" }); } catch { /* ignore */ }
      await refreshSession();
    }
    setAccountOpen(true);
  }, [session, refreshSession]);

  const logout = useCallback(async () => {
    try { await apiRequest("/api/v1/auth/sign-out", "", { method: "POST" }); } catch { /* ignore */ }
    saveToken("");
    try { window.localStorage.removeItem("nexus_user_plan"); } catch { /* ignore */ }
    await refreshSession();
    setAccountOpen(true);
  }, [refreshSession, saveToken]);

  return <ModelProvider token={token}><WorkspaceProvider token={token}>
    <Frame page={page} workPage={workPage} navigate={navigate} collapsed={collapsed} mobileNavOpen={mobileNavOpen} closeNav={() => setMobileNavOpen(false)} toggleSidebar={toggleSidebar}
      dock={dock} dispatchDock={dispatchDock} showDock={showDock} inspector={inspector} toggleInspector={toggleInspector} hint={hint}
      openPalette={() => setCommandOpen(true)} commandOpen={commandOpen} closePalette={() => setCommandOpen(false)} connect={connect}
      token={token} scope={currentScope} accountOpen={accountOpen} setAccountOpen={setAccountOpen} saveScope={saveScope} saveToken={saveToken}
      modal={modal} setModal={setModal} helpOpen={helpOpen} setHelpOpen={setHelpOpen} bindings={bindings} changeBindings={changeBindings} mac={mac}
      theme={theme} setTheme={setTheme} onLogout={logout} closeSettings={closeSettings} session={session} refreshSession={refreshSession} />
  </WorkspaceProvider></ModelProvider>;
}

type FrameProps = {
  page: NavPage; workPage: NavPage; navigate: (page: NavPage) => void; collapsed: boolean; mobileNavOpen: boolean; closeNav: () => void; toggleSidebar: () => void;
  dock: ShellValue["dock"]; dispatchDock: (action: Parameters<typeof dockAfter>[1]) => void; showDock: (tab: DockTab) => void; inspector: boolean; toggleInspector: () => void;
  hint: (action: string) => string; openPalette: () => void; commandOpen: boolean; closePalette: () => void; connect: (() => void) | null;
  token: string; scope: WorkspaceScope; accountOpen: boolean; setAccountOpen: (open: boolean) => void; saveScope: (value: WorkspaceScope) => void; saveToken: (value: string) => void;
  modal: "api-key" | "widget" | null; setModal: (kind: "api-key" | "widget" | null) => void; helpOpen: boolean; setHelpOpen: (open: boolean) => void;
  bindings: Bindings; changeBindings: (next: Bindings) => void; mac: boolean; theme: "dark" | "light" | "system"; setTheme: (value: "dark" | "light" | "system") => void;
  onLogout: (() => void) | null; closeSettings: () => void; session: AuthSession | null; refreshSession: () => Promise<void>;
};

/** The window: title bar on top; sidebar, the work and the right dock below; settings as a panel over everything. Lives inside the workspace providers. */
function Frame(props: FrameProps) {
  const [sidebarWidth, resizeSidebar] = usePanelSize("nexus_sidebar_width", 240, 200, 360);
  const { t } = useLocale();
  const { page, navigate, token, scope, dock, showDock, dispatchDock, openPalette, hint, inspector, toggleInspector } = props;
  const workspace = useWorkspace();
  const { conversations, reload } = useConversations();
  const review = useReview();
  const { mode: studioMode, setMode: setStudioMode } = workspace;
  useEffect(() => {
    const change = (event: Event) => {
      const action = (event as CustomEvent<string>).detail;
      setStudioMode(action === "mode-canvas" ? "canvas" : action === "mode-editor" ? "editor" : action === "toggle-workbench" ? studioMode === "editor" ? "mission" : "editor" : "mission");
      navigate("studio");
    };
    window.addEventListener("nexus:set-mode", change);
    return () => window.removeEventListener("nexus:set-mode", change);
  }, [studioMode, setStudioMode, navigate]);
  const inSettings = SETTINGS_FAMILY.includes(page);
  const shownPage: NavPage = inSettings ? props.workPage : page;
  const toggleDock = useCallback(() => dispatchDock({ type: "toggle" }), [dispatchDock]);
  const closeDock = useCallback(() => dispatchDock({ type: "close" }), [dispatchDock]);
  const expandDock = useCallback(() => dispatchDock({ type: "expand" }), [dispatchDock]);

  const value = useMemo<ShellValue>(() => ({
    conversations,
    dock, showDock, toggleDock, closeDock, expandDock,
    navigate, openPalette, hint, review, inspector, toggleInspector,
  }), [dock, showDock, toggleDock, closeDock, expandDock, navigate, openPalette, hint, review, inspector, toggleInspector, conversations]);

  const current = conversations.find((item) => item.run.id === workspace.runId);
  const firstWords = workspace.messages.find((message) => message.role === "user")?.text ?? null;
  const [projects, setProjects] = useState<Record<string, string>>({});
  useEffect(() => { try { setProjects(loadProjects(window.localStorage)); } catch { setProjects({}); } }, [conversations]);
  const project = workspace.runId ? projects[workspace.runId] ?? null : null;
  const start = () => { workspace.newTask(); navigate("studio"); window.setTimeout(() => window.dispatchEvent(new Event("nexus:focus-composer")), 0); };
  const { newTask } = workspace;
  useEffect(() => {
    const create = () => { newTask(); navigate("studio"); window.setTimeout(() => window.dispatchEvent(new Event("nexus:focus-composer")), 0); };
    window.addEventListener("nexus:new-conversation", create);
    return () => window.removeEventListener("nexus:new-conversation", create);
  }, [newTask, navigate]);

  const palette = [
    { label: t("shell.newConversation"), hint: t("shell.conversations"), run: start },
    { label: t("dock.review"), hint: hint("toggle-dock"), run: () => showDock("review") },
    { label: t("dock.files"), hint: hint("toggle-dock"), run: () => showDock("files") },
    { label: t("dock.terminal"), hint: hint("toggle-dock"), run: () => showDock("terminal") },
    { label: t("dock.browser"), hint: hint("toggle-dock"), run: () => showDock("browser") },
    { label: t("dock.canvas"), hint: hint("toggle-canvas"), run: () => showDock("canvas") },
    ...conversations.slice(0, 8).map((item) => ({ label: item.name, hint: t("shell.conversations"), run: () => { if (item.run.workflow_id) { if (item.run.id !== workspace.runId) workspace.openRun(item.run.workflow_id, item.run.id, item.name); } else if (item.run.conversation_id) workspace.openConversation(item.run.conversation_id); navigate("studio"); } })),
  ];

  const account = (
    <AccountRow
      session={props.session}
      onRefreshSession={props.refreshSession}
      onSettings={() => navigate("general")}
      onProviders={() => navigate("settings")}
      onTools={() => navigate("integrations")}
      onShortcuts={() => navigate("shortcuts")}
      onLogout={props.onLogout}
      onSwitch={props.connect}
    />
  );
  return <ShellContext.Provider value={value}><div style={{ "--ag-sidebar-w": `${sidebarWidth}px` } as React.CSSProperties} className={`app-shell app-shell--chat ag-app nx-shell ${props.collapsed ? "is-collapsed" : ""} ${props.mobileNavOpen ? "is-nav-open" : ""}`}>
    <TitleBar page={shownPage} title={current?.name ?? firstWords ?? ""} project={project} collapsed={props.collapsed} onToggleSidebar={props.toggleSidebar} review={review} onShortcuts={() => navigate("shortcuts")} onNewConversation={start} />
    <div className="ag-body" inert={inSettings} aria-hidden={inSettings || undefined}>
      <Sidebar preferenceScope={scope.tenant_id && scope.workspace_id ? `${scope.tenant_id}:${scope.workspace_id}` : ""} page={shownPage} conversations={conversations} reload={reload} navigate={navigate} footer={account} />
      <PanelResize className="ag-sidebar-resize" label={t("layout.sidebar")} value={sidebarWidth} min={200} max={360} initial={240} onChange={resizeSidebar} />
      {props.mobileNavOpen && <button className="ag-scrim" aria-label={t("shell.closeNav")} onClick={props.closeNav} />}
      <main className="ag-main">
        {shownPage === "studio"
          ? <div className="ag-studio"><WorkspaceApp scope={scope} onModal={props.setModal} onNavigate={navigate} /></div>
          : <div className={`content-area content-area--${shownPage}`}>
            {shownPage === "integrations" && <CustomizePage token={token} onConnect={() => props.setAccountOpen(true)} />}
            {shownPage === "teammates" && <TeammatesHost token={token} scope={scope} onConnect={() => props.setAccountOpen(true)} />}
          </div>}
      </main>
    </div>
    {inSettings && <SettingsModal page={page} navigate={navigate} onClose={props.closeSettings} onShortcuts={() => navigate("shortcuts")} footer={account}>
      {page === "general" && <GeneralSettings />}
      {page === "application" && <ApplicationSettings />}
      {page === "appearance" && <AppearanceSettings />}
      {page === "shortcuts" && <ShortcutSettings bindings={props.bindings} onChange={props.changeBindings} mac={props.mac} />}
      {page === "notifications" && <NotificationSettings />}
      {page === "settings" && <ProviderSettings token={token} onNavigate={navigate} />}
      {page === "intelligence" && <><div className="page-title-row"><div><h1>{t("intelligence.title")}</h1><p>{t("intelligence.body")}</p></div></div><div className="intelligence-stack"><EvolutionHost token={token} onOpened={() => navigate("studio")} /><LearningTransferRadar token={token} /><FinOpsDistillationPanel token={token} /></div></>}
    </SettingsModal>}
    {props.accountOpen && <AccountDialog onSessionUpdated={props.refreshSession} onClose={() => props.setAccountOpen(false)} />}
    {props.modal && <IntegrationDialog kind={props.modal} token={token} onClose={() => props.setModal(null)} />}
    {props.commandOpen && <CommandPalette onClose={props.closePalette} onNavigate={navigate} hint={props.hint} onConnect={props.connect} extra={palette} />}
    {props.helpOpen && <ShortcutsDialog bindings={props.bindings} onChange={props.changeBindings} mac={props.mac} onClose={() => props.setHelpOpen(false)} />}
  </div></ShellContext.Provider>;
}
