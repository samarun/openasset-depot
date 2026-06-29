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

  return (
    <div className="operation-progress" role="status" aria-live="polite">
      <span className="operation-progress-icon">
        {finished ? <CheckCircle2 size={17} /> : <LoaderCircle size={17} className="spin" />}
      </span>
      <span className="operation-progress-copy">
        <strong>{progress.command === "submit" ? "Submitting changes" : "Syncing workspace"}</strong>
        <small>{progress.message}</small>
      </span>
      <span className="operation-progress-track" aria-hidden="true">
        <span style={{ width: `${total > 0 ? (completed / total) * 100 : 0}%` }} />
      </span>
    </div>
  );
}
