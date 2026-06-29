ALTER TABLE workspaces
    ADD COLUMN deleted_at TIMESTAMPTZ;

ALTER TABLE workspaces
    DROP CONSTRAINT workspaces_user_id_name_key;

CREATE UNIQUE INDEX uq_workspaces_user_name_active
    ON workspaces (user_id, name)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_workspaces_user_active_name
    ON workspaces (user_id, name)
    WHERE deleted_at IS NULL;
