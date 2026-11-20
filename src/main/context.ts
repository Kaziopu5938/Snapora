import { createContext, useContext } from "react";
import type {
  CaptureMode,
  HistoryItem,
  Settings,
  ShortcutReport,
  Toast,
  ToastKind,
} from "../lib/types";

export type View = "home" | "history" | "settings" | "about" | "editor";

export interface EditorTarget {
  path: string;
  fileName: string;
  width: number;
  height: number;
  source: string;
  isTemp: boolean;
  fromView?: View;
}

export interface AppContextValue {
  view: View;
  navigate: (v: View) => void;
  settings: Settings | null;
  updateSettings: (s: Settings) => Promise<void>;
  shortcutReport: ShortcutReport;
  history: HistoryItem[];
  refreshHistory: () => void;
  historyBusy: boolean;
  toasts: Toast[];
  pushToast: (kind: ToastKind, title: string, message?: string, actionLabel?: string, onAction?: () => void) => void;
  dismissToast: (id: number) => void;
  startCapture: (mode: CaptureMode, monitorIndex?: number) => void;
  openEditor: (target: EditorTarget) => void;
  closeEditor: (discard?: boolean) => void;
  editorTarget: EditorTarget | null;
}

export const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used within MainApp");
  return ctx;
}
