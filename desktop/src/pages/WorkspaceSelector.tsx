import { CheckCircle2, FolderKanban, FolderOpen, Plus, RefreshCcw, Rocket } from "lucide-react";
import { useMemo, useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { chooseWorkspaceDirectory, isNativeDesktop } from "../native/workspaces";
import type { CreateWorkspaceInput, Depot, Stream, Workspace } from "../types/domain";

interface WorkspaceSelectorProps {
  workspaces: Workspace[];
  depots: Depot[];
  streams: Stream[];
  loading: boolean;
  error?: string;
  isAdmin: boolean;
  onRefresh: () => void;
  onCreateDepot: (name: string, description?: string) => Promise<Depot>;
  onCreateStream: (name: string, depotId: string) => Promise<Stream>;
  onCreate: (input: CreateWorkspaceInput) => Promise<void>;
  onSelect: (workspace: Workspace) => void;
}

export function WorkspaceSelector({
  workspaces,
  depots,
  streams,
  loading,
  error,
  isAdmin,
  onRefresh,
  onCreateDepot,
  onCreateStream,
  onCreate,
  onSelect,
}: WorkspaceSelectorProps) {
  const [createOpen, setCreateOpen] = useState(false);
  const [name, setName] = useState("");
  const [depotId, setDepotId] = useState("");
  const [streamId, setStreamId] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [creating, setCreating] = useState(false);
  const [formError, setFormError] = useState<string>();
  const [setupOpen, setSetupOpen] = useState(false);
  const [setupDepot, setSetupDepot] = useState("");
  const [setupStream, setSetupStream] = useState("main");
  const [setupWorkspace, setSetupWorkspace] = useState("");
  const [setupPath, setSetupPath] = useState("");
  const [setupBusy, setSetupBusy] = useState(false);
  const [setupError, setSetupError] = useState<string>();
  const [setupDepotId, setSetupDepotId] = useState<string>();
  const [setupStreamId, setSetupStreamId] = useState<string>();
  const availableStreams = useMemo(
    () => streams.filter((stream) => !depotId || stream.depot_id === depotId),
    [depotId, streams],
  );
  const valid = name.trim().length >= 2 && depotId && streamId && localPath.trim().length > 0;

  function openCreate() {
    const firstDepot = depots[0]?.id ?? "";
    const firstStream = streams.find((stream) => stream.depot_id === firstDepot)?.id ?? "";
    setName("");
    setDepotId(firstDepot);
    setStreamId(firstStream);
    setLocalPath("");
    setFormError(undefined);
    setCreateOpen(true);
  }

  async function createWorkspace() {
    if (!valid) return;
    setCreating(true);
    setFormError(undefined);
    try {
      await onCreate({
        name: name.trim(),
        depot: depotId,
        stream: streamId,
        local_path: localPath.trim(),
      });
      setCreateOpen(false);
    } catch (createError) {
      setFormError(createError instanceof Error ? createError.message : "Workspace creation failed.");
    } finally {
      setCreating(false);
    }
  }

  function openSetup() {
    setSetupDepot("");
    setSetupStream("main");
    setSetupWorkspace("");
    setSetupPath("");
    setSetupError(undefined);
    setSetupDepotId(undefined);
    setSetupStreamId(undefined);
    setSetupOpen(true);
  }

  async function setupStudio() {
    if (!setupDepot.trim() || !setupStream.trim() || !setupWorkspace.trim() || !setupPath.trim()) return;
    setSetupBusy(true);
    setSetupError(undefined);
    try {
      let depotId = setupDepotId;
      if (!depotId) {
        const depot = await onCreateDepot(setupDepot.trim(), "Created during studio setup");
        depotId = depot.id;
        setSetupDepotId(depotId);
      }
      let streamId = setupStreamId;
      if (!streamId) {
        const stream = await onCreateStream(setupStream.trim(), depotId);
        streamId = stream.id;
        setSetupStreamId(streamId);
      }
      await onCreate({
        name: setupWorkspace.trim(),
        depot: depotId,
        stream: streamId,
        local_path: setupPath.trim(),
      });
      setSetupOpen(false);
    } catch (setupFailure) {
      setSetupError(setupFailure instanceof Error ? setupFailure.message : "Studio setup failed.");
    } finally {
      setSetupBusy(false);
    }
  }

  return (
    <main className="selector-shell">
      <section className="selector-panel">
        <header className="page-heading">
          <div>
            <span className="eyebrow">Workspace</span>
            <h1>Choose a workspace</h1>
          </div>
          <div className="button-row">
            <button className="primary-button" type="button" onClick={openCreate} disabled={depots.length === 0}>
              <Plus size={16} />
              New Workspace
            </button>
            <button className="secondary-button" type="button" onClick={onRefresh}>
              <RefreshCcw size={16} />
              Refresh
            </button>
          </div>
        </header>
        {error && <div className="warning-strip">{error}</div>}
        {workspaces.length === 0 && !loading && depots.length === 0 && isAdmin ? (
          <div className="onboarding-card">
            <span className="onboarding-icon"><Rocket size={24} /></span>
            <div>
              <span className="eyebrow">First run</span>
              <h2>Set up your studio in one pass</h2>
              <p>Create the first depot, its main stream, and your local workspace. You can rename or expand them later.</p>
            </div>
            <ol className="onboarding-steps">
              <li><CheckCircle2 size={16} /> Server connected</li>
              <li><span>2</span> Depot and stream</li>
              <li><span>3</span> Local workspace</li>
            </ol>
            <button className="primary-button" type="button" onClick={openSetup}>
              Start Studio Setup
            </button>
          </div>
        ) : workspaces.length === 0 && !loading ? (
          <EmptyState
            icon={FolderKanban}
            title="No workspaces"
            detail={depots.length > 0 ? "Create a local workspace to start working." : "Create a depot and stream before adding a workspace."}
            action={depots.length > 0 ? (
              <button className="primary-button" type="button" onClick={openCreate}>
                <Plus size={16} /> New Workspace
              </button>
            ) : undefined}
          />
        ) : (
          <div className="workspace-list">
            {workspaces.map((workspace) => (
              <button key={workspace.id} className="workspace-option" type="button" onClick={() => onSelect(workspace)}>
                <span>
                  <FolderKanban size={18} />
                  {workspace.name}
                </span>
                <small>{workspace.local_path}</small>
              </button>
            ))}
          </div>
        )}
      </section>

      <ConfirmDialog
        open={createOpen}
        title="Create Workspace"
        confirmLabel={creating ? "Creating" : "Create Workspace"}
        confirmDisabled={!valid || creating}
        onClose={() => !creating && setCreateOpen(false)}
        onConfirm={() => void createWorkspace()}
      >
        <div className="form-stack workspace-form">
          <label>
            Workspace Name
            <input value={name} onChange={(event) => setName(event.target.value)} placeholder="artist-main" />
          </label>
          <label>
            Depot
            <select
              value={depotId}
              onChange={(event) => {
                const nextDepot = event.target.value;
                setDepotId(nextDepot);
                setStreamId(streams.find((stream) => stream.depot_id === nextDepot)?.id ?? "");
              }}
            >
              {depots.map((depot) => <option key={depot.id} value={depot.id}>{depot.name}</option>)}
            </select>
          </label>
          <label>
            Stream
            <select value={streamId} onChange={(event) => setStreamId(event.target.value)}>
              {availableStreams.map((stream) => <option key={stream.id} value={stream.id}>{stream.name}</option>)}
            </select>
          </label>
          <label>
            Local Folder
            <span className="path-picker-row">
              <input
                value={localPath}
                onChange={(event) => setLocalPath(event.target.value)}
                placeholder="/Projects/MyGame"
                readOnly={isNativeDesktop()}
              />
              {isNativeDesktop() && (
                <button
                  className="secondary-button"
                  type="button"
                  onClick={() => void chooseWorkspaceDirectory().then((path) => path && setLocalPath(path))}
                >
                  <FolderOpen size={16} /> Browse
                </button>
              )}
            </span>
          </label>
          {formError && <p className="form-error">{formError}</p>}
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={setupOpen}
        title="Set Up Your Studio"
        confirmLabel={setupBusy ? "Setting up" : "Create and Open Workspace"}
        confirmDisabled={setupBusy || !setupDepot.trim() || !setupStream.trim() || !setupWorkspace.trim() || !setupPath.trim()}
        onClose={() => !setupBusy && setSetupOpen(false)}
        onConfirm={() => void setupStudio()}
      >
        <div className="form-stack workspace-form">
          <label>
            Depot Name
            <input
              value={setupDepot}
              onChange={(event) => setSetupDepot(event.target.value)}
              placeholder="My Production"
              disabled={Boolean(setupDepotId)}
            />
          </label>
          <label>
            First Stream
            <input
              value={setupStream}
              onChange={(event) => setSetupStream(event.target.value)}
              placeholder="main"
              disabled={Boolean(setupStreamId)}
            />
          </label>
          <label>
            Workspace Name
            <input value={setupWorkspace} onChange={(event) => setSetupWorkspace(event.target.value)} placeholder="artist-main" />
          </label>
          <label>
            Local Folder
            <span className="path-picker-row">
              <input
                value={setupPath}
                onChange={(event) => setSetupPath(event.target.value)}
                placeholder="/Projects/MyProduction"
                readOnly={isNativeDesktop()}
              />
              {isNativeDesktop() && (
                <button
                  className="secondary-button"
                  type="button"
                  onClick={() => void chooseWorkspaceDirectory().then((path) => path && setSetupPath(path))}
                >
                  <FolderOpen size={16} /> Browse
                </button>
              )}
            </span>
          </label>
          {setupError && <p className="form-error">{setupError}</p>}
        </div>
      </ConfirmDialog>
    </main>
  );
}
