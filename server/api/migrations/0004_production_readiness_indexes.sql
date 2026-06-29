CREATE INDEX IF NOT EXISTS idx_files_stream_path_not_deleted
    ON files (stream_id, depot_path)
    WHERE deleted = FALSE;

CREATE INDEX IF NOT EXISTS idx_locks_stream_path_active
    ON locks (stream_id, depot_path)
    WHERE state = 'active';

CREATE INDEX IF NOT EXISTS idx_changelist_files_path
    ON changelist_files (depot_path);

CREATE INDEX IF NOT EXISTS idx_audit_events_workspace_created_at
    ON audit_events (workspace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_events_stream_path_created_at
    ON audit_events (stream_id, depot_path, created_at DESC)
    WHERE depot_path IS NOT NULL;
