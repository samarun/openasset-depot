import { FolderOpen, Lock, UploadCloud, X } from "lucide-react";
import type { Workspace } from "../types/domain";

interface FirstSyncGuideProps {
  workspace: Workspace;
  busy?: boolean;
  onSync: () => void;
  onDismiss: () => void;
}

const COMPLETED_KEY = "oad.firstSyncGuide.completed";

/**
 * Workspaces that have finished onboarding.
 *
 * A completed sync counts even when it transferred nothing, so a studio whose
 * depot is still empty is not told to sync forever.
 */
function completedWorkspaces(): string[] {
  try {
    const raw = localStorage.getItem(COMPLETED_KEY);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

export function isFirstSyncGuideComplete(workspaceId: string): boolean {
  return completedWorkspaces().includes(workspaceId);
}

export function completeFirstSyncGuide(workspaceId: string): void {
  const current = completedWorkspaces();
  if (current.includes(workspaceId)) return;
  try {
    localStorage.setItem(COMPLETED_KEY, JSON.stringify([...current, workspaceId]));
  } catch {
    // A full or blocked storage quota only costs us the dismissal memory.
  }
}

/**
 * Walks a new artist from an empty workspace folder to their first checkout.
 *
 * Step one is the only actionable control: the remaining steps describe what
 * happens next rather than offering buttons that cannot work yet.
 */
export function FirstSyncGuide({
  workspace,
  busy = false,
  onSync,
  onDismiss,
}: FirstSyncGuideProps) {
  return (
    <section className="first-sync-guide" aria-labelledby="first-sync-title">
      <header className="first-sync-header">
        <div>
          <span className="section-kicker">Getting started</span>
          <h2 id="first-sync-title">Bring the depot into this folder</h2>
          <p>
            {workspace.name} is linked but empty. One sync copies the current depot revision
            into {workspace.local_path}.
          </p>
        </div>
        <button
          className="first-sync-dismiss"
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss getting started guide"
        >
          <X size={15} />
        </button>
      </header>
      <ol className="first-sync-steps">
        <GuideStep
          icon={UploadCloud}
          title="Sync the depot"
          detail="Downloads the latest revision of every file you have access to."
          action={(
            <button className="primary-button" type="button" onClick={onSync} disabled={busy}>
              <UploadCloud size={16} />
              Sync Latest
            </button>
          )}
        />
        <GuideStep
          icon={FolderOpen}
          title="Open the project in your creative app"
          detail="Point Blender, Maya, Houdini, Nuke, Unreal, or Unity at the workspace folder."
        />
        <GuideStep
          icon={Lock}
          title="Check out before you edit"
          detail="Checking out reserves the file so two artists never overwrite each other."
        />
      </ol>
    </section>
  );
}

function GuideStep({
  icon: Icon,
  title,
  detail,
  action,
}: {
  icon: typeof UploadCloud;
  title: string;
  detail: string;
  action?: React.ReactNode;
}) {
  return (
    <li className="first-sync-step">
      <span className="first-sync-step-icon">
        <Icon size={17} />
      </span>
      <span className="first-sync-step-copy">
        <strong>{title}</strong>
        <small>{detail}</small>
      </span>
      {action && <span className="first-sync-step-action">{action}</span>}
    </li>
  );
}
