import {
  Activity,
  CheckCircle2,
  ChevronRight,
  CircleDot,
  FolderOpen,
  Lock,
  Send,
  UploadCloud,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { AssetCard } from "../components/AssetCard";
import type { AssetFile, Changelist, LockInfo, ViewKey, Workspace } from "../types/domain";

interface HomeProps {
  workspace?: Workspace;
  files: AssetFile[];
  changelists: Changelist[];
  locks: LockInfo[];
  currentUser: string;
  onNavigate: (view: ViewKey) => void;
  onSelectFile: (file: AssetFile) => void;
  onSync: () => void;
  onSubmit: () => void;
  busy?: boolean;
  loadPreview?: (file: AssetFile) => Promise<string | undefined>;
}

interface WorkItem {
  label: string;
  detail: string;
  count: number;
  icon: LucideIcon;
  tone: "teal" | "amber" | "coral" | "blue";
  run: () => void;
}

export function Home({
  workspace,
  files,
  changelists,
  locks,
  currentUser,
  onNavigate,
  onSelectFile,
  onSync,
  onSubmit,
  busy = false,
  loadPreview,
}: HomeProps) {
  const needsSync = files.filter((file) => file.statuses.includes("Needs Sync"));
  const modified = files.filter((file) => file.statuses.some((status) =>
    status === "Ready to Submit" || status === "New File" || status === "Marked for Delete"
  ));
  const isLockedByMe = (lock: LockInfo) =>
    workspace?.id === "demo-workspace" ? lock.user_id === currentUser : lock.workspace_id === workspace?.id;
  const lockedByMe = locks.filter(isLockedByMe);
  const lockedByOthers = locks.filter((lock) => !isLockedByMe(lock));
  const warnings = changelists.reduce((count, change) => count + change.warnings.length, 0);
  const primaryAction = needsSync.length > 0
    ? { label: "Sync Latest", icon: UploadCloud, run: onSync }
    : modified.length > 0
      ? { label: "Submit Changes", icon: Send, run: onSubmit }
      : lockedByOthers.length > 0
        ? { label: "View Locks", icon: Lock, run: () => onNavigate("locks") }
        : { label: "Open Assets", icon: FolderOpen, run: () => onNavigate("workspace") };
  const PrimaryIcon = primaryAction.icon;
  const workItems = ([
    {
      label: "Ready to submit",
      detail: "Pending local changes",
      count: modified.length,
      icon: Send,
      tone: "teal",
      run: onSubmit,
    },
    {
      label: "Checked out by you",
      detail: "Assets reserved in this workspace",
      count: lockedByMe.length,
      icon: Lock,
      tone: "blue",
      run: () => onNavigate("locks"),
    },
    {
      label: "Updates available",
      detail: "Newer depot versions are ready",
      count: needsSync.length,
      icon: UploadCloud,
      tone: "amber",
      run: onSync,
    },
    {
      label: "Unavailable assets",
      detail: "Currently checked out by collaborators",
      count: lockedByOthers.length,
      icon: Lock,
      tone: "coral",
      run: () => onNavigate("locks"),
    },
  ] satisfies WorkItem[]).filter((item) => item.count > 0);

  const workspaceReady = needsSync.length === 0 && warnings === 0;

  return (
    <main className="page studio-home">
      <section className="production-header" aria-labelledby="workspace-title">
        <div className="production-header-main">
          <span className="production-kicker">
            <CircleDot size={13} /> Active Workspace
          </span>
          <h1 id="workspace-title">{workspace?.name ?? "Workspace"}</h1>
          <p>{workspaceReady ? "Your workspace is ready for production." : "A few items need your attention."}</p>
        </div>
        <div className="production-actions">
          <button className="primary-button" type="button" onClick={primaryAction.run} disabled={busy}>
            <PrimaryIcon size={17} />
            {primaryAction.label}
          </button>
          <button className="production-secondary" type="button" onClick={() => onNavigate("workspace")}>
            Browse Assets
            <ChevronRight size={16} />
          </button>
        </div>
        <div className="production-location">
          <FolderOpen size={14} />
          <span>{workspace?.local_path ?? "No workspace selected"}</span>
        </div>
      </section>

      <section className="production-pulse" aria-label="Production pulse">
        <div className="pulse-summary">
          <span>Production Pulse</span>
          <strong>{workspaceReady ? "Clear to create" : "Attention needed"}</strong>
        </div>
        <PulseItem label="Sync" value={needsSync.length === 0 ? "Up to date" : `${needsSync.length} waiting`} tone={needsSync.length ? "amber" : "teal"} />
        <PulseItem label="My checkouts" value={String(lockedByMe.length)} tone="blue" />
        <PulseItem label="Local changes" value={String(modified.length)} tone="teal" />
      </section>

      <div className="studio-workspace-grid">
        <section className="studio-section asset-workbench">
          <header className="studio-section-heading">
            <div>
              <span className="section-kicker">Assets</span>
              <h2>Continue Working</h2>
            </div>
            <button className="text-button" type="button" onClick={() => onNavigate("workspace")}>
              Open Asset Browser <ChevronRight size={15} />
            </button>
          </header>
          {files.length > 0 ? (
            <div className="asset-grid studio-asset-grid">
              {files.slice(0, 3).map((file) => (
                <AssetCard key={file.id} file={file} onSelect={onSelectFile} loadPreview={loadPreview} />
              ))}
            </div>
          ) : (
            <div className="studio-empty-state">
              <span className="studio-empty-icon"><FolderOpen size={22} /></span>
              <div>
                <strong>Your asset workspace is ready</strong>
                <p>Submitted scenes, shots, models, and media will appear here.</p>
              </div>
              <button className="secondary-button" type="button" onClick={() => onNavigate("workspace")}>
                Open Assets
              </button>
            </div>
          )}
        </section>

        <aside className="my-work-panel" aria-label="My work queue">
          <header className="studio-section-heading">
            <div>
              <span className="section-kicker">Queue</span>
              <h2>My Work</h2>
            </div>
          </header>
          {workItems.length > 0 ? (
            <div className="work-queue">
              {workItems.map((item) => (
                <button key={item.label} type="button" className="work-queue-row" onClick={item.run}>
                  <span className={`queue-icon queue-${item.tone}`}><item.icon size={16} /></span>
                  <span className="queue-copy">
                    <strong>{item.label}</strong>
                    <small>{item.detail}</small>
                  </span>
                  <span className="queue-count">{item.count}</span>
                  <ChevronRight size={15} />
                </button>
              ))}
            </div>
          ) : (
            <div className="work-clear-state">
              <CheckCircle2 size={24} />
              <strong>Nothing blocking your work</strong>
              <p>New checkouts, changes, and review requests will collect here.</p>
            </div>
          )}
        </aside>
      </div>

      <section className="studio-section depot-activity">
        <header className="studio-section-heading">
          <div>
            <span className="section-kicker">Depot</span>
            <h2>Latest Activity</h2>
          </div>
          <button className="text-button" type="button" onClick={() => onNavigate("locks")}>View Locks</button>
        </header>
        {locks.length > 0 ? (
          <div className="activity-list">
            {locks.slice(0, 4).map((lock) => (
              <div className="activity-row" key={`${lock.depot_path}-${lock.id ?? lock.user_id}`}>
                <span className="activity-icon"><Activity size={15} /></span>
                <span>
                  <strong>{lockBelongsToCurrentWorkspace(lock, workspace) ? "You checked out an asset" : "Asset checked out"}</strong>
                  <small>{lock.depot_path}</small>
                </span>
                <time>{formatActivityTime(lock.created_at)}</time>
              </div>
            ))}
          </div>
        ) : (
          <div className="activity-clear">
            <CheckCircle2 size={17} /> No recent source-control activity
          </div>
        )}
      </section>
    </main>
  );
}

function PulseItem({ label, value, tone }: { label: string; value: string; tone: "teal" | "amber" | "blue" }) {
  return (
    <div className="pulse-item">
      <span className={`pulse-dot pulse-${tone}`} />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function lockBelongsToCurrentWorkspace(lock: LockInfo, workspace?: Workspace): boolean {
  return Boolean(workspace && lock.workspace_id === workspace.id);
}

function formatActivityTime(value?: string): string {
  if (!value) return "Recently";
  const minutes = Math.max(0, Math.round((Date.now() - new Date(value).getTime()) / 60_000));
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  return hours < 24 ? `${hours} hr ago` : `${Math.round(hours / 24)} days ago`;
}
