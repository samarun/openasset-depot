import { AlertCircle, CheckCircle2, X } from "lucide-react";

interface ActionNoticeProps {
  message: string;
  tone: "success" | "error" | "info";
  onClose: () => void;
}

export function ActionNotice({ message, tone, onClose }: ActionNoticeProps) {
  return (
    <div className={`action-notice action-notice-${tone}`} role="status">
      {tone === "error" ? <AlertCircle size={16} /> : <CheckCircle2 size={16} />}
      <span>{message}</span>
      <button className="icon-button quiet-icon-button" type="button" aria-label="Dismiss" onClick={onClose}>
        <X size={14} />
      </button>
    </div>
  );
}
