import { RefreshCcw, ShieldAlert } from "lucide-react";
import { LockCard } from "../components/LockCard";
import type { LockInfo, Workspace } from "../types/domain";

interface LocksProps {
  locks: LockInfo[];
  currentUser?: string;
  workspace?: Workspace;
  loading: boolean;
  onRefresh: () => void;
  onUnlock: (lock: LockInfo) => void;
}

export function Locks({ locks, currentUser, workspace, loading, onRefresh, onUnlock }: LocksProps) {
  const isLockedByMe = (lock: LockInfo) =>
    workspace?.id === "demo-workspace" ? lock.user_id === currentUser : lock.workspace_id === workspace?.id;
  const lockedByMe = locks.filter(isLockedByMe);
  const lockedByOthers = locks.filter((lock) => !isLockedByMe(lock));
  const stale = locks.filter((lock) => lock.created_at && Date.now() - new Date(lock.created_at).getTime() > 3600_000);

  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Locks</span>
          <h1>Lock Center</h1>
        </div>
        <button className="primary-button" type="button" onClick={onRefresh}>
          <RefreshCcw size={17} />
          {loading ? "Refreshing" : "Refresh"}
        </button>
      </header>
      <section className="content-grid three-columns">
        <LockSection title="Checked Out by Me" locks={lockedByMe} currentUser={currentUser} onRefresh={onRefresh} onUnlock={onUnlock} mine />
        <LockSection title="Locked by Others" locks={lockedByOthers} currentUser={currentUser} onRefresh={onRefresh} />
        <LockSection title="Stale Locks" locks={stale} currentUser={currentUser} onRefresh={onRefresh} />
      </section>
    </main>
  );
}

function LockSection({
  title,
  locks,
  currentUser,
  onRefresh,
  onUnlock,
  mine = false,
}: {
  title: string;
  locks: LockInfo[];
  currentUser?: string;
  onRefresh: () => void;
  onUnlock?: (lock: LockInfo) => void;
  mine?: boolean;
}) {
  return (
    <section className="panel">
      <header className="section-heading">
        <h2>{title}</h2>
        <span>{locks.length}</span>
      </header>
      {locks.length === 0 ? (
        <div className="quiet-state">
          <ShieldAlert size={22} />
          <span>No active locks</span>
          <button className="text-button" type="button" onClick={onRefresh}>Refresh</button>
        </div>
      ) : (
        <div className="card-list compact-cards">
          {locks.map((lock) => (
            <LockCard
              key={`${lock.depot_path}-${lock.user_id}`}
              lock={lock}
              isMine={mine}
              currentUser={currentUser}
              onUnlock={mine ? onUnlock : undefined}
            />
          ))}
        </div>
      )}
    </section>
  );
}
