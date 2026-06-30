import { CheckCircle2, LoaderCircle } from "lucide-react";
import type { IntegrationProgress } from "../native/integration";

interface OperationProgressBarProps {
  progress?: IntegrationProgress;
}

export function OperationProgressBar({ progress }: OperationProgressBarProps) {
  if (!progress) return null;
  const total = progress.total ?? 100;
  const completed = Math.min(progress.completed ?? 0, total);
  const finished = progress.phase === "complete";
  const percent = total > 0 ? Math.round((completed / total) * 100) : 0;

  return (
    <div className="operation-progress" role="status" aria-live="polite">
      <span className="operation-progress-icon">
        {finished ? <CheckCircle2 size={17} /> : <LoaderCircle size={17} className="spin" />}
      </span>
      <span className="operation-progress-copy">
        <strong>{operationLabel(progress.command)}</strong>
        <small>{progress.message}</small>
      </span>
      <strong className="operation-progress-value">{finished ? "Done" : `${percent}%`}</strong>
      <span className="operation-progress-track" aria-hidden="true">
        <span style={{ width: `${percent}%` }} />
      </span>
    </div>
  );
}

function operationLabel(command: IntegrationProgress["command"]): string {
  return {
    context: "Opening project",
    pending: "Checking local changes",
    status: "Refreshing asset status",
    checkout: "Checking out asset",
    add: "Adding asset",
    delete: "Marking asset for deletion",
    lock: "Reserving asset",
    unlock: "Releasing asset",
    revert: "Reverting checkout",
    sync: "Syncing latest files",
    submit: "Submitting changes",
    history: "Loading version history",
    validate: "Validating work",
  }[command];
}
