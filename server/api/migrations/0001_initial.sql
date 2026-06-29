CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT NOT NULL UNIQUE,
    display_name TEXT,
    password_hash TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE user_groups (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, group_id)
);

CREATE TABLE depots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    owner_user_id UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE streams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    depot_id UUID NOT NULL REFERENCES depots(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    parent_stream_id UUID REFERENCES streams(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (depot_id, name)
);

CREATE TABLE workspaces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    depot_id UUID NOT NULL REFERENCES depots(id) ON DELETE CASCADE,
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    local_path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, name)
);

CREATE TABLE blobs (
    hash TEXT PRIMARY KEY,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    chunk_count INTEGER NOT NULL CHECK (chunk_count >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE chunks (
    hash TEXT PRIMARY KEY,
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE blob_chunks (
    blob_hash TEXT NOT NULL REFERENCES blobs(hash) ON DELETE CASCADE,
    chunk_hash TEXT NOT NULL REFERENCES chunks(hash),
    chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    PRIMARY KEY (blob_hash, chunk_index)
);

CREATE TABLE files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    head_revision INTEGER NOT NULL DEFAULT 0 CHECK (head_revision >= 0),
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (stream_id, depot_path)
);

CREATE TABLE file_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    revision_number INTEGER NOT NULL CHECK (revision_number > 0),
    blob_hash TEXT NOT NULL REFERENCES blobs(hash),
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    action TEXT NOT NULL CHECK (action IN ('add', 'edit', 'delete')),
    changelist_id UUID,
    submitted_by UUID NOT NULL REFERENCES users(id),
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (file_id, revision_number)
);

CREATE INDEX idx_file_revisions_stream_path ON file_revisions (stream_id, depot_path, revision_number DESC);
CREATE INDEX idx_file_revisions_file_history ON file_revisions (file_id, revision_number DESC);

CREATE TABLE locks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'exclusive' CHECK (kind IN ('exclusive')),
    state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'released', 'force_released')),
    reason TEXT,
    force_released_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX uq_active_exclusive_lock
    ON locks (stream_id, depot_path)
    WHERE state = 'active' AND kind = 'exclusive';
CREATE INDEX idx_locks_user_active ON locks (user_id, state);
CREATE INDEX idx_locks_stream_path ON locks (stream_id, depot_path);

CREATE TABLE changelists (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'submitted', 'abandoned')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    submitted_at TIMESTAMPTZ
);

CREATE INDEX idx_changelists_workspace_status ON changelists (workspace_id, status);

CREATE TABLE changelist_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    changelist_id UUID NOT NULL REFERENCES changelists(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('add', 'edit', 'delete')),
    file_revision_id UUID REFERENCES file_revisions(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (changelist_id, depot_path)
);

ALTER TABLE file_revisions
    ADD CONSTRAINT fk_file_revisions_changelist
    FOREIGN KEY (changelist_id) REFERENCES changelists(id);

CREATE TABLE file_type_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    priority INTEGER NOT NULL DEFAULT 100,
    rule_kind TEXT NOT NULL CHECK (rule_kind IN ('exact', 'directory', 'extension', 'fallback')),
    exact_path TEXT,
    directory_prefix TEXT,
    extension TEXT,
    glob TEXT,
    regex TEXT,
    asset_class TEXT NOT NULL,
    is_binary BOOLEAN NOT NULL DEFAULT TRUE,
    lock_required BOOLEAN NOT NULL DEFAULT FALSE,
    large_file BOOLEAN NOT NULL DEFAULT FALSE,
    generated BOOLEAN NOT NULL DEFAULT FALSE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_file_type_rules_kind ON file_type_rules (rule_kind, enabled, priority);
CREATE INDEX idx_file_type_rules_extension ON file_type_rules (extension) WHERE extension IS NOT NULL;
CREATE INDEX idx_file_type_rules_directory ON file_type_rules (directory_prefix) WHERE directory_prefix IS NOT NULL;

CREATE TABLE dcc_adapters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE asset_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    revision_id UUID REFERENCES file_revisions(id) ON DELETE CASCADE,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE asset_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    target_file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    dependency_type TEXT NOT NULL DEFAULT 'unknown',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (source_file_id, target_file_id, dependency_type)
);

CREATE INDEX idx_asset_dependencies_source ON asset_dependencies (source_file_id);
CREATE INDEX idx_asset_dependencies_target ON asset_dependencies (target_file_id);

CREATE TABLE audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type TEXT NOT NULL,
    actor_user_id UUID REFERENCES users(id),
    stream_id UUID REFERENCES streams(id),
    workspace_id UUID REFERENCES workspaces(id),
    depot_path TEXT,
    changelist_id UUID REFERENCES changelists(id),
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_events_created_at ON audit_events (created_at DESC);
CREATE INDEX idx_audit_events_type ON audit_events (event_type);
CREATE INDEX idx_audit_events_actor ON audit_events (actor_user_id);

CREATE TABLE idempotency_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key TEXT NOT NULL UNIQUE,
    actor_user_id UUID REFERENCES users(id),
    request_hash TEXT NOT NULL,
    response JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION reject_immutable_change() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'immutable table % cannot be modified after insert', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER file_revisions_immutable_update
    BEFORE UPDATE OR DELETE ON file_revisions
    FOR EACH ROW EXECUTE FUNCTION reject_immutable_change();

CREATE TRIGGER audit_events_append_only
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION reject_immutable_change();

CREATE TRIGGER blobs_immutable
    BEFORE UPDATE OR DELETE ON blobs
    FOR EACH ROW EXECUTE FUNCTION reject_immutable_change();

INSERT INTO file_type_rules
    (name, priority, rule_kind, extension, asset_class, is_binary, lock_required, large_file, generated)
VALUES
    ('Unreal asset package', 10, 'extension', '.uasset', 'unreal_asset', TRUE, TRUE, TRUE, FALSE),
    ('Unreal map package', 10, 'extension', '.umap', 'unreal_map', TRUE, TRUE, TRUE, FALSE),
    ('Unity scene', 10, 'extension', '.unity', 'unity_scene', FALSE, TRUE, FALSE, FALSE),
    ('Unity prefab', 10, 'extension', '.prefab', 'unity_prefab', FALSE, TRUE, FALSE, FALSE),
    ('Unity metadata', 5, 'extension', '.meta', 'unity_metadata', FALSE, FALSE, FALSE, FALSE),
    ('Blender scene', 10, 'extension', '.blend', 'blender_scene', TRUE, TRUE, TRUE, FALSE),
    ('Maya ASCII', 10, 'extension', '.ma', 'maya_ascii', FALSE, TRUE, FALSE, FALSE),
    ('Maya binary', 10, 'extension', '.mb', 'maya_binary', TRUE, TRUE, TRUE, FALSE),
    ('Houdini project', 10, 'extension', '.hip', 'houdini_project', TRUE, TRUE, TRUE, FALSE),
    ('Houdini asset', 10, 'extension', '.hda', 'houdini_asset', TRUE, TRUE, TRUE, FALSE),
    ('Nuke script', 20, 'extension', '.nk', 'nuke_script', FALSE, FALSE, FALSE, FALSE),
    ('Photoshop document', 10, 'extension', '.psd', 'photoshop_document', TRUE, TRUE, TRUE, FALSE),
    ('Photoshop large document', 10, 'extension', '.psb', 'photoshop_large_document', TRUE, TRUE, TRUE, FALSE),
    ('Premiere project', 10, 'extension', '.prproj', 'premiere_project', TRUE, TRUE, TRUE, FALSE),
    ('After Effects project', 10, 'extension', '.aep', 'after_effects_project', TRUE, TRUE, TRUE, FALSE),
    ('DaVinci Resolve project export', 10, 'extension', '.drp', 'resolve_project', TRUE, TRUE, TRUE, FALSE),
    ('FBX interchange', 15, 'extension', '.fbx', 'interchange_binary', TRUE, FALSE, TRUE, FALSE),
    ('USD scene', 15, 'extension', '.usd', 'interchange_scene', FALSE, FALSE, TRUE, FALSE),
    ('Alembic cache', 15, 'extension', '.abc', 'geometry_cache', TRUE, FALSE, TRUE, FALSE),
    ('OpenEXR image', 15, 'extension', '.exr', 'image_sequence', TRUE, FALSE, TRUE, FALSE),
    ('QuickTime movie', 15, 'extension', '.mov', 'movie', TRUE, FALSE, TRUE, FALSE),
    ('Wave audio', 15, 'extension', '.wav', 'audio', TRUE, FALSE, TRUE, FALSE),
    ('Fallback default source asset', 1000, 'fallback', NULL, 'generic_asset', TRUE, FALSE, FALSE, FALSE)
ON CONFLICT (name) DO NOTHING;

INSERT INTO file_type_rules
    (name, priority, rule_kind, directory_prefix, asset_class, is_binary, lock_required, large_file, generated)
VALUES
    ('Unreal DerivedDataCache', 1, 'directory', 'DerivedDataCache/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('Unreal Intermediate', 1, 'directory', 'Intermediate/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('Unity Library', 1, 'directory', 'Library/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('Unity Temp', 1, 'directory', 'Temp/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('Generic cache', 1, 'directory', 'Cache/', 'generated_cache', TRUE, FALSE, TRUE, TRUE)
ON CONFLICT (name) DO NOTHING;
