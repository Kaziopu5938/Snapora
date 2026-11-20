import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MainApp } from "./main/MainApp";
import { OverlayApp } from "./overlay/OverlayApp";

export default function App() {
  const [isOverlay, setIsOverlay] = useState<boolean | null>(null);

  useEffect(() => {
    setIsOverlay(getCurrentWindow().label.startsWith("overlay-"));
  }, []);

  if (isOverlay === null) return <div style={{ width: "100vw", height: "100vh", background: "transparent" }} />;
  return isOverlay ? <OverlayApp /> : <MainApp />;
}
