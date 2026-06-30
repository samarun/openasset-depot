import { AlertTriangle, LogOut, RefreshCcw, Server, ShieldCheck, Trash2 } from "lucide-react";
import { type FormEvent, useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import type { FileTypeRule, UserSession, Workspace } from "../types/domain";

interface SettingsProps {
  session: UserSession;
  workspace: Workspace;
  filetypes: FileTypeRule[];
  deletingWorkspace: boolean;
  onRefreshFiletypes: () => void;
  onDeleteWorkspace: (deleteLocalFiles: boolean) => Promise<void>;
  onLogout: () => void;
  onChangePassword: (currentPassword: string, newPassword: string) => Promise<void>;
}

export function Settings({
  session,
  workspace,
  filetypes,
  deletingWorkspace,
  onRefreshFiletypes,
  onDeleteWorkspace,
  onLogout,
  onChangePassword,
}: SettingsProps) {
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteLocalFiles, setDeleteLocalFiles] = useState(false);
  const [confirmation, setConfirmation] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [passwordBusy, setPasswordBusy] = useState(false);
  const [passwordMessage, setPasswordMessage] = useState<{ text: string; error: boolean }>();
  const confirmDisabled = deletingWorkspace
    || (deleteLocalFiles && confirmation !== workspace.name);

  function openDelete() {
    setDeleteLocalFiles(false);
    setConfirmation("");
    setDeleteOpen(true);
  }

  async function changePassword(event: FormEvent) {
    event.preventDefault();
    if (newPassword !== confirmPassword) {
      setPasswordMessage({ text: "New passwords do not match.", error: true });
      return;
    }
    setPasswordBusy(true);
    setPasswordMessage(undefined);
    try {
      await onChangePassword(currentPassword, newPassword);
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
      setPasswordMessage({ text: "Password changed successfully.", error: false });
    } catch (error) {
      setPasswordMessage({
        text: error instanceof Error ? error.message : "Password change failed.",
        error: true,
      });
    } finally {
      setPasswordBusy(false);
    }
  }

  return (
    <main className="page">
      <header className="page-heading">
        <div>
          <span className="eyebrow">Settings</span>
          <h1>Workspace & Account</h1>
        </div>
        <button className="secondary-button" type="button" onClick={onLogout}>
          <LogOut size={17} />
          Sign Out
        </button>
      </header>
      <section className="content-grid two-columns">
        <div className="panel settings-panel">
          <header className="section-heading">
            <h2>Connection</h2>
            <Server size={18} />
          </header>
          <label>
            Server
            <input value={session.serverUrl} readOnly />
          </label>
          <label>
            User
            <input value={session.username} readOnly />
          </label>
          <label>
            Workspace
            <input value={workspace.local_path} readOnly />
          </label>
        </div>
        <form className="panel settings-panel password-panel" onSubmit={(event) => void changePassword(event)}>
          <header className="section-heading">
            <h2>Account Security</h2>
            <ShieldCheck size={18} />
          </header>
          <p className="subtle-copy">Change the password for <strong>{session.username}</strong>.</p>
          <label>
            Current password
            <input
              type="password"
              value={currentPassword}
              onChange={(event) => setCurrentPassword(event.target.value)}
              autoComplete="current-password"
              required
            />
          </label>
          <label>
            New password
            <input
              type="password"
              value={newPassword}
              onChange={(event) => setNewPassword(event.target.value)}
              autoComplete="new-password"
              minLength={8}
              required
            />
          </label>
          <label>
            Confirm new password
            <input
              type="password"
              value={confirmPassword}
              onChange={(event) => setConfirmPassword(event.target.value)}
              autoComplete="new-password"
              minLength={8}
              required
            />
          </label>
          {passwordMessage && (
            <p className={passwordMessage.error ? "form-error" : "form-success"}>{passwordMessage.text}</p>
          )}
          <button
            className="primary-button"
            type="submit"
            disabled={passwordBusy || !currentPassword || newPassword.length < 8 || !confirmPassword}
          >
            {passwordBusy ? "Changing…" : "Change Password"}
          </button>
        </form>
      </section>

      <section className="panel settings-filetypes-panel">
        <header className="section-heading">
          <h2>File Type Rules</h2>
          <button className="icon-button" type="button" onClick={onRefreshFiletypes} aria-label="Refresh file types">
            <RefreshCcw size={17} />
          </button>
        </header>
        <div className="compact-list tall-list">
          {filetypes.map((rule) => (
            <span key={rule.id ?? rule.name} className="compact-row">
              <span>
                <ShieldCheck size={14} />
                {rule.extension ?? rule.directory_prefix ?? rule.name}
              </span>
              <span>{rule.lock_required ? "Lock required" : "No lock"}</span>
            </span>
          ))}
        </div>
      </section>

      <section className="workspace-danger-zone">
        <div>
          <span className="section-kicker">Workspace Lifecycle</span>
          <h2>Remove This Workspace</h2>
          <p>Release its locks and remove the local OpenAsset metadata. Project files are preserved by default.</p>
        </div>
        <button className="danger-button" type="button" onClick={openDelete}>
          <Trash2 size={16} /> Remove Workspace
        </button>
      </section>

      <ConfirmDialog
        open={deleteOpen}
        title="Remove Workspace"
        confirmLabel={deletingWorkspace ? "Removing" : "Remove Workspace"}
        confirmDisabled={confirmDisabled}
        onClose={() => !deletingWorkspace && setDeleteOpen(false)}
        onConfirm={() => void onDeleteWorkspace(deleteLocalFiles)
          .then(() => setDeleteOpen(false))
          .catch(() => undefined)}
      >
        <div className="delete-workspace-dialog">
          <span className="delete-warning-icon"><AlertTriangle size={22} /></span>
          <div>
            <strong>Remove {workspace.name}?</strong>
            <p>Active locks will be released and pending changelists will be abandoned.</p>
          </div>
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={deleteLocalFiles}
              onChange={(event) => setDeleteLocalFiles(event.target.checked)}
            />
            Also permanently delete local project files
          </label>
          {deleteLocalFiles && (
            <label>
              Type <strong>{workspace.name}</strong> to confirm
              <input value={confirmation} onChange={(event) => setConfirmation(event.target.value)} />
            </label>
          )}
        </div>
      </ConfirmDialog>
    </main>
  );
}
