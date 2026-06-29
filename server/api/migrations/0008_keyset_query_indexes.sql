CREATE INDEX IF NOT EXISTS idx_locks_active_created_id
    ON locks (created_at DESC, id DESC)
    WHERE state = 'active';

CREATE INDEX IF NOT EXISTS idx_locks_active_stream_created_id
    ON locks (stream_id, created_at DESC, id DESC)
    WHERE state = 'active';

CREATE INDEX IF NOT EXISTS idx_audit_events_created_id
    ON audit_events (created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_audit_events_stream_created_id
    ON audit_events (stream_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_dependency_edges_stream_scan_id
    ON asset_dependency_edges (stream_id, scan_time DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_dependency_edges_source_scan_id
    ON asset_dependency_edges (stream_id, source_file, scan_time DESC, id DESC);
