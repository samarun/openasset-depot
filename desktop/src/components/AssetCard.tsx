import { Box, Clapperboard, FileImage, FileText, Music2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { previewPatternClass, previewSignatureStyle } from "../data/previewSignature";
import type { AssetFile } from "../types/domain";
import { AssetPreview } from "./AssetPreview";
import { StatusBadge } from "./StatusBadge";

interface AssetCardProps {
  file: AssetFile;
  onSelect: (file: AssetFile) => void;
  selected?: boolean;
  loadPreview?: (file: AssetFile) => Promise<string | undefined>;
}

const PREVIEW_ICONS: Record<AssetFile["previewTone"], LucideIcon> = {
  audio: Music2,
  document: FileText,
  image: FileImage,
  model: Box,
  scene: Clapperboard,
};

export function AssetCard({ file, onSelect, selected = false, loadPreview }: AssetCardProps) {
  const PreviewIcon = PREVIEW_ICONS[file.previewTone];
  const extension = file.name.includes(".") ? file.name.split(".").pop()?.toUpperCase() : "ASSET";
  const previewStyle = previewSignatureStyle(file.path);

  return (
    <button
      type="button"
      className={`asset-card${selected ? " is-selected" : ""}`}
      aria-pressed={selected}
      onClick={() => onSelect(file)}
    >
      <AssetPreview
        file={file}
        className={`asset-preview asset-preview-${file.previewTone} ${previewPatternClass(file.path)}`}
        style={previewStyle}
        loadPreview={loadPreview}
        fallback={(
          <>
            <PreviewIcon size={30} strokeWidth={1.6} />
            <span className="asset-preview-format">{extension}</span>
          </>
        )}
      />
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