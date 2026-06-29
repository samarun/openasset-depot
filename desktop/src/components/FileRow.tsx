import { Box, FileText, Music, Package, Video } from "lucide-react";
import type { AssetFile } from "../types/domain";
import { StatusBadge } from "./StatusBadge";

interface FileRowProps {
  file: AssetFile;
  selected: boolean;
  onSelect: (file: AssetFile) => void;
}

export function FileRow({ file, selected, onSelect }: FileRowProps) {
  const Icon = iconFor(file.previewTone);
  return (
    <button
      type="button"
      className={`file-row ${selected ? "is-selected" : ""}`}
      onClick={() => onSelect(file)}
      aria-pressed={selected}
    >
      <span className="file-row-icon" aria-hidden="true">
        <Icon size={18} />
      </span>
      <span className="file-row-main">
        <span className="file-row-name">{file.name}</span>
        <span className="file-row-path">{file.path}</span>
      </span>
      <span className="file-row-meta">{file.kind}</span>
      <span className="file-row-size">{file.size}</span>
      <span className="file-row-statuses">
        {file.statuses.slice(0, 2).map((status) => (
          <StatusBadge key={status} status={status} />
        ))}
      </span>
    </button>
  );
}

function iconFor(tone: AssetFile["previewTone"]) {
  if (tone === "audio") return Music;
  if (tone === "model") return Box;
  if (tone === "scene") return Package;
  if (tone === "image") return Video;
  return FileText;
}
