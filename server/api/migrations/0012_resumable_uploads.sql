-- Resumable uploads for large binary submits.
--
-- Submit is multipart and atomic: a dropped connection on a 40 GB plate meant
-- re-sending every byte. An upload session lets a client stage one file's bytes
-- across several requests, ask how much the server already holds, and continue
-- from there. Once staged, the bytes are ingested into the normal
-- content-addressed chunk store and submit references the resulting blob hash,
-- so nothing about revision commit or deduplication changes.

CREATE TABLE upload_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    -- Client-declared total, used to reject overruns and to report progress.
    -- NULL when the client streams without knowing the length up front.
    declared_size_bytes BIGINT CHECK (declared_size_bytes IS NULL OR declared_size_bytes >= 0),
    received_bytes BIGINT NOT NULL DEFAULT 0 CHECK (received_bytes >= 0),
    -- Set once the staged bytes are ingested into chunk storage.
    blob_hash TEXT,
    finalized_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT upload_sessions_finalized_has_hash CHECK (
        (finalized_at IS NULL AND blob_hash IS NULL)
        OR (finalized_at IS NOT NULL AND blob_hash IS NOT NULL)
    )
);

-- Resume lookup: a client that lost its upload id finds the open session for
-- the same workspace and path instead of starting over.
CREATE UNIQUE INDEX idx_upload_sessions_open_target
    ON upload_sessions (workspace_id, depot_path)
    WHERE finalized_at IS NULL;

CREATE INDEX idx_upload_sessions_user ON upload_sessions (user_id, created_at DESC);

-- Lets the maintenance worker reclaim staged bytes from abandoned sessions.
CREATE INDEX idx_upload_sessions_stale ON upload_sessions (updated_at)
    WHERE finalized_at IS NULL;
