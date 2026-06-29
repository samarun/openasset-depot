CREATE TABLE workspace_file_states (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    revision_number INTEGER NOT NULL CHECK (revision_number > 0),
    synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, depot_path)
);

CREATE INDEX idx_workspace_file_states_revision
    ON workspace_file_states (workspace_id, revision_number);

-- Depot paths are case-preserving but case-insensitive for cross-platform safety.
CREATE UNIQUE INDEX uq_files_stream_path_case_insensitive
    ON files (stream_id, lower(depot_path));

CREATE UNIQUE INDEX uq_active_exclusive_lock_case_insensitive
    ON locks (stream_id, lower(depot_path))
    WHERE state = 'active' AND kind = 'exclusive';

CREATE UNIQUE INDEX uq_changelist_files_path_case_insensitive
    ON changelist_files (changelist_id, lower(depot_path));
