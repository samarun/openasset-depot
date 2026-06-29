import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ViewKey } from "../types/domain";

export interface CommandAction {
  id: string;
  label: string;
  group: string;
  view?: ViewKey;
  run: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  actions: CommandAction[];
  onClose: () => void;
}

export function CommandPalette({ open, actions, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return actions;
    return actions.filter((action) =>
      `${action.group} ${action.label}`.toLowerCase().includes(normalized),
    );
  }, [actions, query]);

  useEffect(() => {
    if (open) {
      setQuery("");
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="palette-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="command-palette"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <label className="palette-search">
          <Search size={18} />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search actions"
          />
        </label>
        <div className="palette-results">
          {filtered.map((action) => (
            <button
              key={action.id}
              type="button"
              className="palette-result"
              onClick={() => {
                action.run();
                onClose();
              }}
            >
              <span>{action.label}</span>
              <small>{action.group}</small>
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}
