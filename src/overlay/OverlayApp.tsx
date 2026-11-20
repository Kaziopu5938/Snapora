import { useEffect, useRef, useState } from "react";
import { Command, MousePointer2, ScanLine, X } from "lucide-react";
import { api } from "../lib/api";
import { assetUrl, cn } from "../lib/utils";
import type { OverlayInfo, WindowRect } from "../lib/types";

type Point = { x: number; y: number };

export function OverlayApp() {
  const [info, setInfo] = useState<OverlayInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [start, setStart] = useState<Point | null>(null);
  const [current, setCurrent] = useState<Point | null>(null);
  const [points, setPoints] = useState<Point[]>([]);
  const [highlight, setHighlight] = useState<WindowRect | null>(null);
  const [finishing, setFinishing] = useState(false);
  const dragRef = useRef(false);
  const imgRef = useRef<HTMLImageElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.documentElement.classList.add("overlay");
    return () => document.documentElement.classList.remove("overlay");
  }, []);

  useEffect(() => {
    let alive = true;
    api.overlayInfo()
      .then((i) => alive && setInfo(i))
      .catch((e) => alive && setError(String(e)));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        api.cancelCapture().catch(() => undefined);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      alive = false;
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const scale = () => {
    const img = imgRef.current;
    const rect = img?.getBoundingClientRect();
    return {
      x: img && rect?.width ? img.naturalWidth / rect.width : 1,
      y: img && rect?.height ? img.naturalHeight / rect.height : 1,
    };
  };

  const toGlobal = (p: Point) => {
    const s = scale();
    const m = info!.monitor;
    return { x: Math.round(m.x + p.x * s.x), y: Math.round(m.y + p.y * s.y) };
  };

  const finish = (args: Parameters<typeof api.overlayFinish>[0]) => {
    if (finishing) return;
    setFinishing(true);
    api.overlayFinish(args).catch((e) => {
      setError(String(e));
      setFinishing(false);
    });
  };

  const localPos = (e: React.PointerEvent<HTMLDivElement>): Point => {
    const r = rootRef.current!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!info || finishing || e.button !== 0 || info.kind === "window") return;
    e.currentTarget.setPointerCapture(e.pointerId);
    dragRef.current = true;
    const p = localPos(e);
    setStart(p);
    setCurrent(p);
    if (info.kind === "freeform") setPoints([p]);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!info || !info.bitmapPath || !imgRef.current) return;
    const p = localPos(e);
    if (info.kind === "window") {
      const g = toGlobal(p);
      setHighlight(
        info.windows.find((w) => g.x >= w.x && g.x < w.x + w.w && g.y >= w.y && g.y < w.y + w.h) ?? null
      );
      return;
    }
    if (!dragRef.current) return;
    setCurrent(p);
    if (info.kind === "freeform") setPoints((prev) => [...prev, p]);
  };

  const onPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!dragRef.current || !info) return;
    dragRef.current = false;
    const p = localPos(e);
    setCurrent(p);

    if (info.kind === "region") {
      const a = toGlobal(start ?? p);
      const b = toGlobal(p);
      const x = Math.min(a.x, b.x);
      const y = Math.min(a.y, b.y);
      const w = Math.abs(b.x - a.x);
      const h = Math.abs(b.y - a.y);
      if (w < 4 || h < 4) { setStart(null); setCurrent(null); return; }
      finish({ x, y, w, h });
    } else if (info.kind === "freeform") {
      const all = [...points, p];
      if (all.length < 3) { setStart(null); setPoints([]); return; }
      finish({ points: all.map((pt) => { const g = toGlobal(pt); return [g.x, g.y] as [number, number]; }) });
    }
  };

  const selectWindow = () => {
    if (info?.kind === "window" && highlight) finish({ windowId: highlight.id });
  };

  const hint = info
    ? info.kind === "region"
      ? "Drag to select a region"
      : info.kind === "freeform"
        ? "Draw a freeform selection"
        : "Click a window to capture it"
    : "Preparing capture…";

  const sel = start && current ? {
    left: Math.min(start.x, current.x),
    top: Math.min(start.y, current.y),
    width: Math.abs(current.x - start.x),
    height: Math.abs(current.y - start.y),
  } : null;
  const selSize = sel ? { w: Math.round(sel.width * scale().x), h: Math.round(sel.height * scale().y) } : null;

  const hlLocal = highlight && info ? {
    left: (highlight.x - info.monitor.x) / scale().x,
    top: (highlight.y - info.monitor.y) / scale().y,
    width: highlight.w / scale().x,
    height: highlight.h / scale().y,
  } : null;

  return (
    <div
      ref={rootRef}
      className="overlay-root"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onClick={selectWindow}
      onContextMenu={(e) => { e.preventDefault(); api.cancelCapture().catch(() => undefined); }}
      style={{ cursor: info?.kind === "window" ? "default" : "crosshair" }}
    >
      {info && (
        <img
          ref={imgRef}
          className="overlay-bg"
          src={assetUrl(info.bitmapPath)}
          alt="Frozen screen"
          draggable={false}
        />
      )}
      <div className="overlay-veil" />

      <div className="overlay-hint">
        {info?.kind === "window" ? <MousePointer2 size={15} /> : info?.kind === "freeform" ? <ScanLine size={15} /> : <Command size={15} />}
        <span>{hint}</span>
        <kbd>Esc</kbd>
        <span style={{ opacity: 0.7 }}>cancel</span>
      </div>

      {info?.kind === "region" && sel && (
        <>
          <div className="selection-box" style={sel} />
          {selSize && (
            <div
              className="selection-size"
              style={{ left: sel.left, top: Math.max(0, sel.top - 26) }}
            >
              {selSize.w} × {selSize.h}
            </div>
          )}
        </>
      )}

      {info?.kind === "freeform" && points.length > 1 && (
        <svg style={{ position: "absolute", inset: 0, width: "100%", height: "100%", pointerEvents: "none" }}>
          <polygon
            points={points.map((p) => `${p.x},${p.y}`).join(" ")}
            fill="rgba(232,147,12,0.16)"
            stroke="#fff"
            strokeWidth={1.5}
            strokeDasharray="6 4"
          />
        </svg>
      )}

      {info?.kind === "window" && hlLocal && (
        <>
          <div className="window-highlight" style={hlLocal} />
          {highlight && (
            <div className="window-label" style={{ left: hlLocal.left, top: Math.max(0, hlLocal.top - 32) }}>
              {highlight.title || "Window"}
            </div>
          )}
        </>
      )}

      {(error || finishing) && (
        <div className={cn("overlay-hint")} style={{ top: "auto", bottom: 24 }}>
          {error ? <X size={15} /> : null}
          <span>{error ?? "Finalizing capture…"}</span>
        </div>
      )}
    </div>
  );
}
