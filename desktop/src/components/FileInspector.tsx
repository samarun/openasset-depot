import { useEffect, useState } from "react";
import {
  ChevronDown,
  Clock3,
  FileBox,
  MousePointer2,
  CheckCircle2,
  GitBranch,
  History,
  Link2,
  Lock,
  MessageSquare,
  RotateCcw,
  Send,
  Trash2,
  TriangleAlert,
  Unlock,
  X,
} from "lucide-react";
import { previewPatternClass, previewSignatureStyle } from "../data/previewSignature";
import type { AssetFile, DependencyImpact, DependencyImpactEdge } from "../types/domain";
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
  onRequestReview?: (file: AssetFile) => void;
  open?: boolean;
  onClose?: () => void;
  busy?: boolean;
  loadPreview?: (file: AssetFile) => Promise<string | undefined>;
  loadImpact?: (file: AssetFile) => Promise<DependencyImpact | undefined>;
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
  onRequestReview,
  open = false,
  onClose,
  busy = false,
  loadPreview,
  loadImpact,
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
          className={`inspector-preview asset-preview asset-preview-${file.previewTone} ${previewPatternClass(file.path)}`}
          style={previewSignatureStyle(file.path)}
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
          {`Version ${file.revision}`}
        </span>
      </div>
      <DependencyImpactSection file={file} loadImpact={loadImpact} />
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
            {`${file.kind} · ${file.size}`}
          </span>
          <span title={file.path}>
            <GitBranch size={15} />
            {file.path}
          </span>
          <span>
            <Clock3 size={15} />
            {`Depot revision ${file.revision}`}
          </span>
        </div>
      </details>
      <div className="inspector-actions">
        <button className="secondary-button review-launch-button" type="button" onClick={() => onReview(file)}>
          <MessageSquare size={16} />
          Review &amp; Annotate
        </button>
        {onRequestReview && file.revision > 0 && (
          <button
            className="secondary-button"
            type="button"
            onClick={() => onRequestReview(file)}
            disabled={busy}
          >
            <CheckCircle2 size={16} />
            Request Review
          </button>
        )}
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

/** Rows shown before the list collapses behind a count. */
const IMPACT_PREVIEW_ROWS = 4;

/**
 * Answers "is this risky to change" for the selected asset.
 *
 * Lives in its own component so the panel's own early return for the empty
 * state stays a plain early return, and so a slow graph query never delays the
 * rest of the inspector rendering.
 */
function DependencyImpactSection({
  file,
  loadImpact,
}: {
  file: AssetFile;
  loadImpact?: (file: AssetFile) => Promise<DependencyImpact | undefined>;
}) {
  const [impact, setImpact] = useState<DependencyImpact>();
  const [state, setState] = useState<"idle" | "loading" | "failed">("idle");
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    if (!loadImpact) return;
    let cancelled = false;
    setState("loading");
    setImpact(undefined);
    setExpanded(false);
    void loadImpact(file)
      .then((result) => {
        if (cancelled) return;
        setImpact(result);
        setState("idle");
      })
      .catch(() => {
        // An unreachable graph must not look like an asset with no dependents,
        // which would be the more dangerous of the two wrong answers.
        if (!cancelled) setState("failed");
      });
    return () => {
      cancelled = true;
    };
  }, [file, loadImpact]);

  if (!loadImpact) return null;

  return (
    <section className="inspector-section impact-section">
      <h3>Dependency impact</h3>
      {state === "loading" && <p className="subtle-copy">Checking the dependency graph…</p>}
      {state === "failed" && (
        <p className="subtle-copy">Could not reach the dependency graph.</p>
      )}
      {state === "idle" && impact && <ImpactBody impact={impact} expanded={expanded} onExpand={() => setExpanded(true)} />}
    </section>
  );
}

function ImpactBody({
  impact,
  expanded,
  onExpand,
}: {
  impact: DependencyImpact;
  expanded: boolean;
  onExpand: () => void;
}) {
  if (!impact.last_scanned_at) {
    return (
      <p className="subtle-copy">
        Not scanned yet. Run a dependency scan from a host integration to see what uses this
        asset.
      </p>
    );
  }

  const visible = expanded ? impact.required_by : impact.required_by.slice(0, IMPACT_PREVIEW_ROWS);
  const outgoing = expanded ? impact.depends_on : impact.depends_on.slice(0, IMPACT_PREVIEW_ROWS);
  const hidden =
    impact.required_by_count - visible.length + impact.depends_on_count - outgoing.length;

  return (
    <>
      <p className={impact.required_by_count > 0 ? "impact-headline is-risky" : "impact-headline"}>
        {impact.required_by_count > 0 ? (
          <>
            <TriangleAlert size={15} />
            {impact.required_by_count === 1
              ? "1 asset uses this file"
              : `${impact.required_by_count} assets use this file`}
          </>
        ) : (
          "Nothing else references this file"
        )}
      </p>
      {visible.length > 0 && (
        <div className="compact-list">
          {visible.map((edge) => (
            <ImpactRow key={`${edge.path}-${edge.adapter_name}`} edge={edge} />
          ))}
        </div>
      )}
      <p className="subtle-copy">{describeOutgoing(impact)}</p>
      {outgoing.length > 0 && (
        <div className="compact-list">
          {outgoing.map((edge) => (
            <ImpactRow key={`out-${edge.path}-${edge.adapter_name}`} edge={edge} />
          ))}
        </div>
      )}
      {hidden > 0 && (
        <button className="ghost-button impact-more" type="button" onClick={onExpand}>
          Show {hidden} more
        </button>
      )}
      {impact.truncated && (
        <p className="subtle-copy">Showing the most recently scanned links.</p>
      )}
    </>
  );
}

function ImpactRow({ edge }: { edge: DependencyImpactEdge }) {
  return (
    <span className="compact-row impact-row">
      <span title={edge.path}>
        <Link2 size={14} />
        {edge.path}
      </span>
      <span className="impact-kind">
        {edge.dependency_type}
        {/* A referencing file that is not in the depot cannot actually break,
            so saying so keeps the count from reading as worse than it is. */}
        {!edge.in_depot && " · not in depot"}
      </span>
    </span>
  );
}

function describeOutgoing(impact: DependencyImpact): string {
  if (impact.depends_on_count === 0) return "This file references nothing itself.";
  const references =
    impact.depends_on_count === 1
      ? "It references 1 other file"
      : `It references ${impact.depends_on_count} other files`;
  if (impact.missing_count === 0) return `${references}.`;
  return `${references}, ${impact.missing_count} of which could not be found.`;
}

function availabilityLabel(file: AssetFile): string {
  if (file.statuses.includes("Checked Out")) return "Checked out by you";
  if (file.statuses.includes("In Use")) {
    return file.owner ? `Checked out by ${file.owner}` : "Checked out by another artist";
  }
  return "Available to edit";
}
