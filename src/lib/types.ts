// Shared types mirroring the Rust serde shapes in src-tauri/src.

export type CaptureMode =
  | "region"
  | "freeform"
  | "windowSelect"
  | "activeWindow"
  | "fullscreen"
  | "allMonitors"
  | "monitor";

export type ImageFormat = "png" | "jpeg" | "webp";
export type Theme = "system" | "light" | "dark";

export interface HotkeySettings {
  regionEnabled: boolean;
  activeWindowEnabled: boolean;
  fullscreenEnabled: boolean;
  freeformEnabled: boolean;
}

export interface Settings {
  startWithWindows: boolean;
  minimizeToTray: boolean;
  launchMinimized: boolean;
  saveDir: string | null;
  format: ImageFormat;
  defaultMode: CaptureMode;
  delaySecs: number;
  includeCursor: boolean;
  fullscreenAllMonitors: boolean;
  hotkeys: HotkeySettings;
  theme: Theme;
}

export interface ShortcutStatus {
  id: string;
  label: string;
  combo: string;
  enabled: boolean;
  ok: boolean;
  error: string | null;
}

export interface ShortcutReport {
  items: ShortcutStatus[];
  osSnippingEnabled: boolean | null;
}

export interface HistoryItem {
  fileName: string;
  path: string;
  sizeBytes: number;
  modifiedMs: number;
  width: number;
  height: number;
}

export interface SavedInfo {
  path: string;
  fileName: string;
  dir: string;
}

export interface CaptureReady {
  tempPath: string;
  fileName: string;
  width: number;
  height: number;
  source: string;
}

export interface Monitor {
  index: number;
  x: number;
  y: number;
  width: number;
  height: number;
  dpi: number;
  primary: boolean;
}

export interface WindowRect {
  id: number;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface OverlayInfo {
  kind: "region" | "freeform" | "window";
  monitor: Monitor;
  bitmapPath: string;
  windows: WindowRect[];
}

export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
  actionLabel?: string;
  onAction?: () => void;
}
