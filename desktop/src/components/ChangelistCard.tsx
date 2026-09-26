import { AlertTriangle, Inbox, Send } from "lucide-react";
import type { Changelist } from "../types/domain";
import { StatusBadge } from "./StatusBadge";

interface ChangelistCardProps {
  changelist: Changelist;
  onSubmit: (changelist: Changelist) => void;
  onShelve?: (changelist: Changelist) => void;
  busy?: boolean;
}

export function ChangelistCard({ changelist, onSubmit, onShelve, busy = false }: ChangelistCardProps) {
  return (
    <article className="changelist-card">
      <header className="card-header-row">
        <div>
          <h3>{changelist.title}</h3>
          <p>{changelist.description}</p>
        </div>
        <StatusBadge status={changelist.ready ? "Ready" : "Blocked"} />
      </header>
      <div className="compact-list">
        {changelist.files.map((file) => (
          <span key={file.id} className="compact-row">
            <span>{file.name}</span>
            <span>{file.size}</span>
          </span>
        ))}
      </div>
      {changelist.warnings.length > 0 && (
        <div className="warning-strip">
          <AlertTriangle size={16} />
          <span>{changelist.warnings[0].message}</span>
        </div>
      )}
      <footer className="button-row">
        <button className="primary-button" type="button" onClick={() => onSubmit(changelist)} disabled={busy}>
          <Send size={16} />
          Submit
        </button>
        {onShelve && (
          <button className="secondary-button" type="button" onClick={() => onShelve(changelist)} disabled={busy}>
            <Inbox size={16} />
            Shelve Changes
          </button>
        )}
      </footer>
    </article>
  );
}
