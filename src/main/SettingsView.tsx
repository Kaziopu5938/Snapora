import {
  AlertTriangle,
  Check,
  ExternalLink,
  Folder,
  Image as ImageIcon,
  Keyboard,
  Monitor,
  Palette,
  Settings2,
  ShieldCheck,
  X,
} from "lucide-react";
import { api } from "../lib/api";
import { cn, FORMAT_LABELS, THEME_LABELS } from "../lib/utils";
import type { Theme } from "../lib/types";
import { useApp } from "./context";
import { Panel, ToggleRow } from "../components/ui";

const FORMATS = ["png", "jpeg", "webp"] as const;
const MODES = [
  ["region", "Region"],
  ["freeform", "Freeform"],
  ["windowSelect", "Window selection"],
  ["activeWindow", "Active window"],
  ["fullscreen", "Fullscreen"],
  ["allMonitors", "All monitors"],
] as const;

export function SettingsView() {
  const { settings, updateSettings, shortcutReport, pushToast } = useApp();
  if (!settings) return null;

  const set = <K extends keyof typeof settings>(key: K, value: (typeof settings)[K]) =>
    updateSettings({ ...settings, [key]: value });

  const browse = async () => {
    try {
      const dir = await api.chooseSaveDir();
      if (dir) set("saveDir", dir);
    } catch (e) {
      pushToast("error", "Could not choose folder", String(e));
    }
  };

  const conflicts = shortcutReport.items.filter((i) => i.enabled && !i.ok);
  const snipping = shortcutReport.osSnippingEnabled;
  const anyProblem = conflicts.length > 0 || snipping === true;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      {anyProblem && (
        <div className="note warn">
          <AlertTriangle />
          <span>
            <b>Print Screen conflict detected.</b> Windows may hand the key to Snipping Tool before
            Snapora sees it. In Windows Settings → Ease of Access → Keyboard, turn <b>off</b> “Print
            screen key opens Snipping Tool” (Windows 11: Settings → Accessibility → Keyboard).
          </span>
        </div>
      )}

      <div className="settings-grid">
        <Panel icon={<Settings2 />} title="General">
          <ToggleRow
            title="Start with Windows"
            description="Launch Snapora automatically when you sign in."
            checked={settings.startWithWindows}
            onChange={(v) => set("startWithWindows", v)}
          />
          <ToggleRow
            title="Minimize to system tray"
            description="Closing the window keeps Snapora running in the background."
            checked={settings.minimizeToTray}
            onChange={(v) => set("minimizeToTray", v)}
          />
          <ToggleRow
            title="Launch minimized"
            description="Start directly in the tray without showing the main window."
            checked={settings.launchMinimized}
            onChange={(v) => set("launchMinimized", v)}
          />

          <div className="field" style={{ marginTop: 16, marginBottom: 0 }}>
            <label className="field-label">Default save directory</label>
            <div style={{ display: "flex", gap: 8 }}>
              <input className="input mono" readOnly value={settings.saveDir ?? "%USERPROFILE%\\Pictures\\Snapora"} />
              <button className="btn" onClick={browse} title="Choose folder"><Folder /></button>
            </div>
            <span className="field-help">Screenshots are saved here with collision-safe, date-stamped filenames.</span>
          </div>

          <div className="field" style={{ marginTop: 16, marginBottom: 0 }}>
            <label className="field-label">Default image format</label>
            <select
              className="select"
              value={settings.format}
              onChange={(e) => set("format", e.target.value as typeof settings.format)}
            >
              {FORMATS.map((f) => <option key={f} value={f}>{FORMAT_LABELS[f]}</option>)}
            </select>
          </div>
        </Panel>

        <Panel icon={<ImageIcon />} title="Capture">
          <div className="field">
            <label className="field-label">Default capture mode</label>
            <select
              className="select"
              value={settings.defaultMode}
              onChange={(e) => set("defaultMode", e.target.value as typeof settings.defaultMode)}
            >
              {MODES.map(([v, label]) => <option key={v} value={v}>{label}</option>)}
            </select>
          </div>
          <div className="field">
            <label className="field-label">Capture delay (seconds)</label>
            <input
              className="input"
              type="number"
              min={0}
              max={60}
              value={settings.delaySecs}
              onChange={(e) => set("delaySecs", Math.max(0, Math.min(60, Number(e.target.value) || 0)))}
            />
            <span className="field-help">Useful for capturing menus or timed content.</span>
          </div>
          <ToggleRow
            title="Include mouse cursor"
            description="Composite the cursor into captured screenshots."
            checked={settings.includeCursor}
            onChange={(v) => set("includeCursor", v)}
          />
          <ToggleRow
            title="Fullscreen captures all monitors"
            description="When off, fullscreen captures only the primary display."
            checked={settings.fullscreenAllMonitors}
            onChange={(v) => set("fullscreenAllMonitors", v)}
          />
        </Panel>

        <Panel icon={<Keyboard />} title="Keyboard & shortcuts">
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            <ToggleRow title="Print Screen — Region" checked={settings.hotkeys.regionEnabled} onChange={(v) => set("hotkeys", { ...settings.hotkeys, regionEnabled: v })} />
            <ToggleRow title="Alt + Print Screen — Active window" checked={settings.hotkeys.activeWindowEnabled} onChange={(v) => set("hotkeys", { ...settings.hotkeys, activeWindowEnabled: v })} />
            <ToggleRow title="Ctrl + Print Screen — Fullscreen" checked={settings.hotkeys.fullscreenEnabled} onChange={(v) => set("hotkeys", { ...settings.hotkeys, fullscreenEnabled: v })} />
            <ToggleRow title="Shift + Print Screen — Freeform" checked={settings.hotkeys.freeformEnabled} onChange={(v) => set("hotkeys", { ...settings.hotkeys, freeformEnabled: v })} />
          </div>

          <div style={{ marginTop: 14, paddingTop: 12, borderTop: "1px solid var(--border)" }}>
            <label className="field-label">Registration status</label>
            <div style={{ display: "flex", flexDirection: "column", gap: 6, marginTop: 9 }}>
              {shortcutReport.items.length === 0 && (
                <span className="field-help">Checking shortcuts…</span>
              )}
              {shortcutReport.items.map((item) => (
                <div className="status-line" key={item.id} style={{ padding: "5px 0" }}>
                  <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    {item.ok && item.enabled ? (
                      <Check size={14} color="var(--success)" />
                    ) : !item.enabled ? (
                      <X size={14} className="muted" />
                    ) : (
                      <AlertTriangle size={14} color="var(--warning)" />
                    )}
                    {item.label} <span className="mono muted" style={{ fontSize: 10.5 }}>{item.combo}</span>
                  </span>
                  <b style={{ color: !item.enabled ? "var(--text-3)" : item.ok ? "var(--success)" : "var(--warning)" }}>
                    {!item.enabled ? "Disabled" : item.ok ? "Active" : "Blocked"}
                  </b>
                </div>
              ))}
            </div>
            <div className="field-help" style={{ marginTop: 10 }}>
              Windows Snipping Tool setting:{" "}
              <b>
                {snipping === null ? "not detected" : snipping ? "enabled (may intercept)" : "disabled"}
              </b>
            </div>
            <button className="btn btn-subtle" style={{ marginTop: 10 }} onClick={() => api.openWindowsSettings().catch(() => undefined)}>
              <ExternalLink /> Open Windows keyboard settings
            </button>
          </div>
        </Panel>

        <Panel icon={<Palette />} title="Appearance">
          <label className="field-label">Theme</label>
          <div className="seg" style={{ display: "flex", width: "fit-content", marginTop: 9 }}>
            {(["light", "dark", "system"] as Theme[]).map((t) => (
              <button
                key={t}
                className={cn(settings.theme === t && "on")}
                onClick={() => set("theme", t)}
              >
                {t === "light" ? "☀" : t === "dark" ? "☾" : "⌘"} {THEME_LABELS[t]}
              </button>
            ))}
          </div>
          <p className="field-help" style={{ marginTop: 12 }}>
            “System” follows your Windows light/dark preference automatically. The window frame and
            this interface stay in sync.
          </p>

          <div className="note info" style={{ marginTop: 16 }}>
            <ShieldCheck />
            <span>
              Settings are stored locally in your app data folder and restored on every launch.
            </span>
          </div>

          <div style={{ marginTop: 16, display: "flex", alignItems: "center", gap: 8, color: "var(--text-3)", fontSize: 11.5 }}>
            <Monitor size={14} /> Changes apply immediately.
          </div>
        </Panel>
      </div>
    </div>
  );
}
