CREATE TABLE revision_previews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    revision_id UUID NOT NULL UNIQUE REFERENCES file_revisions(id) ON DELETE CASCADE,
    blob_hash TEXT NOT NULL REFERENCES blobs(hash),
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    content_type TEXT NOT NULL CHECK (content_type IN ('image/png', 'image/jpeg', 'image/webp')),
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_revision_previews_blob_hash ON revision_previews (blob_hash);

CREATE TRIGGER revision_previews_immutable
    BEFORE UPDATE OR DELETE ON revision_previews
    FOR EACH ROW EXECUTE FUNCTION reject_immutable_change();
