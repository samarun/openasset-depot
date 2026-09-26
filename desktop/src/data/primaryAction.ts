import { FolderOpen, Lock, Send, UploadCloud } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { AssetFile, LockInfo, ViewKey, Workspace } from "../types/domain";

export type PrimaryActionKind = "sync" | "submit" | "locks" | "browse";

export interface PrimaryAction {
  kind: PrimaryActionKind;
  label: string;
  icon: LucideIcon;
  run: () => void;
}

export interface PrimaryActionInput {
  files: AssetFile[];
  locks: LockInfo[];
  workspace?: Workspace;
  currentUser: string;
  onSync: () => void;
  onSubmit: () => void;
  onNavigate: (view: ViewKey) => void;
}

const PENDING_STATUSES = new Set<AssetFile["statuses"][number]>([
  "Ready to Submit",
  "New File",
  "Marked for Delete",
]);

export function isLockHeldBy(
  lock: LockInfo,
  workspace: Workspace | undefined,
  currentUser: string,
): boolean {
  // Demo fixtures carry no workspace id, so they fall back to the username.
  return workspace?.id === "demo-workspace"
    ? lock.user_id === currentUser
    : lock.workspace_id === workspace?.id;
}

export function countNeedsSync(files: AssetFile[]): number {
  return files.filter((file) => file.statuses.includes("Needs Sync")).length;
}

export function countPending(files: AssetFile[]): number {
  return files.filter((file) => file.statuses.some((status) => PENDING_STATUSES.has(status))).length;
}

/**
 * Resolves the single next action for the current workspace state.
 *
 * Both the home hero and the top bar render from this so the app never offers
 * two competing "primary" actions at the same time.
 */
export function resolvePrimaryAction({
  files,
  locks,
  workspace,
  currentUser,
  onSync,
  onSubmit,
  onNavigate,
}: PrimaryActionInput): PrimaryAction {
  if (countNeedsSync(files) > 0) {
    return { kind: "sync", label: "Sync Latest", icon: UploadCloud, run: onSync };
  }
  if (countPending(files) > 0) {
    return { kind: "submit", label: "Submit Changes", icon: Send, run: onSubmit };
  }
  if (locks.some((lock) => !isLockHeldBy(lock, workspace, currentUser))) {
    return { kind: "locks", label: "View Locks", icon: Lock, run: () => onNavigate("locks") };
  }
  return { kind: "browse", label: "Open Assets", icon: FolderOpen, run: () => onNavigate("workspace") };
}
