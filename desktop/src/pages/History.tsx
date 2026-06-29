import { FolderOpen, GitCommitVertical, History as HistoryIcon } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import type { AssetFile, FileHistoryEntry } from "../types/domain";

interface HistoryProps {
  file?: AssetFile;
  entries: FileHistoryEntry[];
  loading: boolean;
  error?: string;
  onBrowseWorkspace: () => void;
}

export function History({ file, entries, loading, error, onBrowseWorkspace }: HistoryProps) {
  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">History</span>
          <h1>{file?.name ?? "File history"}</h1>
          <p>{file?.path ?? "Select a file in Workspace"}</p>
        </div>
      </header>
      <section className="timeline-panel">
        {loading ? (
          <div className="quiet-state"><HistoryIcon size={24} /><span>Loading History</span></div>
        ) : file && entries.length > 0 ? (
          entries.map((entry) => (
            <article className="timeline-row" key={`${entry.revision_number}-${entry.blob_hash}`}>
              <GitCommitVertical size={18} />
              <div>
                <h3>Version {entry.revision_number}</h3>
                <p>{entry.action} · {formatDate(entry.submitted_at)}</p>
              </div>
              <span>{formatBytes(entry.size_bytes)}</span>
            </article>
          ))
        ) : (
          <EmptyState
            icon={file ? HistoryIcon : FolderOpen}
            title={file ? "No submitted versions" : "Choose a file to view history"}
            detail={error ?? (file ? "This asset has no submitted history yet." : "Open the workspace and select an asset.")}
            action={<button className="primary-button" type="button" onClick={onBrowseWorkspace}>Browse Workspace</button>}
          />
        )}
      </section>
    </main>
  );
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${Math.round(bytes / 1_024)} KB`;
  return `${(bytes / 1_048_576).toFixed(bytes < 10_485_760 ? 1 : 0)} MB`;
}
