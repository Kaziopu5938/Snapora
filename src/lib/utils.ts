import { convertFileSrc } from "@tauri-apps/api/core";
import type { CaptureMode, ImageFormat, Theme } from "./types";

export function cn(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

export function b64ToDataUrl(b64: string): string {
  return `data:image/png;base64,${b64}`;
}

export function dataUrlToB64(dataUrl: string): string {
  const i = dataUrl.indexOf(",");
  return i >= 0 ? dataUrl.slice(i + 1) : dataUrl;
}

export function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const v = bytes / Math.pow(1024, i);
  return `${v >= 10 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function formatDate(ms: number): string {
  if (!ms) return "—";
  const d = new Date(ms);
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function relativeDate(ms: number): string {
  if (!ms) return "—";
  const diff = Date.now() - ms;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "Just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return formatDate(ms);
}

export function assetUrl(path: string): string {
  return convertFileSrc(path);
}

export function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("Failed to load image"));
    img.src = src;
  });
}

export const CAPTURE_LABELS: Record<CaptureMode, string> = {
  region: "Region",
  freeform: "Freeform",
  windowSelect: "Window selection",
  activeWindow: "Active window",
  fullscreen: "Fullscreen",
  allMonitors: "All monitors",
  monitor: "Single monitor",
};

export const FORMAT_LABELS: Record<ImageFormat, string> = {
  png: "PNG — lossless",
  jpeg: "JPEG — smaller",
  webp: "WebP — modern",
};

export const THEME_LABELS: Record<Theme, string> = {
  system: "System",
  light: "Light",
  dark: "Dark",
};

export function applyTheme(theme: Theme) {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const update = () => {
    const dark = theme === "dark" || (theme === "system" && mq.matches);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  };
  update();
  mq.onchange = () => {
    if (theme === "system") update();
  };
}
