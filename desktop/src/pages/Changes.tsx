import { Edit3 } from "lucide-react";
import { ChangelistCard } from "../components/ChangelistCard";
import { EmptyState } from "../components/EmptyState";
import type { Changelist } from "../types/domain";

interface ChangesProps {
  changelists: Changelist[];
  onSubmit: (changelist: Changelist) => void;
  onBrowseWorkspace: () => void;
}

export function Changes({ changelists, onSubmit, onBrowseWorkspace }: ChangesProps) {
  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Changes</span>
          <h1>Work Packages</h1>
        </div>
        <button
          className="primary-button"
          type="button"
          onClick={() => changelists[0] && onSubmit(changelists[0])}
          disabled={changelists.length === 0}
        >
          <Edit3 size={17} />
          Submit Changes
        </button>
      </header>
      {changelists.length === 0 ? (
        <EmptyState
          icon={Edit3}
          title="No pending changes"
          detail="Open the workspace to check out or add files."
          action={<button className="primary-button" type="button" onClick={onBrowseWorkspace}>Browse Workspace</button>}
        />
      ) : (
        <section className="card-list">
          {changelists.map((changelist) => (
            <ChangelistCard key={changelist.id} changelist={changelist} onSubmit={onSubmit} />
          ))}
        </section>
      )}
    </main>
  );
}
