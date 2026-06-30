# OpenAsset Depot for Blender

Install `openasset-depot-blender-0.1.2.zip` through Blender's **Install from Disk** action. The packaged add-on includes `openasset_depot_bridge`; no `pip` install is required. The **OpenAsset** panel appears in the 3D View sidebar.

Available operations:

- Refresh current `.blend` status.
- Add a new `.blend` to pending changes for its first submission.
- Check out and lock the current scene.
- Sync the workspace with overwrite protection.
- Validate and submit pending files.
- Revert source-control intent and release the scene lock.

Submitting an untracked current scene automatically marks it for addition. The
panel saves dirty Blender changes before starting a submission and shows the
named operation, current phase, and percentage while it works. Completion and
failure states remain visible instead of collapsing back to a generic message.
Expired sessions are translated into a short recovery message with an **Open
Desktop App to Sign In** action; raw server JSON is never shown to an artist.

The add-on never stores a JWT. It uses the authenticated `oad` CLI configuration and runs network/file operations on one bounded worker thread.

Platform-specific packages bundle and auto-discover the matching CLI. A tracked
scene modified before checkout is safely checked out during submit when no
newer revision or competing lock exists. Camera previews are rendered with
Eevee and attached to the submitted revision without workspace sidecar files.
