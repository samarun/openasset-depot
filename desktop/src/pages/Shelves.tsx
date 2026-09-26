import { Inbox, RefreshCcw, RotateCcw, Trash2 } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import type { ShelfSummary } from "../types/domain";

interface ShelvesProps {
  shelves: ShelfSummary[];
  currentUser?: string;
  loading: boolean;
  busy?: boolean;
  onRefresh: () => void;
  onRestore: (shelf: ShelfSummary) => void;
  onDiscard: (shelf: ShelfSummary) => void;
}

/**
 * Lists work parked on the server, the caller's own first.
 *
 * Teammates' shelves are shown but not actionable: a shelf belongs to the
 * changelist that created it, so restoring someone else's would drop their
 * files into the wrong workspace.
 */
export function Shelves({
  shelves,
  currentUser,
  loading,
  busy = false,
  onRefresh,
  onRestore,
  onDiscard,
}: ShelvesProps) {
  const mine = shelves.filter((shelf) => shelf.owner === currentUser);
  const others = shelves.filter((shelf) => shelf.owner !== currentUser);

  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Shelves</span>
          <h1>Parked Work</h1>
        </div>
        <button className="primary-button" type="button" onClick={onRefresh} disabled={loading}>
          <RefreshCcw size={17} />
          {loading ? "Refreshing" : "Refresh"}
        </button>
      </header>
      {shelves.length === 0 ? (
        <EmptyState
          icon={Inbox}
          title="Nothing shelved"
          detail="Shelving stores unfinished work on the server without submitting it, so you can switch machines or hand a task over without publishing a broken asset."
          action={
            <button className="secondary-button" type="button" onClick={onRefresh}>
              <RefreshCcw size={16} />
              Refresh
            </button>
          }
        />
      ) : (
        <section className="content-grid two-columns">
          <ShelfSection
            title="My Shelves"
            shelves={mine}
            emptyCopy="You have nothing parked."
            onRestore={onRestore}
            onDiscard={onDiscard}
            busy={busy}
          />
          <ShelfSection
            title="Shelved by Others"
            shelves={others}
            emptyCopy="No teammates have parked work on this stream."
            busy={busy}
          />
        </section>
      )}
    </main>
  );
}

function ShelfSection({
  title,
  shelves,
  emptyCopy,
  onRestore,
  onDiscard,
  busy,
}: {
  title: string;
  shelves: ShelfSummary[];
  emptyCopy: string;
  onRestore?: (shelf: ShelfSummary) => void;
  onDiscard?: (shelf: ShelfSummary) => void;
  busy: boolean;
}) {
  return (
    <section className="panel">
      <header className="section-heading">
        <h2>{title}</h2>
        <span>{shelves.length}</span>
      </header>
      {shelves.length === 0 ? (
        <p className="subtle-copy">{emptyCopy}</p>
      ) : (
        <div className="card-list compact-cards">
          {shelves.map((shelf) => (
            <article className="shelf-card" key={shelf.changelist_id}>
              <header>
                <h3 title={shelf.description}>{shelf.description || "Untitled changelist"}</h3>
                <span className="shelf-meta">
                  {shelf.owner} · {shelf.file_count} {shelf.file_count === 1 ? "file" : "files"} ·{" "}
                  {formatBytes(shelf.total_bytes)}
                </span>
              </header>
              <span className="shelf-meta">Shelved {formatWhen(shelf.shelved_at)}</span>
              {(onRestore || onDiscard) && (
                <div className="shelf-actions">
                  {onRestore && (
                    <button
                      className="secondary-button"
                      type="button"
                      onClick={() => onRestore(shelf)}
                      disabled={busy}
                    >
                      <RotateCcw size={16} />
                      Restore Shelf
                    </button>
                  )}
                  {onDiscard && (
                    <button
                      className="ghost-button danger-text-button"
                      type="button"
                      onClick={() => onDiscard(shelf)}
                      disabled={busy}
                    >
                      <Trash2 size={16} />
                      Discard
                    </button>
                  )}
                </div>
              )}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit + 1 < units.length) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}

function formatWhen(timestamp: string): string {
  const when = new Date(timestamp);
  if (Number.isNaN(when.getTime())) return "recently";
  const minutes = Math.round((Date.now() - when.getTime()) / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return when.toLocaleDateString();
}
