import { useState } from "react";
import {
  Copy,
  FolderOpen,
  ImageOff,
  Pencil,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { api } from "../lib/api";
import { formatBytes, formatDate } from "../lib/utils";
import type { HistoryItem } from "../lib/types";
import { useApp } from "./context";
import { Thumbnail } from "./Thumbnail";
import { Modal } from "../components/ui";

export function HistoryView() {
  const { history, historyBusy, refreshHistory, openEditor, pushToast, settings } = useApp();
  const [pendingDelete, setPendingDelete] = useState<HistoryItem | null>(null);
  const [deleting, setDeleting] = useState(false);

  const copy = async (item: HistoryItem) => {
    try {
      const b64 = await api.readImageB64(item.path);
      await api.copyImage(b64);
      pushToast("success", "Copied to clipboard", item.fileName);
    } catch (e) {
      pushToast("error", "Could not copy image", String(e));
    }
  };

  const reveal = async (item: HistoryItem) => {
    try {
      await api.revealInExplorer(item.path);
    } catch (e) {
      pushToast("error", "Could not open folder", String(e));
    }
  };

  const doDelete = async () => {
    if (!pendingDelete) return;
    setDeleting(true);
    try {
      await api.deleteHistoryItem(pendingDelete.path);
      pushToast("success", "Moved to Recycle Bin", pendingDelete.fileName);
      setPendingDelete(null);
      refreshHistory();
    } catch (e) {
      pushToast("error", "Could not delete screenshot", String(e));
    } finally {
      setDeleting(false);
    }
  };

  const openFolder = async () => {
    try {
      const dir = await api.getSaveDir();
      await api.openPath(dir);
    } catch (e) {
      pushToast("error", "Could not open save folder", String(e));
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 18 }}>
      <div className="section-head" style={{ marginBottom: 0 }}>
        <h2>All screenshots</h2>
        <span className="hint">
          {history.length} image{history.length === 1 ? "" : "s"} · {settings?.saveDir ?? "default folder"}
        </span>
        <span className="spacer" />
        <button className="btn btn-subtle" onClick={openFolder} title="Open save folder">
          <FolderOpen /> Open folder
        </button>
        <button className="icon-btn" onClick={refreshHistory} title="Refresh history" disabled={historyBusy}>
          <RefreshCw className={historyBusy ? "spin" : ""} />
        </button>
      </div>

      {history.length === 0 ? (
        <div className="card">
          <div className="empty-state">
            <div className="empty-icon"><ImageOff /></div>
            <h3>No screenshots yet</h3>
            <p>
              Captures you save are listed here with thumbnails. Use a capture mode from Home or press
              the Print Screen key to take your first one.
            </p>
            <button className="btn btn-primary" onClick={refreshHistory}>
              <RefreshCw /> Refresh
            </button>
          </div>
        </div>
      ) : (
        <div className="history-grid">
          {history.map((item) => (
            <div className="thumb-card" key={item.path}>
              <div
                className="thumb-frame"
                onDoubleClick={() =>
                  openEditor({
                    path: item.path,
                    fileName: item.fileName,
                    width: item.width,
                    height: item.height,
                    source: "history",
                    isTemp: false,
                    fromView: "history",
                  })
                }
              >
                <Thumbnail path={item.path} />
                <div className="thumb-actions">
                  <button
                    className="thumb-act"
                    title="Edit"
                    onClick={() =>
                      openEditor({
                        path: item.path,
                        fileName: item.fileName,
                        width: item.width,
                        height: item.height,
                        source: "history",
                        isTemp: false,
                        fromView: "history",
                      })
                    }
                  >
                    <Pencil />
                  </button>
                  <button className="thumb-act" title="Copy to clipboard" onClick={() => copy(item)}>
                    <Copy />
                  </button>
                  <button className="thumb-act" title="Open folder" onClick={() => reveal(item)}>
                    <FolderOpen />
                  </button>
                  <button className="thumb-act danger" title="Delete" onClick={() => setPendingDelete(item)}>
                    <Trash2 />
                  </button>
                </div>
              </div>
              <div className="thumb-meta">
                <div className="thumb-name" title={item.fileName}>{item.fileName}</div>
                <div className="thumb-sub">
                  <span>{formatDate(item.modifiedMs)}</span>
                  <span>{formatBytes(item.sizeBytes)}</span>
                </div>
                <div className="thumb-sub" style={{ marginTop: 2 }}>
                  <span>{item.width} × {item.height}</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {pendingDelete && (
        <Modal
          title="Delete screenshot?"
          description="This will move the file to the Recycle Bin. You can restore it from there."
          onClose={() => setPendingDelete(null)}
          footer={
            <>
              <button className="btn" onClick={() => setPendingDelete(null)}>Cancel</button>
              <button className="btn btn-primary" onClick={doDelete} disabled={deleting}>
                <Trash2 /> {deleting ? "Deleting…" : "Delete"}
              </button>
            </>
          }
        />
      )}
    </div>
  );
}
