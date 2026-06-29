import { ChevronDown, Command, FolderGit2, Moon, Search, Sun } from "lucide-react";
import type { UserSession, Workspace } from "../types/domain";

interface TopBarProps {
  session: UserSession;
  workspace?: Workspace;
  theme: "light" | "dark";
  onOpenPalette: () => void;
  onToggleTheme: () => void;
  onSwitchWorkspace: () => void;
}

export function TopBar({
  session,
  workspace,
  theme,
  onOpenPalette,
  onToggleTheme,
  onSwitchWorkspace,
}: TopBarProps) {
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
