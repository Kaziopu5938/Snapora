import { useCallback, useEffect, useRef, useState } from "react";
import {
  Camera,
  History as HistoryIcon,
  Home,
  Info,
  Moon,
  Settings as SettingsIcon,
  Sun,
} from "lucide-react";
import {
  api,
  onCaptureError,
  onCaptureReady,
  onHistoryChanged,
  onSettingsApplied,
  onShortcutsUpdated,
  onTrayOpen,
} from "../lib/api";
import { applyTheme, cn } from "../lib/utils";
import type { CaptureReady, HistoryItem, Settings, ShortcutReport, Toast, ToastKind } from "../lib/types";
import { AppContext, type EditorTarget, type View } from "./context";
import { ToastHost } from "../components/ui";
import { HomeView } from "./HomeView";
import { HistoryView } from "./HistoryView";
import { SettingsView } from "./SettingsView";
import { AboutView } from "./AboutView";
import { Editor } from "../editor/Editor";

const VIEW_TITLES: Record<View, { title: string; sub: string }> = {
  home: { title: "Home", sub: "Capture. Edit. Share." },
  history: { title: "Capture history", sub: "Your recent screenshots" },
  settings: { title: "Settings", sub: "Make Snapora yours" },
  about: { title: "About Snapora", sub: "Version and credits" },
  editor: { title: "Editor", sub: "Fine-tune your capture" },
};

export function MainApp() {
  const [view, setView] = useState<View>("home");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [report, setReport] = useState<ShortcutReport>({ items: [], osSnippingEnabled: null });
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [historyBusy, setHistoryBusy] = useState(false);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [editorTarget, setEditorTarget] = useState<EditorTarget | null>(null);
  const toastId = useRef(0);

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const pushToast = useCallback(
    (kind: ToastKind, title: string, message?: string, actionLabel?: string, onAction?: () => void) => {
      const id = ++toastId.current;
      setToasts((prev) => [...prev.slice(-3), { id, kind, title, message, actionLabel, onAction }]);
      window.setTimeout(() => dismissToast(id), 5200);
    },
    [dismissToast]
  );

  const refreshHistory = useCallback(async () => {
    setHistoryBusy(true);
    try {
      setHistory(await api.listHistory());
    } catch (e) {
      pushToast("error", "Could not load history", String(e));
    } finally {
      setHistoryBusy(false);
    }
  }, [pushToast]);

  const navigate = useCallback((v: View) => setView(v), []);

  const openEditor = useCallback((target: EditorTarget) => {
    setEditorTarget(target);
    setView("editor");
  }, []);

  const closeEditor = useCallback(
    (discard = false) => {
      if (editorTarget) {
        if (discard && editorTarget.isTemp) {
          api.discardCapture(editorTarget.path).catch(() => undefined);
        }
        const back = editorTarget.fromView && editorTarget.fromView !== "editor" ? editorTarget.fromView : "home";
        setView(back);
      } else {
        setView("home");
      }
      setEditorTarget(null);
      refreshHistory();
    },
    [editorTarget, refreshHistory]
  );

  const startCapture = useCallback(
    (mode: Parameters<typeof api.startCapture>[0], monitorIndex?: number) => {
      api.startCapture(mode, monitorIndex).catch((e) =>
        pushToast("error", "Capture could not start", String(e))
      );
    },
    [pushToast]
  );

  const updateSettings = useCallback(
    async (s: Settings) => {
      try {
        await api.saveSettings(s);
        setSettings(s);
        applyTheme(s.theme);
      } catch (e) {
        pushToast("error", "Could not save settings", String(e));
      }
    },
    [pushToast]
  );

  // Startup load + event wiring
  useEffect(() => {
    let alive = true;
    const unsubs: Array<Promise<() => void>> = [];

    (async () => {
      try {
        const s = await api.getSettings();
        if (alive) {
          setSettings(s);
          applyTheme(s.theme);
        }
      } catch {
        applyTheme("system");
      }
      if (alive) {
        api.getShortcutReport().then(setReport).catch(() => undefined);
        refreshHistory();
      }
    })();

    unsubs.push(
      onCaptureReady((p: CaptureReady) => {
        openEditor({
          path: p.tempPath,
          fileName: p.fileName,
          width: p.width,
          height: p.height,
          source: p.source,
          isTemp: true,
          fromView: view,
        });
        // Show the window only after React has committed the editor view, so
        // a shortcut capture never flashes the Home screen first.
        window.setTimeout(() => api.revealMainWindow().catch(() => undefined), 150);
      }),
      onCaptureError((msg) => pushToast("error", "Capture failed", msg)),
      onHistoryChanged(() => refreshHistory()),
      onShortcutsUpdated(setReport),
      onSettingsApplied((s) => {
        setSettings(s);
        applyTheme(s.theme);
      }),
      onTrayOpen(() => undefined)
    );

    return () => {
      alive = false;
      unsubs.forEach((p) => p.then((f) => f()).catch(() => undefined));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshHistory, openEditor, pushToast]);

  const isDark = document.documentElement.dataset.theme === "dark";
  const meta = VIEW_TITLES[view];
  const okCount = report.items.filter((i) => i.ok && i.enabled).length;
  const enabledCount = report.items.filter((i) => i.enabled).length;
  const shortcutWarn = report.osSnippingEnabled === true || report.items.some((i) => i.enabled && !i.ok);

  const ctx = {
    view,
    navigate,
    settings,
    updateSettings,
    shortcutReport: report,
    history,
    refreshHistory,
    historyBusy,
    toasts,
    pushToast,
    dismissToast,
    startCapture,
    openEditor,
    closeEditor,
    editorTarget,
  };

  const navItems: Array<{ id: View; label: string; icon: typeof Home }> = [
    { id: "home", label: "Home", icon: Home },
    { id: "history", label: "Capture history", icon: HistoryIcon },
    { id: "settings", label: "Settings", icon: SettingsIcon },
    { id: "about", label: "About Snapora", icon: Info },
  ];

  return (
    <AppContext.Provider value={ctx}>
      {view === "editor" && editorTarget ? (
        <Editor key={editorTarget.path} />
      ) : (
        <div className="app-shell">
          <aside className="sidebar">
            <div className="brand">
              <div className="brand-mark">
                <Camera strokeWidth={2.2} />
              </div>
              <div>
                <div className="brand-name">Snapora</div>
                <div className="brand-tag">Capture · Edit · Share</div>
              </div>
            </div>

            <div className="nav-label">Workspace</div>
            <nav style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {navItems.map((item) => {
                const Icon = item.icon;
                return (
                  <button
                    key={item.id}
                    className={cn("nav-item", view === item.id && "active")}
                    onClick={() => navigate(item.id)}
                  >
                    <Icon />
                    <span>{item.label}</span>
                    {item.id === "history" && history.length > 0 && (
                      <span className="nav-count">{history.length}</span>
                    )}
                  </button>
                );
              })}
            </nav>

            <div className="sidebar-spacer" />

            <button
              className="status-card"
              style={{ textAlign: "left", width: "100%" }}
              onClick={() => navigate("settings")}
              title="Open keyboard settings"
            >
              <div className="status-head">
                <span className={cn("status-dot", shortcutWarn && "warn")} />
                <span>Shortcut status</span>
              </div>
              <div className="status-line">
                <span>Print Screen keys</span>
                <b>
                  {enabledCount === 0 ? "Off" : `${okCount}/${enabledCount} active`}
                </b>
              </div>
              <div className="status-line">
                <span>Save format</span>
                <b>{settings?.format?.toUpperCase() ?? "PNG"}</b>
              </div>
              {shortcutWarn && (
                <div className="status-line" style={{ color: "var(--warning)", marginTop: 4 }}>
                  Review Windows conflicts
                </div>
              )}
            </button>
          </aside>

          <div className="main">
            <header className="topbar">
              <div>
                <span className="topbar-title">{meta.title}</span>
                <span className="topbar-sub">· {meta.sub}</span>
              </div>
              <div className="topbar-actions">
                <button
                  className="icon-btn"
                  title={isDark ? "Switch to light theme" : "Switch to dark theme"}
                  onClick={() => {
                    if (!settings) return;
                    const next = isDark ? "light" : "dark";
                    updateSettings({ ...settings, theme: next });
                  }}
                >
                  {isDark ? <Sun /> : <Moon />}
                </button>
                <button className="icon-btn" title="Settings" onClick={() => navigate("settings")}>
                  <SettingsIcon />
                </button>
                <button
                  className="btn btn-primary"
                  onClick={() => startCapture(settings?.defaultMode ?? "region")}
                >
                  <Camera />
                  New capture
                </button>
              </div>
            </header>

            <div className="view">
              <div className="view-inner">
                {view === "home" && <HomeView />}
                {view === "history" && <HistoryView />}
                {view === "settings" && <SettingsView />}
                {view === "about" && <AboutView />}
              </div>
            </div>
          </div>
        </div>
      )}
      <ToastHost toasts={toasts} dismiss={dismissToast} />
    </AppContext.Provider>
  );
}
