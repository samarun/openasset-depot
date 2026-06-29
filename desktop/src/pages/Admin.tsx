import { Database, GitBranch, Plus, RefreshCcw, Shield } from "lucide-react";
import { useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { Depot, FileTypeRule, Stream } from "../types/domain";

interface AdminProps {
  depots: Depot[];
  streams: Stream[];
  filetypes: FileTypeRule[];
  onCreateDepot: (name: string, description?: string) => Promise<void>;
  onCreateStream: (name: string, depotId: string) => Promise<void>;
  onRefresh: () => void;
}

export function Admin({
  depots,
  streams,
  filetypes,
  onCreateDepot,
  onCreateStream,
  onRefresh,
}: AdminProps) {
  const [dialog, setDialog] = useState<"depot" | "stream">();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [depotId, setDepotId] = useState("");
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string>();

  function openDialog(next: "depot" | "stream") {
    setDialog(next);
    setName("");
    setDescription("");
    setDepotId(depots[0]?.id ?? "");
    setFormError(undefined);
  }

  async function save() {
    if (!dialog || name.trim().length < 2) return;
    setSaving(true);
    setFormError(undefined);
    try {
      if (dialog === "depot") {
        await onCreateDepot(name.trim(), description.trim() || undefined);
      } else {
        await onCreateStream(name.trim(), depotId);
      }
      setDialog(undefined);
    } catch (error) {
      setFormError(error instanceof Error ? error.message : "Creation failed.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Admin</span>
          <h1>Depot Controls</h1>
        </div>
        <div className="button-row">
          <button className="primary-button" type="button" onClick={() => openDialog("depot")}>
            <Plus size={17} /> New Depot
          </button>
          <button className="secondary-button" type="button" onClick={() => openDialog("stream")} disabled={depots.length === 0}>
            <GitBranch size={17} /> New Stream
          </button>
          <button className="icon-button" type="button" onClick={onRefresh} aria-label="Refresh admin data">
            <RefreshCcw size={17} />
          </button>
        </div>
      </header>
      <section className="content-grid two-columns">
        <div className="panel">
          <header className="section-heading">
            <h2>Depots and Streams</h2>
            <Database size={18} />
          </header>
          <div className="compact-list admin-depot-list">
            {depots.length === 0 ? (
              <div className="inline-empty-state">
                <strong>No depots yet</strong>
                <span>Create a depot to organize a game, show, or production.</span>
              </div>
            ) : (
              depots.map((depot) => (
                <div key={depot.id} className="admin-depot-row">
                  <span className="admin-depot-name">
                    <Database size={14} /> {depot.name}
                  </span>
                  <small>{depot.description ?? "Creative production depot"}</small>
                  <div className="admin-stream-list">
                    {streams
                      .filter((stream) => stream.depot_id === depot.id)
                      .map((stream) => (
                        <span key={stream.id}>
                          <GitBranch size={13} /> {stream.name}
                        </span>
                      ))}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
        <div className="panel">
          <header className="section-heading">
            <h2>Lock-required Rules</h2>
            <Shield size={18} />
          </header>
          <div className="compact-list">
            {filetypes
              .filter((rule) => rule.lock_required)
              .slice(0, 12)
              .map((rule) => (
                <span key={rule.id ?? rule.name} className="compact-row">
                  <span>{rule.extension ?? rule.directory_prefix ?? rule.rule_kind}</span>
                  <span>{rule.asset_class}</span>
                </span>
              ))}
          </div>
        </div>
      </section>

      <ConfirmDialog
        open={Boolean(dialog)}
        title={dialog === "stream" ? "Create Stream" : "Create Depot"}
        confirmLabel={saving ? "Creating" : dialog === "stream" ? "Create Stream" : "Create Depot"}
        confirmDisabled={saving || name.trim().length < 2 || (dialog === "stream" && !depotId)}
        onClose={() => !saving && setDialog(undefined)}
        onConfirm={() => void save()}
      >
        <div className="form-stack">
          <label>
            Name
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder={dialog === "stream" ? "main" : "MyGame"}
            />
          </label>
          {dialog === "depot" ? (
            <label>
              Description
              <input
                value={description}
                onChange={(event) => setDescription(event.target.value)}
                placeholder="Game production depot"
              />
            </label>
          ) : (
            <label>
              Depot
              <select value={depotId} onChange={(event) => setDepotId(event.target.value)}>
                {depots.map((depot) => (
                  <option key={depot.id} value={depot.id}>
                    {depot.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          {formError && <p className="form-error">{formError}</p>}
        </div>
      </ConfirmDialog>
    </main>
  );
}
