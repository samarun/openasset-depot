-- Shelving stores pending work on the server without creating a revision.
--
-- The `shelved_changes` table shipped in 0002 but was never written to, so this
-- migration completes it for the handlers that now use it.

-- A shelf has to remember whether the file was new, because at unshelve time the
-- depot may have gained or lost that path and guessing would silently turn an
-- add into an edit. Deletes are not shelvable: the table requires a blob, and a
-- delete has no content to park.
ALTER TABLE shelved_changes
    ADD COLUMN action TEXT NOT NULL DEFAULT 'edit'
        CHECK (action IN ('add', 'edit'));

-- Listing a workspace's shelves and reading one shelf are the two hot paths.
CREATE INDEX idx_shelved_changes_changelist_state
    ON shelved_changes (changelist_id, state);

-- Case-insensitive path guard matching files and changelist_files, so a shelf
-- cannot hold both `Art/Hero.fbx` and `art/hero.fbx` for one changelist.
CREATE UNIQUE INDEX uq_shelved_changes_path_case_insensitive
    ON shelved_changes (changelist_id, lower(depot_path));
