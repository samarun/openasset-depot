CREATE TABLE shelved_changes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    changelist_id UUID NOT NULL REFERENCES changelists(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    blob_hash TEXT NOT NULL REFERENCES blobs(hash),
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'unshelved', 'deleted')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (changelist_id, depot_path)
);

CREATE INDEX idx_shelved_changes_workspace ON shelved_changes (workspace_id, state);
CREATE INDEX idx_shelved_changes_stream_path ON shelved_changes (stream_id, depot_path);
