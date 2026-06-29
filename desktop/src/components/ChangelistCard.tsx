import { AlertTriangle, Send } from "lucide-react";
import type { Changelist } from "../types/domain";
import { StatusBadge } from "./StatusBadge";

interface ChangelistCardProps {
  changelist: Changelist;
  onSubmit: (changelist: Changelist) => void;
}

export function ChangelistCard({ changelist, onSubmit }: ChangelistCardProps) {
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
        <button className="primary-button" type="button" onClick={() => onSubmit(changelist)}>
          <Send size={16} />
          Submit
        </button>
      </footer>
    </article>
  );
}
