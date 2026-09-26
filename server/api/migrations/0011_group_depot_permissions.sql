-- Grant depot access to a group rather than to each artist individually.
--
-- `groups` and `user_groups` have existed since the initial schema but nothing
-- referenced them; this migration gives them meaning and exposes a single view
-- that every permission check reads, so direct and group-derived access can
-- never drift apart.

CREATE TABLE depot_group_permissions (
    depot_id UUID NOT NULL REFERENCES depots(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('read', 'write', 'admin')),
    granted_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (depot_id, group_id)
);

CREATE INDEX idx_depot_group_permissions_group ON depot_group_permissions (group_id);

-- Optional identity-provider linkage, so an external directory group can map
-- onto a depot group without duplicating membership by hand.
ALTER TABLE groups ADD COLUMN description TEXT;
ALTER TABLE groups ADD COLUMN external_id TEXT UNIQUE;

-- A user's effective role on a depot: the strongest of their direct grant and
-- any grant held by a group they belong to. Every authorization query reads
-- this view so the two sources are always combined the same way.
CREATE VIEW effective_depot_permissions AS
SELECT
    depot_id,
    user_id,
    (ARRAY['read', 'write', 'admin'])[MAX(rank)] AS role
FROM (
    SELECT
        p.depot_id,
        p.user_id,
        CASE p.role WHEN 'admin' THEN 3 WHEN 'write' THEN 2 ELSE 1 END AS rank
    FROM depot_user_permissions p

    UNION ALL

    SELECT
        gp.depot_id,
        ug.user_id,
        CASE gp.role WHEN 'admin' THEN 3 WHEN 'write' THEN 2 ELSE 1 END AS rank
    FROM depot_group_permissions gp
    JOIN user_groups ug ON ug.group_id = gp.group_id
) grants
GROUP BY depot_id, user_id;
