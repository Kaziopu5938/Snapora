import { useState, type ReactNode } from "react";
import {
  ArrowUpRight,
  Crop,
  Lasso,
  Maximize2,
  Monitor,
  PanelsTopLeft,
  MousePointer2,
  PenLine,
  ScanLine,
  Sparkles,
} from "lucide-react";
import { api } from "../lib/api";
import { formatBytes, relativeDate } from "../lib/utils";
import { useApp } from "./context";
import { Thumbnail } from "./Thumbnail";

type ArtKind = "region" | "fullscreen" | "window" | "active" | "freeform" | "monitor" | "all";

function TileArt({ kind }: { kind: ArtKind }) {
  const common = { fill: "none", stroke: "currentColor", strokeWidth: 1.6, strokeLinejoin: "round" as const };
  return (
    <svg viewBox="0 0 240 104" className="tile-art" aria-hidden="true">
      <rect x="14" y="10" width="212" height="84" rx="8" className="art-screen" />
      <rect x="24" y="20" width="192" height="64" rx="4" className="art-content" />
      <rect x="24" y="20" width="192" height="13" rx="4" className="art-bar" />
      <circle cx="32" cy="26.5" r="2" className="art-dot" />
      <circle cx="40" cy="26.5" r="2" className="art-dot" />
      <rect x="34" y="44" width="60" height="6" rx="3" className="art-line" />
      <rect x="34" y="56" width="92" height="5" rx="2.5" className="art-line dim" />
      <rect x="34" y="66" width="74" height="5" rx="2.5" className="art-line dim" />
      {kind === "region" && (
        <>
          <rect x="92" y="38" width="96" height="46" rx="4" className="art-select" strokeDasharray="5 4" />
          <rect x="153" y="78" width="36" height="14" rx="3" className="art-badge" />
          <rect x="158" y="83" width="26" height="4" rx="2" className="art-badge-txt" />
          {[[92, 38], [188, 38], [92, 84], [188, 84]].map(([x, y]) => (
            <circle key={`${x}-${y}`} cx={x} cy={y} r="3.2" className="art-handle" />
          ))}
        </>
      )}
      {kind === "fullscreen" && (
        <rect x="92" y="38" width="96" height="46" rx="4" className="art-select solid" />
      )}
      {kind === "window" && (
        <>
          <rect x="60" y="32" width="104" height="56" rx="5" className="art-select solid" />
          <rect x="60" y="32" width="104" height="11" rx="5" className="art-window-bar" />
          <MousePointer2 x={150} y={74} width={18} height={18} className="art-cursor" {...common} />
        </>
      )}
      {kind === "active" && (
        <>
          <rect x="60" y="32" width="104" height="56" rx="5" className="art-select solid" />
          <rect x="60" y="32" width="104" height="11" rx="5" className="art-window-bar" />
          <circle cx="157" cy="39" r="3" className="art-badge" />
        </>
      )}
      {kind === "freeform" && (
        <path d="M96 46 L132 34 L178 44 L186 70 L150 84 L104 76 Z" className="art-select" strokeDasharray="5 4" />
      )}
      {kind === "monitor" && (
        <>
          <rect x="88" y="36" width="104" height="44" rx="5" className="art-select solid" />
          <rect x="124" y="82" width="32" height="4" rx="2" className="art-stand" />
          <rect x="110" y="86" width="60" height="4" rx="2" className="art-stand" />
        </>
      )}
      {kind === "all" && (
        <>
          <rect x="56" y="40" width="66" height="38" rx="4" className="art-select solid" />
          <rect x="130" y="40" width="66" height="38" rx="4" className="art-select solid" />
          <rect x="110" y="82" width="32" height="3" rx="1.5" className="art-stand" />
        </>
      )}
    </svg>
  );
}

const Kbd = ({ children }: { children: ReactNode }) => <kbd>{children}</kbd>;

export function HomeView() {
  const { startCapture, history, openEditor, settings } = useApp();
  const [monitorBusy, setMonitorBusy] = useState(false);

  const startMonitor = async (all = false) => {
    if (all) return startCapture("allMonitors");
    setMonitorBusy(true);
    try {
      const monitors = await api.getMonitors();
      const primary = monitors.find((m) => m.primary) ?? monitors[0];
      startCapture("monitor", primary?.index ?? 0);
    } catch {
      startCapture("monitor", 0);
    } finally {
      setMonitorBusy(false);
    }
  };

  const recent = history.slice(0, 4);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
      <section className="hero">
        <div className="hero-eyebrow">
          <Sparkles size={14} /> Welcome back
        </div>
        <h1>What will you capture today?</h1>
        <p>
          Pick a mode to start. Snapora freezes the screen for pixel-perfect selections, then drops
          you straight into a full editor. Every shortcut works even when Snapora lives in the tray.
        </p>
      </section>

      <section>
        <div className="section-head">
          <h2>Quick capture</h2>
          <span className="hint">One click or a keyboard shortcut</span>
        </div>
        <div className="tile-grid">
          <button className="tile lg-tile" onClick={() => startCapture("region")}>
            <div className="tile-top">
              <span className="tile-icon"><Crop /></span>
              <span className="tile-kbd">PrtSc</span>
            </div>
            <div className="tile-hero lg"><TileArt kind="region" /></div>
            <div>
              <h3>Region capture</h3>
              <p>Drag across any part of the screen for a precise rectangular selection.</p>
            </div>
          </button>
          <button className="tile" onClick={() => startCapture("fullscreen")}>
            <div className="tile-top">
              <span className="tile-icon"><Maximize2 /></span>
              <span className="tile-kbd">Ctrl + PrtSc</span>
            </div>
            <div className="tile-hero lg"><TileArt kind="fullscreen" /></div>
            <div>
              <h3>Fullscreen capture</h3>
              <p>Grab the whole desktop — all connected displays in a single image.</p>
            </div>
          </button>
        </div>
      </section>

      <section>
        <div className="section-head">
          <h2>More capture modes</h2>
        </div>
        <div className="grid-4">
          <button className="mini-tile" onClick={() => startCapture("windowSelect")}>
            <span className="tile-icon"><MousePointer2 /></span>
            <b>Window</b>
            <span>Pick any visible window</span>
          </button>
          <button className="mini-tile" onClick={() => startCapture("activeWindow")}>
            <span className="tile-icon"><ScanLine /></span>
            <b>Active window</b>
            <span>Focused app · Alt+PrtSc</span>
          </button>
          <button className="mini-tile" onClick={() => startCapture("freeform")}>
            <span className="tile-icon"><Lasso /></span>
            <b>Freeform</b>
            <span>Lasso an area · Shift+PrtSc</span>
          </button>
          <button className="mini-tile" onClick={() => startMonitor(false)} disabled={monitorBusy}>
            <span className="tile-icon"><Monitor /></span>
            <b>Monitor</b>
            <span>A single display</span>
          </button>
          <button className="mini-tile" onClick={() => startMonitor(true)}>
            <span className="tile-icon"><PanelsTopLeft /></span>
            <b>All monitors</b>
            <span>Every display at once</span>
          </button>
          <button className="mini-tile" onClick={() => startCapture("region")}>
            <span className="tile-icon"><PenLine /></span>
            <b>Annotate after</b>
            <span>Every capture opens the editor</span>
          </button>
        </div>
      </section>

      <section>
        <div className="section-head">
          <h2>Recent captures</h2>
          <span className="spacer" />
          <span className="hint">{history.length} saved</span>
        </div>
        {recent.length === 0 ? (
          <div className="card" style={{ padding: 22, display: "flex", alignItems: "center", gap: 14 }}>
            <span className="tile-icon"><Crop /></span>
            <div>
              <div style={{ fontWeight: 650, fontSize: 13 }}>Nothing captured yet</div>
              <div className="muted" style={{ fontSize: 12, marginTop: 3 }}>
                Your screenshots will appear here, saved to{" "}
                <span className="mono">{settings?.saveDir ?? "%USERPROFILE%\\Pictures\\Snapora"}</span>.
              </div>
            </div>
          </div>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 12 }}>
            {recent.map((item) => (
              <button
                key={item.path}
                className="thumb-card"
                onClick={() =>
                  openEditor({
                    path: item.path,
                    fileName: item.fileName,
                    width: item.width,
                    height: item.height,
                    source: "history",
                    isTemp: false,
                    fromView: "home",
                  })
                }
              >
                <div className="thumb-frame"><Thumbnail path={item.path} /></div>
                <div className="thumb-meta">
                  <div className="thumb-name">{item.fileName}</div>
                  <div className="thumb-sub">
                    <span>{relativeDate(item.modifiedMs)}</span>
                    <span>{formatBytes(item.sizeBytes)}</span>
                  </div>
                </div>
              </button>
            ))}
          </div>
        )}
      </section>

      <section>
        <div className="section-head">
          <h2>Keyboard shortcuts</h2>
          <span className="hint">Global — work from anywhere, even minimized</span>
        </div>
        <div className="card card-pad">
          <div className="kbd-list">
            <div className="kbd-row"><span>Region capture</span><span className="keys"><Kbd>PrtSc</Kbd></span></div>
            <div className="kbd-row"><span>Active window</span><span className="keys"><Kbd>Alt</Kbd><Kbd>PrtSc</Kbd></span></div>
            <div className="kbd-row"><span>Fullscreen</span><span className="keys"><Kbd>Ctrl</Kbd><Kbd>PrtSc</Kbd></span></div>
            <div className="kbd-row"><span>Freeform</span><span className="keys"><Kbd>Shift</Kbd><Kbd>PrtSc</Kbd></span></div>
            <div className="kbd-row"><span>Cancel capture</span><span className="keys"><Kbd>Esc</Kbd></span></div>
            <div className="kbd-row"><span>Save / Copy</span><span className="keys"><Kbd>Ctrl</Kbd><Kbd>S</Kbd> <Kbd>Ctrl</Kbd><Kbd>C</Kbd></span></div>
          </div>
          <div className="note info" style={{ marginTop: 14 }}>
            <ArrowUpRight size={16} />
            <span>
              Windows 11 can reserve Print Screen for Snipping Tool. Snapora detects this conflict and
              shows the registration status in Settings → Keyboard.
            </span>
          </div>
        </div>
      </section>
    </div>
  );
}
