CREATE TABLE IF NOT EXISTS depot_user_permissions (
    depot_id UUID NOT NULL REFERENCES depots(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('read', 'write', 'admin')),
    granted_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (depot_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_depot_user_permissions_user
    ON depot_user_permissions (user_id, role);

CREATE INDEX IF NOT EXISTS idx_idempotency_actor_created_at
    ON idempotency_keys (actor_user_id, created_at DESC);

INSERT INTO depot_user_permissions (depot_id, user_id, role)
SELECT id, owner_user_id, 'admin'
FROM depots
WHERE owner_user_id IS NOT NULL
ON CONFLICT (depot_id, user_id) DO NOTHING;
