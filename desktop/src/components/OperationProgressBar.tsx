import { CheckCircle2, LoaderCircle, X } from "lucide-react";
import { useRef } from "react";
import type { IntegrationProgress } from "../native/integration";

interface OperationProgressBarProps {
  progress?: IntegrationProgress;
  onDismiss?: () => void;
}

export function OperationProgressBar({ progress, onDismiss }: OperationProgressBarProps) {
  const rate = useTransferRate(progress);
  if (!progress) return null;
  const total = progress.total ?? 100;
  const completed = Math.min(progress.completed ?? 0, total);
  const finished = progress.phase === "complete";
  const percent = total > 0 ? Math.round((completed / total) * 100) : 0;
  const detail = transferDetail(progress);
  const pace = finished ? undefined : transferPace(rate, progress);

  return (
    <div className="operation-progress" role="status" aria-live="polite">
      <span className="operation-progress-icon">
        {finished ? <CheckCircle2 size={17} /> : <LoaderCircle size={17} className="spin" />}
      </span>
      <span className="operation-progress-copy">
        <strong>{operationLabel(progress.command)}</strong>
        <small>{progress.message}</small>
        {detail && <small className="operation-progress-detail">{detail}</small>}
        {pace && <small className="operation-progress-pace">{pace}</small>}
      </span>
      <strong className="operation-progress-value">{finished ? "Done" : `${percent}%`}</strong>
      {onDismiss && (
        <button
          className="operation-progress-dismiss"
          type="button"
          onClick={onDismiss}
          aria-label="Hide progress. The operation keeps running."
        >
          <X size={14} />
        </button>
      )}
      <span className="operation-progress-track" aria-hidden="true">
        <span style={{ width: `${percent}%` }} />
      </span>
    </div>
  );
}

/** Window over which byte samples are averaged, long enough to absorb per-chunk jitter. */
const RATE_WINDOW_MS = 6_000;

interface RateSample {
  at: number;
  bytes: number;
}

interface TransferRate {
  bytesPerSecond?: number;
}

/**
 * Averages recent byte counts into a transfer rate.
 *
 * Samples are kept in a ref rather than state because the rate is derived
 * display data: recomputing it must not schedule another render.
 */
function useTransferRate(progress?: IntegrationProgress): TransferRate {
  const tracker = useRef<{ key?: string; samples: RateSample[] }>({ samples: [] });

  if (!progress || progress.bytesCompleted === undefined) {
    tracker.current = { samples: [] };
    return {};
  }

  const key = `${progress.operationId}:${progress.command}`;
  if (tracker.current.key !== key) {
    tracker.current = { key, samples: [] };
  }

  const samples = tracker.current.samples;
  const now = Date.now();
  const last = samples.at(-1);
  // A restarted counter means a new transfer leg; older samples would understate the rate.
  if (last && progress.bytesCompleted < last.bytes) {
    samples.length = 0;
  }
  if (!last || last.bytes !== progress.bytesCompleted) {
    samples.push({ at: now, bytes: progress.bytesCompleted });
  }
  while (samples.length > 2 && now - samples[0].at > RATE_WINDOW_MS) {
    samples.shift();
  }

  if (samples.length < 2) return {};
  const first = samples[0];
  const latest = samples[samples.length - 1];
  const elapsedMs = latest.at - first.at;
  if (elapsedMs < 500) return {};
  return { bytesPerSecond: ((latest.bytes - first.bytes) / elapsedMs) * 1_000 };
}

/**
 * Renders throughput and a coarse time-remaining band.
 *
 * The remaining time is deliberately bucketed: a precise countdown derived from
 * a short rate sample would look authoritative while being wrong.
 */
function transferPace(rate: TransferRate, progress: IntegrationProgress): string | undefined {
  const { bytesPerSecond } = rate;
  if (!bytesPerSecond || bytesPerSecond <= 0) return undefined;

  const parts = [`${formatBytes(bytesPerSecond)}/s`];
  const { bytesCompleted, bytesTotal } = progress;
  if (bytesTotal !== undefined && bytesTotal > 0 && bytesCompleted !== undefined) {
    const remaining = bytesTotal - bytesCompleted;
    if (remaining > 0) parts.push(formatEtaBand(remaining / bytesPerSecond));
  }
  return parts.join(" · ");
}

function formatEtaBand(seconds: number): string {
  if (seconds <= 10) return "a few seconds left";
  if (seconds < 60) return "under a minute left";
  if (seconds < 90) return "about a minute left";
  if (seconds < 3_600) return `about ${Math.round(seconds / 60)} min left`;
  return "over an hour left";
}

/**
 * Builds the secondary detail line from whatever the CLI reported.
 *
 * Totals are only rendered when the CLI actually sent them; a paged sync omits
 * them until the work is fully discovered, so we show a running count instead
 * of implying a denominator we do not know.
 */
function transferDetail(progress: IntegrationProgress): string | undefined {
  const parts: string[] = [];
  const { filesCompleted, filesTotal, bytesCompleted, bytesTotal, path } = progress;

  if (filesTotal !== undefined && filesTotal > 0) {
    parts.push(`${filesCompleted ?? 0} of ${filesTotal} files`);
  } else if (filesCompleted !== undefined && filesCompleted > 0) {
    parts.push(`${filesCompleted} ${filesCompleted === 1 ? "file" : "files"}`);
  }

  if (bytesTotal !== undefined && bytesTotal > 0) {
    parts.push(`${formatBytes(bytesCompleted ?? 0)} of ${formatBytes(bytesTotal)}`);
  } else if (bytesCompleted !== undefined && bytesCompleted > 0) {
    parts.push(formatBytes(bytesCompleted));
  }

  if (path) parts.push(path);

  return parts.length > 0 ? parts.join(" · ") : undefined;
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1_024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1_024; index += 1) {
    value /= 1_024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
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
    shelve: "Shelving changes",
    unshelve: "Restoring shelf",
    history: "Loading version history",
    validate: "Validating work",
  }[command];
}
