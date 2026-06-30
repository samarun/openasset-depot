CREATE TABLE asset_review_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    revision_id UUID NOT NULL REFERENCES file_revisions(id) ON DELETE CASCADE,
    author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_comment_id UUID REFERENCES asset_review_comments(id) ON DELETE CASCADE,
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 4000),
    timecode_ms BIGINT CHECK (timecode_ms IS NULL OR timecode_ms >= 0),
    frame_number INTEGER CHECK (frame_number IS NULL OR frame_number >= 0),
    annotation JSONB,
    resolved_at TIMESTAMPTZ,
    resolved_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (annotation IS NULL OR octet_length(annotation::text) <= 131072)
);

CREATE INDEX idx_asset_review_comments_revision
    ON asset_review_comments (revision_id, created_at, id);
CREATE INDEX idx_asset_review_comments_parent
    ON asset_review_comments (parent_comment_id)
    WHERE parent_comment_id IS NOT NULL;
CREATE INDEX idx_asset_review_comments_open
    ON asset_review_comments (revision_id, created_at)
    WHERE resolved_at IS NULL;

CREATE TABLE revision_review_proxies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    revision_id UUID NOT NULL UNIQUE REFERENCES file_revisions(id) ON DELETE CASCADE,
    blob_hash TEXT NOT NULL REFERENCES blobs(hash),
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    content_type TEXT NOT NULL CHECK (content_type IN (
        'model/gltf-binary',
        'model/gltf+json',
        'application/vnd.autodesk.fbx',
        'video/mp4',
        'video/webm'
    )),
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_revision_review_proxies_blob_hash
    ON revision_review_proxies (blob_hash);

CREATE TRIGGER revision_review_proxies_immutable
    BEFORE UPDATE OR DELETE ON revision_review_proxies
    FOR EACH ROW EXECUTE FUNCTION reject_immutable_change();

INSERT INTO file_type_rules
    (name, priority, rule_kind, extension, asset_class, is_binary, lock_required, large_file, generated)
VALUES
    ('glTF binary model', 15, 'extension', '.glb', 'reviewable_3d_model', TRUE, FALSE, TRUE, FALSE),
    ('glTF model', 15, 'extension', '.gltf', 'reviewable_3d_model', FALSE, FALSE, TRUE, FALSE),
    ('MPEG-4 review movie', 15, 'extension', '.mp4', 'reviewable_movie', TRUE, FALSE, TRUE, FALSE),
    ('WebM review movie', 15, 'extension', '.webm', 'reviewable_movie', TRUE, FALSE, TRUE, FALSE),
    ('MP3 review audio', 15, 'extension', '.mp3', 'reviewable_audio', TRUE, FALSE, TRUE, FALSE),
    ('Ogg review audio', 15, 'extension', '.ogg', 'reviewable_audio', TRUE, FALSE, TRUE, FALSE)
ON CONFLICT (name) DO NOTHING;
