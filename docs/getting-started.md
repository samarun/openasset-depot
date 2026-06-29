# Getting Started

## Studio administrator

1. Copy `deploy/.env.example` to `deploy/.env` and replace every secret.
2. Put TLS in front of the API for any non-local deployment. Set
   `OAD_CORS_ALLOWED_ORIGINS` to the exact studio web origins.
3. Start Compose and wait for `GET /ready` to return `{"status":"ready"}`.
4. Create the first user through `POST /api/users`; this one bootstrap user is
   made a system admin.
5. Sign in to the native desktop app. Open **Admin**, create a depot for the
   game/show, and create its `main` stream.
6. Create artists as the admin and grant `read`, `write`, or `admin` depot roles.

## Create a workspace

In the desktop workspace chooser, select **New Workspace**. Choose the depot,
stream, and local project folder. Existing Blender, Maya, Unreal, Unity, Houdini,
or editorial project folders are allowed. OpenAsset writes only the `.oad`
metadata directory during initialization.

The local folder becomes the workspace boundary. Paths outside it, path
traversal, symlink escapes, nonportable Windows device names, and invalid cross-
platform filename characters are rejected.

## Add an existing project

1. Create the workspace at the project root.
2. Select **Add Files** in the desktop app and choose source assets. Generated
   caches such as Unreal `DerivedDataCache`, Unreal `Intermediate`, Unity
   `Library`, Unity `Temp`, and generic `Cache` directories are rejected by file
   rules.
3. Unity assets should include their `.meta` files. Validation reports missing
   pairs before submit.
4. Enter a meaningful initial-submit description and submit.

For very large imports, use the CLI or pipeline automation to add files in
batches. Files are streamed with bounded buffers; the client never loads an
entire binary into memory.

## Daily work

1. Sync before launching the DCC.
2. Check out an exclusive scene/package before editing. Blender, Maya, Houdini,
   Nuke, Unity, Unreal, Adobe, and Resolve panels expose the same operations.
3. Save normally in the creative application.
4. Review Workspace Changes, validate, and submit.
5. Submitted locks are released atomically with the new revisions.

Sync refuses to overwrite pending, modified, untracked, or symlink-escaped files.
Downloads are written to temporary files, BLAKE3 verified, flushed, and replaced
only after integrity succeeds.

## Delete assets and workspaces

**Mark for Delete** creates a pending delete and acquires a required lock. The
server creates a tombstone revision atomically on submit. Other workspaces remove
an unchanged local copy on their next sync and refuse to remove modified work.

Deleting a workspace releases active locks and abandons pending changelists on
the server. The desktop asks separately whether project files should be removed.
Metadata-only removal preserves all creative files.

## Recovery behavior

- A failed submit rolls back all revision metadata.
- Retrying the same submitted changelist with identical content returns the
  original revision result.
- `Revert Intent` preserves local content by design.
- If the app closes during a transfer, temporary downloads remain outside the
  destination and are retried safely. Server upload temporaries older than 24
  hours are removed at startup.
