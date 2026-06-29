import {
  Archive,
  ClipboardList,
  Clock,
  FolderKanban,
  Home,
  LockKeyhole,
  Settings,
  Shield,
} from "lucide-react";
import type { ViewKey } from "../types/domain";

type ConnectionState = "connected" | "reconnecting" | "offline";

interface SidebarProps {
  activeView: ViewKey;
  connectionState: ConnectionState;
  isAdmin: boolean;
  onNavigate: (view: ViewKey) => void;
}

const artistItems: Array<{ key: ViewKey; label: string; icon: typeof Home }> = [
  { key: "home", label: "My Work", icon: Home },
  { key: "workspace", label: "Assets", icon: FolderKanban },
  { key: "changes", label: "Changes", icon: ClipboardList },
  { key: "locks", label: "Locks", icon: LockKeyhole },
];

const leadItems: Array<{ key: ViewKey; label: string; icon: typeof Home }> = [
  { key: "history", label: "History", icon: Clock },
];

const adminItems: Array<{ key: ViewKey; label: string; icon: typeof Home }> = [
  { key: "admin", label: "Admin", icon: Shield },
];

const accountItems: Array<{ key: ViewKey; label: string; icon: typeof Home }> = [
  { key: "settings", label: "Settings", icon: Settings },
];

const CONNECTION_DISPLAY: Record<ConnectionState, { label: string; detail: string; className: string }> = {
  connected: { label: "Connected", detail: "Production services ready", className: "sidebar-status-dot is-connected" },
  reconnecting: { label: "Reconnecting", detail: "Attempting to reach server", className: "sidebar-status-dot is-reconnecting" },
  offline: { label: "Offline", detail: "Running in demo mode", className: "sidebar-status-dot is-offline" },
};

export function Sidebar({ activeView, connectionState, isAdmin, onNavigate }: SidebarProps) {
  const { label, detail, className } = CONNECTION_DISPLAY[connectionState];
  return (
    <aside className="sidebar">
      <div className="brand-lockup">
        <span className="brand-mark">
          <Archive size={22} />
        </span>
        <div>
          <strong>OpenAsset</strong>
          <span>Depot</span>
        </div>
      </div>
      <nav className="sidebar-nav" aria-label="Main navigation">
        <span className="nav-group-label">Production</span>
        <NavigationItems items={artistItems} activeView={activeView} onNavigate={onNavigate} />
        <span className="nav-group-label utility-label">Explore</span>
        <NavigationItems items={leadItems} activeView={activeView} onNavigate={onNavigate} />
        {isAdmin && (
          <>
            <span className="nav-group-label utility-label">Administration</span>
            <NavigationItems items={adminItems} activeView={activeView} onNavigate={onNavigate} />
          </>
        )}
        <span className="nav-group-label utility-label">Account</span>
        <NavigationItems items={accountItems} activeView={activeView} onNavigate={onNavigate} />
      </nav>
      <div className="sidebar-status">
        <span className={className} />
        <span>
          <strong>{label}</strong>
          <small>{detail}</small>
        </span>
      </div>
    </aside>
  );
}

function NavigationItems({
  items,
  activeView,
  onNavigate,
}: {
  items: Array<{ key: ViewKey; label: string; icon: typeof Home }>;
  activeView: ViewKey;
  onNavigate: (view: ViewKey) => void;
}) {
  return items.map((item) => {
    const Icon = item.icon;
    return (
      <button
        key={item.key}
        type="button"
        className={activeView === item.key ? "is-active" : ""}
        onClick={() => onNavigate(item.key)}
      >
        <Icon size={18} />
        {item.label}
      </button>
    );
  });
}
