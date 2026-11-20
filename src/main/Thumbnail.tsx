import { useEffect, useState } from "react";
import { Image as ImageIcon } from "lucide-react";
import { api } from "../lib/api";
import { assetUrl, cn } from "../lib/utils";

export function Thumbnail({
  path,
  className,
  alt = "Screenshot preview",
}: {
  path: string;
  className?: string;
  alt?: string;
}) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let alive = true;
    setSrc(null);
    setFailed(false);
    api
      .getThumbnail(path)
      .then((p) => alive && setSrc(assetUrl(p)))
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [path]);

  if (failed || !src) {
    return (
      <div className={cn("thumb-placeholder", className)}>
        <ImageIcon size={22} />
      </div>
    );
  }
  return <img src={src} alt={alt} className={className} draggable={false} />;
}
