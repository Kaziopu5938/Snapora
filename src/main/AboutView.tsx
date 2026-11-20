import { Camera, Cpu, Heart, Scale, ShieldCheck } from "lucide-react";

export function AboutView() {
  return (
    <div style={{ maxWidth: 720, margin: "0 auto", display: "flex", flexDirection: "column", gap: 16 }}>
      <section className="hero" style={{ textAlign: "center", padding: "34px 28px" }}>
        <div className="brand-mark" style={{ margin: "0 auto 14px", width: 56, height: 56, flexBasis: 56 }}>
          <Camera size={30} strokeWidth={2} />
        </div>
        <div className="hero-eyebrow" style={{ justifyContent: "center" }}>Capture. Edit. Share.</div>
        <h1 style={{ fontSize: 30 }}>Snapora</h1>
        <p style={{ margin: "0 auto" }}>
          A fast, focused screenshot utility for Windows 10 and 11 — built with Rust, Tauri and React.
        </p>
        <div style={{ display: "flex", justifyContent: "center", gap: 8, marginTop: 18, flexWrap: "wrap" }}>
          <span className="badge">v1.0.2</span>
          <span className="badge">x64</span>
          <span className="badge">Windows 10 / 11</span>
        </div>
      </section>

      <div className="settings-grid">
        <section className="panel">
          <div className="panel-head"><span className="pi"><Cpu /></span><h3>Built with</h3></div>
          <div className="panel-body" style={{ fontSize: 12.5, color: "var(--text-2)", lineHeight: 1.9 }}>
            <div><b>Rust</b> + Tauri 2 — native Windows APIs via windows-sys</div>
            <div><b>React 19</b> + TypeScript + Tailwind CSS</div>
            <div><b>arboard</b> clipboard · <b>image</b> codecs · <b>WiX</b> installer</div>
            <div>GDI / DWM capture, per-monitor DPI awareness</div>
          </div>
        </section>
        <section className="panel">
          <div className="panel-head"><span className="pi"><ShieldCheck /></span><h3>Privacy</h3></div>
          <div className="panel-body" style={{ fontSize: 12.5, color: "var(--text-2)", lineHeight: 1.9 }}>
            <div>Everything runs locally on your machine.</div>
            <div>No account, telemetry or network uploads.</div>
            <div>Screenshots stay in your configured save folder.</div>
            <div>Shortcut conflicts are detected by reading one local registry value.</div>
          </div>
        </section>
        <section className="panel">
          <div className="panel-head"><span className="pi"><Scale /></span><h3>License</h3></div>
          <div className="panel-body" style={{ fontSize: 12.5, color: "var(--text-2)", lineHeight: 1.8 }}>
            Snapora is released under the <b>MIT License</b>. Free to use, modify and distribute.
            See <span className="mono">LICENSE</span> in the source repository for the full text.
          </div>
        </section>
        <section className="panel">
          <div className="panel-head"><span className="pi"><Heart /></span><h3>Credits</h3></div>
          <div className="panel-body" style={{ fontSize: 12.5, color: "var(--text-2)", lineHeight: 1.8 }}>
            Designed and engineered as a polished, professional alternative to the Windows Snipping
            Tool. Icons by Lucide. App icon generated for Snapora.
          </div>
        </section>
      </div>
    </div>
  );
}
