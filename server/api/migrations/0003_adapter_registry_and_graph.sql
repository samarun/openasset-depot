ALTER TABLE dcc_adapters
    ADD COLUMN IF NOT EXISTS sdk_contract_version TEXT NOT NULL DEFAULT 'v1';

INSERT INTO dcc_adapters (name, enabled, config)
VALUES
    ('Unreal Engine', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Unity', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Blender', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Maya', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Houdini', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Nuke', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Adobe Premiere', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('After Effects', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Photoshop', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('DaVinci Resolve', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb),
    ('Generic Media', TRUE, '{"contract":"v1","mode":"built_in"}'::jsonb)
ON CONFLICT (name) DO UPDATE
SET enabled = TRUE,
    sdk_contract_version = 'v1',
    config = dcc_adapters.config || EXCLUDED.config;

CREATE TABLE IF NOT EXISTS asset_nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    depot_path TEXT NOT NULL,
    adapter_name TEXT NOT NULL,
    asset_kind TEXT NOT NULL DEFAULT 'unknown',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (stream_id, depot_path)
);

CREATE INDEX IF NOT EXISTS idx_asset_nodes_stream_path ON asset_nodes (stream_id, depot_path);
CREATE INDEX IF NOT EXISTS idx_asset_nodes_adapter ON asset_nodes (adapter_name);
CREATE INDEX IF NOT EXISTS idx_asset_nodes_kind ON asset_nodes (asset_kind);

CREATE TABLE IF NOT EXISTS asset_dependency_edges (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id UUID NOT NULL REFERENCES streams(id) ON DELETE CASCADE,
    source_file TEXT NOT NULL,
    target_file TEXT NOT NULL,
    adapter_name TEXT NOT NULL,
    dependency_type TEXT NOT NULL DEFAULT 'unknown'
        CHECK (dependency_type IN ('metadata', 'reference', 'texture', 'plate', 'lut', 'media', 'unknown')),
    dependency_status TEXT NOT NULL DEFAULT 'unknown'
        CHECK (dependency_status IN ('present', 'missing', 'external', 'unknown')),
    scan_time TIMESTAMPTZ NOT NULL DEFAULT now(),
    confidence NUMERIC(5,4) NOT NULL DEFAULT 0.5000 CHECK (confidence >= 0 AND confidence <= 1),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    UNIQUE (stream_id, source_file, target_file, adapter_name, dependency_type)
);

CREATE INDEX IF NOT EXISTS idx_asset_dependency_edges_source ON asset_dependency_edges (stream_id, source_file);
CREATE INDEX IF NOT EXISTS idx_asset_dependency_edges_target ON asset_dependency_edges (stream_id, target_file);
CREATE INDEX IF NOT EXISTS idx_asset_dependency_edges_adapter ON asset_dependency_edges (adapter_name, scan_time DESC);
CREATE INDEX IF NOT EXISTS idx_asset_dependency_edges_status ON asset_dependency_edges (dependency_status);

ALTER TABLE asset_dependencies
    ADD COLUMN IF NOT EXISTS source_file TEXT,
    ADD COLUMN IF NOT EXISTS target_file TEXT,
    ADD COLUMN IF NOT EXISTS adapter_name TEXT NOT NULL DEFAULT 'unknown',
    ADD COLUMN IF NOT EXISTS dependency_status TEXT NOT NULL DEFAULT 'unknown',
    ADD COLUMN IF NOT EXISTS scan_time TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS confidence NUMERIC(5,4) NOT NULL DEFAULT 0.5000 CHECK (confidence >= 0 AND confidence <= 1);

INSERT INTO file_type_rules
    (name, priority, rule_kind, extension, asset_class, is_binary, lock_required, large_file, generated)
VALUES
    ('Unreal project descriptor', 20, 'extension', '.uproject', 'unreal_project', FALSE, FALSE, FALSE, FALSE),
    ('Unreal plugin descriptor', 20, 'extension', '.uplugin', 'unreal_plugin', FALSE, FALSE, FALSE, FALSE),
    ('Unreal config', 30, 'extension', '.ini', 'unreal_config', FALSE, FALSE, FALSE, FALSE),
    ('Unity material', 15, 'extension', '.mat', 'unity_material', FALSE, FALSE, FALSE, FALSE),
    ('Unity serialized asset', 15, 'extension', '.asset', 'unity_asset', FALSE, FALSE, FALSE, FALSE),
    ('Unity controller', 15, 'extension', '.controller', 'unity_controller', FALSE, FALSE, FALSE, FALSE),
    ('Unity animation', 15, 'extension', '.anim', 'unity_animation', FALSE, FALSE, FALSE, FALSE),
    ('C# source', 30, 'extension', '.cs', 'source_code', FALSE, FALSE, FALSE, FALSE),
    ('Shader source', 30, 'extension', '.shader', 'shader_source', FALSE, FALSE, FALSE, FALSE),
    ('PNG image', 20, 'extension', '.png', 'image', TRUE, FALSE, TRUE, FALSE),
    ('JPEG image', 20, 'extension', '.jpg', 'image', TRUE, FALSE, TRUE, FALSE),
    ('JPEG image alternate', 20, 'extension', '.jpeg', 'image', TRUE, FALSE, TRUE, FALSE),
    ('TIFF image', 20, 'extension', '.tif', 'image', TRUE, FALSE, TRUE, FALSE),
    ('TIFF image alternate', 20, 'extension', '.tiff', 'image', TRUE, FALSE, TRUE, FALSE),
    ('Maya texture cache', 20, 'extension', '.tx', 'texture_cache', TRUE, FALSE, TRUE, TRUE),
    ('OBJ interchange', 20, 'extension', '.obj', 'interchange_text', FALSE, FALSE, FALSE, FALSE),
    ('Blender backup', 5, 'extension', '.blend1', 'backup_file', TRUE, FALSE, TRUE, TRUE),
    ('Houdini non-commercial project', 10, 'extension', '.hipnc', 'houdini_project', TRUE, TRUE, TRUE, FALSE),
    ('Houdini geometry cache', 5, 'extension', '.bgeo', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('OpenVDB cache', 5, 'extension', '.vdb', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('DPX image sequence frame', 20, 'extension', '.dpx', 'image_sequence', TRUE, FALSE, TRUE, FALSE),
    ('Color lookup cube', 20, 'extension', '.cube', 'lut', FALSE, FALSE, FALSE, FALSE),
    ('Color lookup table', 20, 'extension', '.lut', 'lut', FALSE, FALSE, FALSE, FALSE),
    ('MPEG-4 movie', 20, 'extension', '.mp4', 'movie', TRUE, FALSE, TRUE, FALSE)
ON CONFLICT (name) DO NOTHING;

INSERT INTO file_type_rules
    (name, priority, rule_kind, directory_prefix, asset_class, is_binary, lock_required, large_file, generated)
VALUES
    ('Unreal Saved', 1, 'directory', 'Saved/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('Adobe Premiere Auto-Save', 1, 'directory', 'Adobe Premiere Pro Auto-Save/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('After Effects Auto-Save', 1, 'directory', 'Adobe After Effects Auto-Save/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('Resolve CacheClip', 1, 'directory', 'CacheClip/', 'generated_cache', TRUE, FALSE, TRUE, TRUE),
    ('Resolve OptimizedMedia', 1, 'directory', 'OptimizedMedia/', 'generated_cache', TRUE, FALSE, TRUE, TRUE)
ON CONFLICT (name) DO NOTHING;
