import { useEffect, useState } from "react";
import type { AssetFile, Collaborator } from "../types/domain";
import { ConfirmDialog } from "./ConfirmDialog";

interface ReviewRequestDialogProps {
  open: boolean;
  file?: AssetFile;
  collaborators: Collaborator[];
  currentUser?: string;
  loadingCollaborators: boolean;
  busy?: boolean;
  onSubmit: (input: { title: string; description: string; reviewers: string[] }) => void;
  onClose: () => void;
}

/**
 * Names the revision and the people who must sign it off.
 *
 * Reviewers are depot collaborators rather than a free-text username field:
 * assigning someone who cannot open the asset would create a review that can
 * never complete.
 */
export function ReviewRequestDialog({
  open,
  file,
  collaborators,
  currentUser,
  loadingCollaborators,
  busy = false,
  onSubmit,
  onClose,
}: ReviewRequestDialogProps) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [selected, setSelected] = useState<string[]>([]);

  const candidates = collaborators.filter((person) => person.username !== currentUser);

  useEffect(() => {
    if (!open) return;
    setTitle(file ? `${file.name} r${file.revision}` : "");
    setDescription("");
    setSelected([]);
  }, [file, open]);

  function toggle(userId: string) {
    setSelected((current) =>
      current.includes(userId) ? current.filter((id) => id !== userId) : [...current, userId],
    );
  }

  const canSubmit =
    Boolean(file && file.revision > 0) && title.trim().length > 0 && selected.length > 0 && !busy;

  return (
    <ConfirmDialog
      open={open}
      title="Request Review"
      confirmLabel={busy ? "Sending" : "Request Review"}
      confirmDisabled={!canSubmit}
      onClose={onClose}
      onConfirm={() => onSubmit({ title: title.trim(), description: description.trim(), reviewers: selected })}
    >
      <div className="form-stack">
        {file ? (
          <p className="subtle-copy">
            {file.path} · revision {file.revision}. An approval names these bytes, so a later
            submit needs a new review.
          </p>
        ) : (
          <p className="subtle-copy">Select a submitted asset in the inspector first.</p>
        )}
        <label>
          Title
          <input value={title} onChange={(event) => setTitle(event.target.value)} required maxLength={200} />
        </label>
        <label>
          Notes
          <textarea
            rows={3}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            maxLength={4000}
            placeholder="What should the reviewer look at?"
          />
        </label>
        <fieldset className="reviewer-picker">
          <legend>Reviewers</legend>
          {loadingCollaborators && <p className="subtle-copy">Loading collaborators…</p>}
          {!loadingCollaborators && candidates.length === 0 && (
            <p className="subtle-copy">
              Nobody else can open this depot yet. Grant a teammate access first.
            </p>
          )}
          {candidates.map((person) => (
            <label key={person.user_id} className="reviewer-option">
              <input
                type="checkbox"
                checked={selected.includes(person.user_id)}
                onChange={() => toggle(person.user_id)}
              />
              {person.display_name || person.username}
              <small>{person.role}</small>
            </label>
          ))}
        </fieldset>
      </div>
    </ConfirmDialog>
  );
}
