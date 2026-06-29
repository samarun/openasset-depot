import { FileCheck2, Send } from "lucide-react";
import { useEffect, useState } from "react";
import type { Changelist, ValidationMessage } from "../types/domain";
import { ConfirmDialog } from "./ConfirmDialog";
import { SubmitChecklist } from "./SubmitChecklist";

interface SubmitDialogProps {
  changelist?: Changelist;
  open: boolean;
  validating: boolean;
  validationWarnings: ValidationMessage[];
  validationErrors: ValidationMessage[];
  onValidate: () => Promise<boolean>;
  onSubmit: (description: string) => void;
  onClose: () => void;
}

export function SubmitDialog({
  changelist,
  open,
  validating,
  validationWarnings,
  validationErrors,
  onValidate,
  onSubmit,
  onClose,
}: SubmitDialogProps) {
  const [description, setDescription] = useState("");

  useEffect(() => {
    if (open) {
      setDescription(changelist?.description ?? "");
      void onValidate();
    }
  }, [changelist?.description, onValidate, open]);

  const fileCount = changelist?.files.length ?? 0;
  const canSubmit = !validating && validationErrors.length === 0 && description.trim().length > 0 && fileCount > 0;

  return (
    <ConfirmDialog
      open={open}
      title="Submit Changes"
      confirmLabel={validating ? "Checking files" : `Submit ${fileCount} ${fileCount === 1 ? "file" : "files"}`}
      confirmDisabled={!canSubmit}
      onClose={onClose}
      onConfirm={() => onSubmit(description.trim())}
    >
      <div className="submit-dialog">
        <div className="submit-intro">
          <span className="submit-intro-icon"><FileCheck2 size={20} /></span>
          <span>
            <strong>Review once, then send</strong>
            <small>Validation runs automatically. Your files stay untouched if the server rejects the submit.</small>
          </span>
        </div>
        <section className="submit-summary">
          <h3>{changelist?.title ?? "Selected changes"}</h3>
          <div className="compact-list">
            {changelist?.files.map((file) => (
              <span className="compact-row" key={file.id}>
                <span>{file.name}</span>
                <span>{file.size}</span>
              </span>
            ))}
          </div>
        </section>
        <SubmitChecklist
          warnings={validationWarnings.length > 0 ? validationWarnings : changelist?.warnings ?? []}
          errors={validationErrors}
        />
        <label className="submit-description">
          What changed?
          <textarea
            rows={4}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder="Example: Updated hero textures and fixed the walk-cycle timing"
            required
            autoFocus
          />
        </label>
        {validating && (
          <div className="inline-progress">
            <Send size={16} />
            Checking locks, file rules, and dependencies…
          </div>
        )}
      </div>
    </ConfirmDialog>
  );
}
