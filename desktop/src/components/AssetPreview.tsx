import { useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { AssetFile } from "../types/domain";

interface AssetPreviewProps {
  file: AssetFile;
  className: string;
  style?: CSSProperties;
  loadPreview?: (file: AssetFile) => Promise<string | undefined>;
  fallback: ReactNode;
}

export function AssetPreview({ file, className, style, loadPreview, fallback }: AssetPreviewProps) {
  const [source, setSource] = useState<string>();

  useEffect(() => {
    let active = true;
    let loadedSource: string | undefined;
    setSource(undefined);
    if (!file.previewAvailable || !loadPreview) return () => undefined;

    void loadPreview(file).then((value) => {
      loadedSource = value;
      if (active) setSource(value);
    }).catch(() => {
      if (active) setSource(undefined);
    });

    return () => {
      active = false;
      if (loadedSource?.startsWith("blob:")) URL.revokeObjectURL(loadedSource);
    };
  }, [file.path, file.previewAvailable, file.revision, loadPreview]);

  return (
    <span className={`${className}${source ? " has-media" : ""}`} style={style}>
      {source ? <img className="asset-preview-media" src={source} alt={`${file.name} preview`} /> : fallback}
    </span>
  );
}
