import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowUpRight,
  Circle,
  Copy,
  Crop,
  Droplets,
  FlipHorizontal2,
  FlipVertical2,
  FolderOpen,
  Grid2X2,
  Highlighter,
  Minus,
  Pencil,
  Redo2,
  RotateCcw,
  RotateCw,
  Save,
  Square,
  Type,
  Undo2,
} from "lucide-react";
import { api } from "../lib/api";
import { b64ToDataUrl, cn, dataUrlToB64, loadImage } from "../lib/utils";
import { useApp } from "../main/context";

type Tool =
  | "crop"
  | "pen"
  | "highlight"
  | "text"
  | "arrow"
  | "line"
  | "rect"
  | "ellipse"
  | "blur"
  | "pixelate";

type Point = { x: number; y: number };
type Rect = { x: number; y: number; w: number; h: number };

const TOOLS: Array<{ id: Tool; label: string; icon: typeof Crop; sep?: boolean }> = [
  { id: "crop", label: "Crop", icon: Crop },
  { id: "pen", label: "Freehand pen", icon: Pencil },
  { id: "highlight", label: "Highlighter", icon: Highlighter },
  { id: "text", label: "Text", icon: Type },
  { id: "arrow", label: "Arrow", icon: ArrowUpRight },
  { id: "line", label: "Line", icon: Minus },
  { id: "rect", label: "Rectangle", icon: Square },
  { id: "ellipse", label: "Ellipse", icon: Circle },
  { id: "blur", label: "Blur", icon: Droplets, sep: true },
  { id: "pixelate", label: "Pixelate", icon: Grid2X2 },
];

const COLORS = [
  "#e5484d", "#f5a623", "#f5d327", "#46a758", "#3b82f6", "#8b5cf6",
  "#ec4899", "#ffffff", "#16191f", "#0891b2",
];

function normalize(a: Point, b: Point): Rect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    w: Math.abs(b.x - a.x),
    h: Math.abs(b.y - a.y),
  };
}

function boxBlur(data: Uint8ClampedArray, w: number, h: number, radius: number) {
  const tmp = new Uint8ClampedArray(data.length);
  const r = Math.max(1, Math.min(radius, 24));
  for (let y = 0; y < h; y++) {
    const px = [new Float64Array(w + 1), new Float64Array(w + 1), new Float64Array(w + 1)];
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4;
      px[0][x + 1] = px[0][x] + data[i];
      px[1][x + 1] = px[1][x] + data[i + 1];
      px[2][x + 1] = px[2][x] + data[i + 2];
    }
    for (let x = 0; x < w; x++) {
      const x0 = Math.max(0, x - r);
      const x1 = Math.min(w, x + r + 1);
      const n = x1 - x0;
      const i = (y * w + x) * 4;
      tmp[i] = (px[0][x1] - px[0][x0]) / n;
      tmp[i + 1] = (px[1][x1] - px[1][x0]) / n;
      tmp[i + 2] = (px[2][x1] - px[2][x0]) / n;
      tmp[i + 3] = data[i + 3];
    }
  }
  for (let x = 0; x < w; x++) {
    const py = [new Float64Array(h + 1), new Float64Array(h + 1), new Float64Array(h + 1)];
    for (let y = 0; y < h; y++) {
      const i = (y * w + x) * 4;
      py[0][y + 1] = py[0][y] + tmp[i];
      py[1][y + 1] = py[1][y] + tmp[i + 1];
      py[2][y + 1] = py[2][y] + tmp[i + 2];
    }
    for (let y = 0; y < h; y++) {
      const y0 = Math.max(0, y - r);
      const y1 = Math.min(h, y + r + 1);
      const n = y1 - y0;
      const i = (y * w + x) * 4;
      data[i] = (py[0][y1] - py[0][y0]) / n;
      data[i + 1] = (py[1][y1] - py[1][y0]) / n;
      data[i + 2] = (py[2][y1] - py[2][y0]) / n;
    }
  }
}

function pixelate(data: Uint8ClampedArray, w: number, h: number, size: number) {
  const s = Math.max(2, Math.min(size, 64));
  for (let by = 0; by < h; by += s) {
    for (let bx = 0; bx < w; bx += s) {
      const ew = Math.min(s, w - bx);
      const eh = Math.min(s, h - by);
      let r = 0, g = 0, b = 0, n = 0;
      for (let y = by; y < by + eh; y++) {
        for (let x = bx; x < bx + ew; x++) {
          const i = (y * w + x) * 4;
          r += data[i]; g += data[i + 1]; b += data[i + 2]; n++;
        }
      }
      r /= n; g /= n; b /= n;
      for (let y = by; y < by + eh; y++) {
        for (let x = bx; x < bx + ew; x++) {
          const i = (y * w + x) * 4;
          data[i] = r; data[i + 1] = g; data[i + 2] = b;
        }
      }
    }
  }
}

export function Editor() {
  const { editorTarget: target, closeEditor, pushToast, settings } = useApp();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const snapRef = useRef<HTMLCanvasElement>(document.createElement("canvas"));
  const drawing = useRef(false);
  const start = useRef<Point>({ x: 0, y: 0 });
  const last = useRef<Point>({ x: 0, y: 0 });
  const points = useRef<Point[]>([]);
  const textRef = useRef<HTMLInputElement>(null);
  const textDone = useRef(false);

  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState("#e5484d");
  const [strokeWidth, setStrokeWidth] = useState(4);
  const [fontSize, setFontSize] = useState(30);
  const [blurRadius, setBlurRadius] = useState(8);
  const [pixelSize, setPixelSize] = useState(12);
  const [dims, setDims] = useState({ w: target?.width ?? 0, h: target?.height ?? 0 });
  const [history, setHistory] = useState<string[]>([]);
  const [index, setIndex] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [textInput, setTextInput] = useState<{ cx: number; cy: number; left: number; top: number } | null>(null);
  const [textValue, setTextValue] = useState("");

  const ctx = () => canvasRef.current!.getContext("2d")!;

  // Load the capture image into the working canvas.
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const b64 = await api.readImageB64(target!.path);
        const img = await loadImage(b64ToDataUrl(b64));
        const canvas = canvasRef.current!;
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;
        ctx().drawImage(img, 0, 0);
        if (alive) {
          setDims({ w: img.naturalWidth, h: img.naturalHeight });
          setHistory([canvas.toDataURL("image/png")]);
          setIndex(0);
          setLoading(false);
        }
      } catch (e) {
        pushToast("error", "Could not open image", String(e));
        closeEditor(false);
      }
    })();
    return () => { alive = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const restore = useCallback((dataUrl: string) => {
    const img = new Image();
    img.onload = () => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      // Snapshots may predate a crop / rotate / resize, so restore the canvas
      // dimensions too, otherwise pixels are drawn into a mismatched buffer.
      if (canvas.width !== img.naturalWidth || canvas.height !== img.naturalHeight) {
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;
      }
      const c = canvas.getContext("2d")!;
      c.clearRect(0, 0, canvas.width, canvas.height);
      c.drawImage(img, 0, 0);
      setDims({ w: img.naturalWidth, h: img.naturalHeight });
    };
    img.src = dataUrl;
  }, []);

  const commit = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const url = canvas.toDataURL("image/png");
    setHistory((prev) => {
      const next = [...prev.slice(0, index + 1), url];
      const trimmed = next.slice(-16);
      setIndex(trimmed.length - 1);
      return trimmed;
    });
  }, [index]);

  const undo = useCallback(() => {
    if (index <= 0) return;
    const next = index - 1;
    setIndex(next);
    restore(history[next]);
  }, [index, history, restore]);

  const redo = useCallback(() => {
    if (index >= history.length - 1) return;
    const next = index + 1;
    setIndex(next);
    restore(history[next]);
  }, [index, history, restore]);

  const reset = useCallback(() => {
    if (history.length === 0) return;
    setIndex(0);
    restore(history[0]);
  }, [history, restore]);

  const getB64 = useCallback(() => {
    const canvas = canvasRef.current;
    return canvas ? dataUrlToB64(canvas.toDataURL("image/png")) : "";
  }, []);

  const doCopy = useCallback(async () => {
    try {
      setBusy(true);
      await api.copyImage(getB64());
      pushToast("success", "Copied to clipboard", "Paste into Word, Paint, Discord, Teams or email.");
    } catch (e) {
      pushToast("error", "Could not copy image", String(e));
    } finally {
      setBusy(false);
    }
  }, [getB64, pushToast]);

  const doSave = useCallback(async () => {
    try {
      setBusy(true);
      const info = await api.saveImage(getB64(), settings?.format);
      pushToast("success", "Screenshot saved", info.fileName, "Open folder", () =>
        api.openPath(info.dir).catch(() => undefined)
      );
    } catch (e) {
      pushToast("error", "Could not save screenshot", String(e));
    } finally {
      setBusy(false);
    }
  }, [getB64, settings, pushToast]);

  const doSaveAs = useCallback(async () => {
    try {
      setBusy(true);
      const ext = settings?.format === "jpeg" ? "jpg" : settings?.format === "webp" ? "webp" : "png";
      const result = await api.saveImageAs(getB64(), `${target!.fileName}.${ext}`);
      if (result) pushToast("success", "Screenshot saved", result.fileName);
    } catch (e) {
      pushToast("error", "Could not save screenshot", String(e));
    } finally {
      setBusy(false);
    }
  }, [getB64, settings, target, pushToast]);

  const openFolder = useCallback(async () => {
    try {
      const dir = await api.getSaveDir();
      await api.openPath(dir);
    } catch (e) {
      pushToast("error", "Could not open folder", String(e));
    }
  }, [pushToast]);

  // Editor keyboard shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === "s") { e.preventDefault(); doSave(); return; }
      if (mod && e.key.toLowerCase() === "z") { e.preventDefault(); e.shiftKey ? redo() : undo(); return; }
      if (mod && e.key.toLowerCase() === "y") { e.preventDefault(); redo(); return; }
      if (mod && e.key.toLowerCase() === "c" && document.activeElement !== textRef.current) {
        e.preventDefault(); doCopy(); return;
      }
      if (e.key === "Escape") {
        if (drawing.current) { cancelStroke(); return; }
        if (textInput) { textDone.current = true; setTextInput(null); setTextValue(""); }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doSave, doCopy, undo, redo, textInput]);

  // ---- drawing -------------------------------------------------------------

  const getPos = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    return {
      x: (e.clientX - rect.left) * (e.currentTarget.width / rect.width),
      y: (e.clientY - rect.top) * (e.currentTarget.height / rect.height),
      left: e.clientX - rect.left,
      top: e.clientY - rect.top,
    };
  };

  const resetSnap = () => {
    const canvas = canvasRef.current!;
    const snap = snapRef.current;
    if (snap.width !== canvas.width || snap.height !== canvas.height) {
      snap.width = canvas.width;
      snap.height = canvas.height;
    }
    snap.getContext("2d")!.clearRect(0, 0, snap.width, snap.height);
    snap.getContext("2d")!.drawImage(canvas, 0, 0);
  };

  const strokeStyle = () => {
    const c = ctx();
    c.lineCap = "round";
    c.lineJoin = "round";
    c.lineWidth = strokeWidth;
    c.strokeStyle = color;
    c.fillStyle = color;
  };

  const drawSelection = (r: Rect, tint: string, dashed = true) => {
    const canvas = canvasRef.current!;
    const c = ctx();
    c.save();
    c.fillStyle = "rgba(8,12,18,0.55)";
    c.fillRect(0, 0, canvas.width, r.y);
    c.fillRect(0, r.y + r.h, canvas.width, canvas.height - r.y - r.h);
    c.fillRect(0, r.y, r.x, r.h);
    c.fillRect(r.x + r.w, r.y, canvas.width - r.x - r.w, r.h);
    if (tint) { c.fillStyle = tint; c.fillRect(r.x, r.y, r.w, r.h); }
    c.strokeStyle = "#fff";
    c.lineWidth = 1.5;
    if (dashed) c.setLineDash([6, 4]);
    c.strokeRect(r.x + 0.5, r.y + 0.5, r.w, r.h);
    c.restore();
  };

  const drawPreview = () => {
    const c = ctx();
    c.clearRect(0, 0, canvasRef.current!.width, canvasRef.current!.height);
    c.drawImage(snapRef.current, 0, 0);
    const a = start.current;
    const b = last.current;

    switch (tool) {
      case "pen":
      case "highlight": {
        if (points.current.length < 2) return;
        c.save();
        strokeStyle();
        if (tool === "highlight") {
          c.globalCompositeOperation = "multiply";
          c.globalAlpha = 0.5;
          c.lineWidth = Math.max(14, strokeWidth * 3);
        }
        c.beginPath();
        c.moveTo(points.current[0].x, points.current[0].y);
        for (let i = 1; i < points.current.length; i++) c.lineTo(points.current[i].x, points.current[i].y);
        c.stroke();
        c.restore();
        break;
      }
      case "line":
      case "arrow": {
        strokeStyle();
        c.beginPath();
        c.moveTo(a.x, a.y);
        c.lineTo(b.x, b.y);
        c.stroke();
        if (tool === "arrow") {
          const ang = Math.atan2(b.y - a.y, b.x - a.x);
          const len = Math.max(10, strokeWidth * 3.5);
          c.beginPath();
          c.moveTo(b.x, b.y);
          c.lineTo(b.x - len * Math.cos(ang - 0.45), b.y - len * Math.sin(ang - 0.45));
          c.lineTo(b.x - len * Math.cos(ang + 0.45), b.y - len * Math.sin(ang + 0.45));
          c.closePath();
          c.fill();
        }
        break;
      }
      case "rect": {
        const r = normalize(a, b);
        strokeStyle();
        c.strokeRect(r.x, r.y, r.w, r.h);
        break;
      }
      case "ellipse": {
        const r = normalize(a, b);
        strokeStyle();
        c.beginPath();
        c.ellipse(r.x + r.w / 2, r.y + r.h / 2, Math.abs(r.w / 2), Math.abs(r.h / 2), 0, 0, Math.PI * 2);
        c.stroke();
        break;
      }
      case "crop":
        drawSelection(normalize(a, b), "", true);
        break;
      case "blur":
        drawSelection(normalize(a, b), "rgba(59,130,246,0.16)", false);
        break;
      case "pixelate":
        drawSelection(normalize(a, b), "rgba(139,92,246,0.16)", false);
        break;
    }
  };

  const cancelStroke = () => {
    if (!drawing.current) return;
    drawing.current = false;
    const c = ctx();
    c.clearRect(0, 0, canvasRef.current!.width, canvasRef.current!.height);
    c.drawImage(snapRef.current, 0, 0);
  };

  const applyRegionEffect = (r: Rect) => {
    const canvas = canvasRef.current!;
    const c = ctx();
    const x = Math.max(0, Math.floor(r.x));
    const y = Math.max(0, Math.floor(r.y));
    const w = Math.min(canvas.width - x, Math.ceil(r.w));
    const h = Math.min(canvas.height - y, Math.ceil(r.h));
    if (w < 2 || h < 2) return false;
    const data = c.getImageData(x, y, w, h);
    if (tool === "blur") boxBlur(data.data, w, h, blurRadius);
    else pixelate(data.data, w, h, pixelSize);
    c.putImageData(data, x, y);
    return true;
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (loading || e.button !== 0) return;
    const p = getPos(e);
    if (tool === "text") {
      textDone.current = false;
      setTextInput({ cx: p.x, cy: p.y, left: p.left, top: p.top });
      setTextValue("");
      window.setTimeout(() => textRef.current?.focus(), 0);
      return;
    }
    e.currentTarget.setPointerCapture(e.pointerId);
    resetSnap();
    drawing.current = true;
    start.current = { x: p.x, y: p.y };
    last.current = { x: p.x, y: p.y };
    points.current = [{ x: p.x, y: p.y }];
    drawPreview();
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawing.current) return;
    const p = getPos(e);
    last.current = { x: p.x, y: p.y };
    if (tool === "pen" || tool === "highlight") points.current.push({ x: p.x, y: p.y });
    drawPreview();
  };

  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawing.current) return;
    const p = getPos(e);
    last.current = { x: p.x, y: p.y };
    drawPreview();
    drawing.current = false;
    const canvas = canvasRef.current!;
    const c = ctx();

    if (tool === "crop") {
      const r = normalize(start.current, p);
      if (r.w < 8 || r.h < 8) { restore(canvas.toDataURL("image/png")); return; }
      const out = document.createElement("canvas");
      out.width = Math.round(r.w);
      out.height = Math.round(r.h);
      out.getContext("2d")!.drawImage(canvas, r.x, r.y, r.w, r.h, 0, 0, r.w, r.h);
      canvas.width = out.width;
      canvas.height = out.height;
      c.clearRect(0, 0, canvas.width, canvas.height);
      c.drawImage(out, 0, 0);
      setDims({ w: out.width, h: out.height });
      commit();
      return;
    }

    if (tool === "blur" || tool === "pixelate") {
      applyRegionEffect(normalize(start.current, p));
      commit();
      return;
    }

    // All other tools already drew the final shape in drawPreview().
    if (tool === "pen" || tool === "highlight" || tool === "arrow" || tool === "line" || tool === "rect" || tool === "ellipse") {
      commit();
    }
  };

  const commitText = () => {
    if (textDone.current) return;
    textDone.current = true;
    if (!textInput || !textValue.trim()) { setTextInput(null); return; }
    const c = ctx();
    c.save();
    c.font = `600 ${fontSize}px "Segoe UI", system-ui, sans-serif`;
    c.fillStyle = color;
    c.textBaseline = "top";
    c.shadowColor = "rgba(0,0,0,0.6)";
    c.shadowBlur = 5;
    c.fillText(textValue, textInput.cx, textInput.cy);
    c.restore();
    setTextInput(null);
    setTextValue("");
    commit();
  };

  // ---- transforms ----------------------------------------------------------

  const resizeTo = (w: number, h: number) => {
    const canvas = canvasRef.current!;
    const out = document.createElement("canvas");
    out.width = Math.max(1, Math.round(w));
    out.height = Math.max(1, Math.round(h));
    out.getContext("2d")!.drawImage(canvas, 0, 0, out.width, out.height);
    canvas.width = out.width;
    canvas.height = out.height;
    ctx().drawImage(out, 0, 0);
    setDims({ w: out.width, h: out.height });
    commit();
  };

  const rotate = (dir: 1 | -1) => {
    const canvas = canvasRef.current!;
    const out = document.createElement("canvas");
    out.width = canvas.height;
    out.height = canvas.width;
    const o = out.getContext("2d")!;
    o.translate(out.width / 2, out.height / 2);
    o.rotate((dir * Math.PI) / 2);
    o.drawImage(canvas, -canvas.width / 2, -canvas.height / 2);
    canvas.width = out.width;
    canvas.height = out.height;
    ctx().drawImage(out, 0, 0);
    setDims({ w: out.width, h: out.height });
    commit();
  };

  const flip = (horizontal: boolean) => {
    const canvas = canvasRef.current!;
    const out = document.createElement("canvas");
    out.width = canvas.width;
    out.height = canvas.height;
    const o = out.getContext("2d")!;
    o.translate(horizontal ? out.width : 0, horizontal ? 0 : out.height);
    o.scale(horizontal ? -1 : 1, horizontal ? 1 : -1);
    o.drawImage(canvas, 0, 0);
    ctx().clearRect(0, 0, canvas.width, canvas.height);
    ctx().drawImage(out, 0, 0);
    commit();
  };

  const colorEnabled = ["pen", "highlight", "text", "arrow", "line", "rect", "ellipse"].includes(tool);
  const widthEnabled = ["pen", "arrow", "line", "rect", "ellipse"].includes(tool);

  return (
    <div className="editor">
      <div className="editor-top">
        <button className="icon-btn" title="Cancel and close" onClick={() => closeEditor(true)}>
          <ArrowLeft />
        </button>
        <div className="editor-file">
          <b>{target?.fileName ?? "Screenshot"}</b>
          <small>{dims.w} × {dims.h} · unsaved edits are kept in this session</small>
        </div>
        <div className="grow" />
        <div className="row" style={{ gap: 6 }}>
          <button className="icon-btn" title="Undo (Ctrl+Z)" disabled={index <= 0} onClick={undo}><Undo2 /></button>
          <button className="icon-btn" title="Redo (Ctrl+Y)" disabled={index >= history.length - 1} onClick={redo}><Redo2 /></button>
          <button className="icon-btn" title="Reset all changes" disabled={index === 0} onClick={reset}><RotateCcw /></button>
        </div>
        <div className="sep" />
        <button className="btn" onClick={doCopy} disabled={busy || loading}><Copy /> Copy</button>
        <button className="btn" onClick={doSaveAs} disabled={busy || loading}><Save /> Save as</button>
        <button className="icon-btn" title="Open save folder" onClick={openFolder}><FolderOpen /></button>
        <button className="btn btn-primary" onClick={doSave} disabled={busy || loading}><Save /> Save</button>
      </div>

      <div className="editor-body">
        <div className="toolbar">
          {TOOLS.map((t) => {
            const Icon = t.icon;
            return (
              <div key={t.id} style={{ display: "contents" }}>
                {t.sep && <div className="tool-sep" />}
                <button
                  className={cn("tool-btn", tool === t.id && "on")}
                  onClick={() => { cancelStroke(); setTool(t.id); }}
                  aria-label={t.label}
                >
                  <Icon />
                  <span className="tool-tip">{t.label}</span>
                </button>
              </div>
            );
          })}
        </div>

        <div className="canvas-area">
          <div className="canvas-wrap">
            <canvas
              ref={canvasRef}
              className={cn("editor-canvas", (tool === "crop" || tool === "blur" || tool === "pixelate") && "crop")}
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={cancelStroke}
              style={{ opacity: loading ? 0.4 : 1 }}
            />
            {textInput && (
              <div className="text-input-pop" style={{ left: textInput.left, top: textInput.top }}>
                <input
                  ref={textRef}
                  value={textValue}
                  placeholder="Type text…"
                  style={{ color, fontSize: Math.max(14, Math.min(fontSize, 26)) }}
                  onChange={(e) => setTextValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") { e.preventDefault(); commitText(); }
                    if (e.key === "Escape") { textDone.current = true; setTextInput(null); setTextValue(""); }
                  }}
                  onBlur={() => { if (textValue.trim()) commitText(); else { setTextInput(null); } }}
                />
              </div>
            )}
          </div>
        </div>

        <aside className="props">
          <div className="props-head">Tool options</div>
          <div className="props-body">
            {tool === "blur" && (
              <div className="prop-group">
                <label>Blur radius — {blurRadius}px</label>
                <div className="range-row">
                  <input type="range" min={2} max={24} value={blurRadius} onChange={(e) => setBlurRadius(Number(e.target.value))} />
                </div>
                <p className="prop-note">Drag a rectangle over the area to soften.</p>
              </div>
            )}
            {tool === "pixelate" && (
              <div className="prop-group">
                <label>Block size — {pixelSize}px</label>
                <div className="range-row">
                  <input type="range" min={3} max={48} value={pixelSize} onChange={(e) => setPixelSize(Number(e.target.value))} />
                </div>
                <p className="prop-note">Drag a rectangle over the area to mosaic it.</p>
              </div>
            )}
            {tool === "crop" && (
              <div className="prop-group">
                <label>Crop</label>
                <p className="prop-note">Drag on the canvas to keep only the selected area. Undo restores the original.</p>
              </div>
            )}

            {colorEnabled && (
              <div className="prop-group">
                <label>Color</label>
                <div className="swatches">
                  {COLORS.map((c) => (
                    <button
                      key={c}
                      className={cn("swatch", color === c && "on")}
                      style={{ background: c }}
                      onClick={() => setColor(c)}
                      aria-label={`Color ${c}`}
                    />
                  ))}
                </div>
              </div>
            )}

            {widthEnabled && (
              <div className="prop-group">
                <label>Stroke width — {strokeWidth}px</label>
                <div className="range-row">
                  <input type="range" min={1} max={30} value={strokeWidth} onChange={(e) => setStrokeWidth(Number(e.target.value))} />
                  <span className="range-val">{strokeWidth}</span>
                </div>
              </div>
            )}

            {tool === "text" && (
              <div className="prop-group">
                <label>Font size — {fontSize}px</label>
                <div className="range-row">
                  <input type="range" min={12} max={96} value={fontSize} onChange={(e) => setFontSize(Number(e.target.value))} />
                  <span className="range-val">{fontSize}</span>
                </div>
                <p className="prop-note">Click on the canvas, type, then press Enter.</p>
              </div>
            )}

            <div className="prop-group" style={{ borderTop: "1px solid var(--border)", paddingTop: 14 }}>
              <label>Transform</label>
              <div className="dim-row" style={{ marginBottom: 8 }}>
                <input type="number" min={1} value={dims.w} onChange={(e) => setDims({ ...dims, w: Number(e.target.value) || 1 })} />
                <span>×</span>
                <input type="number" min={1} value={dims.h} onChange={(e) => setDims({ ...dims, h: Number(e.target.value) || 1 })} />
              </div>
              <button className="btn btn-subtle full" style={{ marginBottom: 8 }} onClick={() => resizeTo(dims.w, dims.h)}>
                Apply resize
              </button>
              <div className="row" style={{ gap: 6 }}>
                <button className="icon-btn" title="Rotate left" onClick={() => rotate(-1)}><RotateCcw /></button>
                <button className="icon-btn" title="Rotate right" onClick={() => rotate(1)}><RotateCw /></button>
                <button className="icon-btn" title="Flip horizontal" onClick={() => flip(true)}><FlipHorizontal2 /></button>
                <button className="icon-btn" title="Flip vertical" onClick={() => flip(false)}><FlipVertical2 /></button>
              </div>
            </div>

            <div className="prop-group" style={{ borderTop: "1px solid var(--border)", paddingTop: 14 }}>
              <label>History</label>
              <div className="row" style={{ gap: 6 }}>
                <button className="btn btn-subtle" style={{ flex: 1 }} onClick={undo} disabled={index <= 0}><Undo2 /> Undo</button>
                <button className="btn btn-subtle" style={{ flex: 1 }} onClick={redo} disabled={index >= history.length - 1}><Redo2 /> Redo</button>
              </div>
              <button className="btn btn-ghost full" style={{ marginTop: 6 }} onClick={reset} disabled={index === 0}>
                <RotateCcw /> Reset changes
              </button>
            </div>
          </div>
        </aside>
      </div>

      <div className="editor-bottom">
        <span>Tool: <b>{TOOLS.find((t) => t.id === tool)?.label}</b></span>
        <span>Zoom: <b>Fit</b></span>
        <span>History: <b>{index + 1} / {history.length}</b></span>
        <span className="grow" />
        <span><b>Ctrl+S</b> save · <b>Ctrl+C</b> copy · <b>Ctrl+Z</b> undo · <b>Esc</b> cancel stroke</span>
      </div>
    </div>
  );
}
