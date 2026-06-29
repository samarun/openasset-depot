import { Clock, Shield, Unlock } from "lucide-react";
import type { LockInfo } from "../types/domain";
import { StatusBadge } from "./StatusBadge";

interface LockCardProps {
  lock: LockInfo;
  isMine: boolean;
  currentUser?: string;
  onUnlock?: (lock: LockInfo) => void;
}

export function LockCard({ lock, isMine, currentUser, onUnlock }: LockCardProps) {
  return (
    <article className="lock-card">
      <header className="card-header-row">
        <div>
          <h3>{lock.depot_path.split("/").pop()}</h3>
          <p>{lock.depot_path}</p>
        </div>
        <StatusBadge status={isMine ? "Checked Out" : "In Use"} />
      </header>
      <div className="lock-meta-grid">
        <span>
          <Shield size={15} />
          {isMine ? currentUser ?? "You" : lock.user_id}
        </span>
        <span>
          <Clock size={15} />
          {lock.created_at ? lockAge(lock.created_at) : "Active"}
        </span>
      </div>
      <p className="subtle-copy">{lock.reason ?? "No reason recorded"}</p>
      {isMine && onUnlock && (
        <footer className="button-row">
          <button className="secondary-button" type="button" onClick={() => onUnlock(lock)}>
            <Unlock size={16} />
            Unlock
          </button>
        </footer>
      )}
    </article>
  );
}

function lockAge(createdAt: string) {
  const minutes = Math.max(1, Math.round((Date.now() - new Date(createdAt).getTime()) / 60000));
  if (minutes < 60) return `${minutes} min`;
  return `${Math.round(minutes / 60)} hr`;
}
