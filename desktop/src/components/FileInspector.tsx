import {
  ChevronDown,
  Clock3,
  FileBox,
  MousePointer2,
  GitBranch,
  History,
  Link2,
  Lock,
  MessageSquare,
  RotateCcw,
  Send,
  Trash2,
  Unlock,
  X,
} from "lucide-react";
import type { AssetFile } from "../types/domain";
import { AssetPreview } from "./AssetPreview";
import { StatusBadge } from "./StatusBadge";

interface FileInspectorProps {
  file?: AssetFile;
  onHistory: () => void;
  onLock: (file: AssetFile) => void;
  onUnlock: (file: AssetFile) => void;
  onRevert: (file: AssetFile) => void;
  onDelete: (file: AssetFile) => void;
  onSubmit: () => void;
  onReview: (file: AssetFile) => void;
  open?: boolean;
  onClose?: () => void;
  busy?: boolean;
  loadPreview?: (file: AssetFile) => Promise<string | undefined>;
}

export function FileInspector({
  file,
  onHistory,
  onLock,
  onUnlock,
  onRevert,
  onDelete,
  onSubmit,
  onReview,
  open = false,
  onClose,
  busy = false,
  loadPreview,
}: FileInspectorProps) {
  if (!file) {
    return (
      <aside className={`inspector-panel ${open ? "is-open" : ""}`}>
        <button className="inspector-close" type="button" onClick={onClose} aria-label="Close inspector">
          <X size={18} />
        </button>
        <div className="inspector-empty">
          <MousePointer2 size={22} />
          <strong>Select an asset</strong>
          <span>Versions, dependencies, locks, and actions will appear here.</span>
        </div>
      </aside>
    );
  }

  const lockedByMe = file.statuses.includes("Checked Out");
  const lockedBySomeoneElse = file.statuses.includes("In Use");
  const markedForDelete = file.statuses.includes("Marked for Delete");
  const hasLocalChanges = file.statuses.includes("Ready to Submit") || file.statuses.includes("New File") || markedForDelete;

  return (
    <>
      {open && <button className="inspector-backdrop" type="button" onClick={onClose} aria-label="Close inspector" />}
      <aside className={`inspector-panel ${open ? "is-open" : ""}`} aria-label="File inspector">
        <button className="inspector-close" type="button" onClick={onClose} aria-label="Close inspector">
          <X size={18} />
        </button>
        <AssetPreview
          file={file}
          className={`inspector-preview asset-preview-${file.previewTone}`}
          loadPreview={loadPreview}
          fallback={<span>{file.name.split(".").pop()?.toUpperCase()}</span>}
        />
        <header className="inspector-header">
          <h2 title={file.name}>{file.name}</h2>
          <p title={file.path}>{file.path}</p>
        </header>
      <div className="status-cluster">
        {file.statuses.map((status) => (
          <StatusBadge key={status} status={status} />
        ))}
      </div>
      <div className="inspector-detail-grid artist-details">
        <span>
          <Lock size={15} />
          {availabilityLabel(file)}
        </span>
        <span>
          <GitBranch size={15} />
          Version {file.revision}
        </span>
      </div>
      <section className="inspector-section">
        <h3>Dependencies</h3>
        <div className="compact-list">
          {file.dependencies.length > 0 ? (
            file.dependencies.map((dependency) => (
              <span key={dependency} className="compact-row">
                <span>
                  <Link2 size={14} />
                  {dependency}
                </span>
              </span>
            ))
          ) : (
            <span className="subtle-copy">No linked dependencies</span>
          )}
        </div>
      </section>
      <section className="inspector-section">
        <h3>Changelist</h3>
        <p className="subtle-copy">{file.changelist ?? "Not in a changelist"}</p>
      </section>
      <details className="advanced-details">
        <summary>
          <span>Advanced</span>
          <ChevronDown size={15} />
        </summary>
        <div className="inspector-detail-grid">
          <span>
            <FileBox size={15} />
            {file.kind} · {file.size}
          </span>
          <span title={file.path}>
            <GitBranch size={15} />
            {file.path}
          </span>
          <span>
            <Clock3 size={15} />
            Depot revision {file.revision}
          </span>
        </div>
      </details>
      <div className="inspector-actions">
        <button className="secondary-button review-launch-button" type="button" onClick={() => onReview(file)}>
          <MessageSquare size={16} />
          Review &amp; Annotate
        </button>
        <button
          className="primary-button"
          type="button"
          onClick={() => onLock(file)}
          disabled={busy || lockedByMe || lockedBySomeoneElse || markedForDelete}
        >
          <Lock size={16} />
          Lock / Check Out
        </button>
        <button className="secondary-button" type="button" onClick={() => onUnlock(file)} disabled={busy || !lockedByMe}>
          <Unlock size={16} />
          Unlock
        </button>
        <button className="secondary-button" type="button" onClick={onSubmit} disabled={!hasLocalChanges}>
          <Send size={16} />
          Submit Changes
        </button>
        <button className="ghost-button" type="button" onClick={() => onRevert(file)} disabled={busy || !hasLocalChanges}>
          <RotateCcw size={16} />
          Revert Intent
        </button>
        <button
          className="ghost-button danger-text-button"
          type="button"
          onClick={() => onDelete(file)}
          disabled={busy || lockedBySomeoneElse || file.statuses.includes("New File") || markedForDelete}
        >
          <Trash2 size={16} />
          Mark for Delete
        </button>
        <button className="ghost-button" type="button" onClick={onHistory}>
          <History size={16} />
          History
        </button>
      </div>
      </aside>
    </>
  );
}

function availabilityLabel(file: AssetFile): string {
  if (file.statuses.includes("Checked Out")) return "Checked out by you";
  if (file.statuses.includes("In Use")) {
    return file.owner ? `Checked out by ${file.owner}` : "Checked out by another artist";
  }
  return "Available to edit";
}
