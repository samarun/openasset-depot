import { Box, Clapperboard, FileImage, FileText, Music2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { CSSProperties } from "react";
import type { AssetFile } from "../types/domain";
import { StatusBadge } from "./StatusBadge";

interface AssetCardProps {
  file: AssetFile;
  onSelect: (file: AssetFile) => void;
  selected?: boolean;
}

const PREVIEW_ICONS: Record<AssetFile["previewTone"], LucideIcon> = {
  audio: Music2,
  document: FileText,
  image: FileImage,
  model: Box,
  scene: Clapperboard,
};

export function AssetCard({ file, onSelect, selected = false }: AssetCardProps) {
  const PreviewIcon = PREVIEW_ICONS[file.previewTone];
  const extension = file.name.includes(".") ? file.name.split(".").pop()?.toUpperCase() : "ASSET";
  const hue = stableHue(file.path);
  const previewStyle = {
    "--preview-hue": hue,
    "--preview-rotation": `${(hue % 24) - 12}deg`,
  } as CSSProperties;

  return (
    <button
      type="button"
      className={`asset-card${selected ? " is-selected" : ""}`}
      aria-pressed={selected}
      onClick={() => onSelect(file)}
    >
      <span className={`asset-preview asset-preview-${file.previewTone}`} style={previewStyle}>
        <PreviewIcon size={30} strokeWidth={1.6} />
        <span className="asset-preview-format">{extension}</span>
      </span>
      <span className="asset-card-body">
        <span className="asset-card-name">{file.name}</span>
        <span className="asset-card-status">
          <StatusBadge status={file.statuses[0]} />
          {file.statuses.length > 1 && <span className="status-overflow">+{file.statuses.length - 1}</span>}
        </span>
        <span className="asset-card-path" title={file.path}>{file.path}</span>
      </span>
    </button>
  );
}

function stableHue(value: string): number {
  let hash = 0;
  for (const character of value) hash = (hash * 31 + character.charCodeAt(0)) | 0;
  return Math.abs(hash) % 360;
}
