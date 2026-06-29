import type { StatusKind } from "../types/domain";

interface StatusBadgeProps {
  status: StatusKind | "Backend" | "Ready";
}

const STATUS_TONE: Record<string, string> = {
  "Up to Date": "up-to-date",
  "Needs Sync": "needs-sync",
  "Checked Out": "checked-out",
  "In Use": "in-use",
  "Ready to Submit": "ready-to-submit",
  "Blocked": "blocked",
  "Marked for Delete": "marked-for-delete",
  "New File": "new-file",
  "Backend": "backend",
  "Ready": "ready",
};

export function StatusBadge({ status }: StatusBadgeProps) {
  const tone = STATUS_TONE[status] ?? status.toLowerCase().replace(/\s+/g, "-");
  return <span className={`status-badge status-${tone}`}>{status}</span>;
}
