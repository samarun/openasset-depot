import { ChevronDown, Command, FolderGit2, LogOut, Moon, Search, Sun } from "lucide-react";
import type { PrimaryAction } from "../data/primaryAction";
import type { UserSession, Workspace } from "../types/domain";

interface TopBarProps {
  session: UserSession;
  workspace?: Workspace;
  theme: "light" | "dark";
  primaryAction?: PrimaryAction;
  primaryActionBusy?: boolean;
  onOpenPalette: () => void;
  onToggleTheme: () => void;
  onSwitchWorkspace: () => void;
  onLogout: () => void;
}

export function TopBar({
  session,
  workspace,
  theme,
  primaryAction,
  primaryActionBusy = false,
  onOpenPalette,
  onToggleTheme,
  onSwitchWorkspace,
  onLogout,
}: TopBarProps) {
  const PrimaryIcon = primaryAction?.icon;
  return (
    <header className="topbar">
      <button className="search-trigger" type="button" onClick={onOpenPalette}>
        <Search size={17} />
        <span>Find assets, changes, or commands</span>
        <kbd>
          <Command size={12} />K
        </kbd>
      </button>
      <div className="topbar-right">
        {primaryAction && PrimaryIcon && (
          <button
            className="primary-button topbar-primary"
            type="button"
            onClick={primaryAction.run}
            disabled={primaryActionBusy}
          >
            <PrimaryIcon size={16} />
            <span>{primaryAction.label}</span>
          </button>
        )}
        <button className="workspace-chip" type="button" onClick={onSwitchWorkspace}>
          <FolderGit2 size={15} />
          {workspace?.name ?? "No workspace"}
          <ChevronDown size={14} />
        </button>
        <button className="icon-button" type="button" onClick={onToggleTheme} aria-label="Toggle theme">
          {theme === "light" ? <Moon size={18} /> : <Sun size={18} />}
        </button>
        <span className="user-chip">
          <span className="user-avatar">{initials(session.username)}</span>
          <span className="user-name">{session.username}</span>
        </span>
        <button className="secondary-button topbar-signout" type="button" onClick={onLogout}>
          <LogOut size={16} />
          <span>Sign out</span>
        </button>
      </div>
    </header>
  );
}

function initials(username: string): string {
  return username
    .split(/[._-]/)
    .map((part) => part[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
}
