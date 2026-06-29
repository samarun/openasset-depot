import { RefreshCcw, Settings2, WifiOff } from "lucide-react";

interface ConnectionStatusBarProps {
  retrying: boolean;
  onRetry: () => void;
  onOpenSettings: () => void;
}

export function ConnectionStatusBar({
  retrying,
  onRetry,
  onOpenSettings,
}: ConnectionStatusBarProps) {
  return (
    <div className="connection-status" role="status">
      <span className="connection-status-copy">
        <WifiOff size={15} />
        <strong>Demo Mode</strong>
        <span aria-hidden="true">·</span>
        <span>Server not connected</span>
      </span>
      <span className="connection-status-actions">
        <button className="text-button" type="button" onClick={onRetry} disabled={retrying}>
          <RefreshCcw size={14} />
          {retrying ? "Retrying" : "Retry Connection"}
        </button>
        <button className="text-button" type="button" onClick={onOpenSettings}>
          <Settings2 size={14} />
          Open Settings
        </button>
      </span>
    </div>
  );
}
