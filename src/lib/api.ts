// Thin, typed wrapper around Tauri IPC. All backend work is exposed through
// custom commands (allowed by core:default), so no plugin permission scopes
// are needed on the frontend side.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CaptureMode,
  CaptureReady,
  HistoryItem,
  ImageFormat,
  Monitor,
  OverlayInfo,
  SavedInfo,
  Settings,
  ShortcutReport,
  WindowRect,
} from "./types";

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

export const api = {
  // Capture
  startCapture: (mode: CaptureMode, monitorIndex?: number) =>
    call<void>("start_capture", { mode, monitorIndex: monitorIndex ?? null }),
  cancelCapture: () => call<void>("cancel_capture"),
  revealMainWindow: () => call<void>("reveal_main_window"),
  discardCapture: (tempPath: string) => call<void>("discard_capture", { tempPath }),
  getMonitors: () => call<Monitor[]>("get_monitors"),

  // Overlay
  overlayInfo: () => call<OverlayInfo>("overlay_info"),
  overlayWindowAt: (x: number, y: number) =>
    call<WindowRect | null>("overlay_window_at", { x, y }),
  overlayFinish: (args: {
    x?: number;
    y?: number;
    w?: number;
    h?: number;
    points?: [number, number][];
    windowId?: number;
  }) =>
    call<void>("overlay_finish", {
      x: args.x ?? null,
      y: args.y ?? null,
      w: args.w ?? null,
      h: args.h ?? null,
      points: args.points ?? null,
      windowId: args.windowId ?? null,
    }),

  // Image / clipboard
  readImageB64: (path: string) => call<string>("read_image_b64", { path }),
  copyImage: (dataB64: string) => call<void>("copy_image", { dataB64 }),

  // Storage
  getSaveDir: () => call<string>("get_save_dir"),
  chooseSaveDir: () => call<string | null>("choose_save_dir"),
  saveImage: (dataB64: string, format?: ImageFormat) =>
    call<SavedInfo>("save_image", { dataB64, format: format ?? null }),
  saveImageAs: (dataB64: string, suggestedName: string) =>
    call<SavedInfo | null>("save_image_as", { dataB64, suggestedName }),

  // History
  listHistory: () => call<HistoryItem[]>("list_history"),
  getThumbnail: (path: string) => call<string>("get_thumbnail", { path }),
  deleteHistoryItem: (path: string) => call<void>("delete_history_item", { path }),
  revealInExplorer: (path: string) => call<void>("reveal_in_explorer", { path }),
  openPath: (path: string) => call<void>("open_path", { path }),

  // Settings
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),
  getShortcutReport: () => call<ShortcutReport>("get_shortcut_report"),
  openWindowsSettings: () => call<void>("open_windows_settings"),
};

// ----------------------------------------------------------------- events ---

export function onCaptureReady(cb: (payload: CaptureReady) => void): Promise<UnlistenFn> {
  return listen<CaptureReady>("capture://ready", (e) => cb(e.payload));
}

export function onCaptureError(cb: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("capture://error", (e) => cb(String(e.payload)));
}

export function onCaptureCancelled(cb: () => void): Promise<UnlistenFn> {
  return listen("capture://cancelled", () => cb());
}

export function onHistoryChanged(cb: () => void): Promise<UnlistenFn> {
  return listen("history://changed", () => cb());
}

export function onSettingsApplied(cb: (settings: Settings) => void): Promise<UnlistenFn> {
  return listen<Settings>("settings://applied", (e) => cb(e.payload));
}

export function onShortcutsUpdated(cb: (report: ShortcutReport) => void): Promise<UnlistenFn> {
  return listen<ShortcutReport>("shortcuts://updated", (e) => cb(e.payload));
}

export function onTrayOpen(cb: () => void): Promise<UnlistenFn> {
  return listen("tray://open", () => cb());
}
