# OpenAsset Depot for Blender

Install `openasset-depot-blender-0.1.0.zip` through Blender's **Install from Disk** action. The packaged add-on includes `openasset_depot_bridge`; no `pip` install is required. The **OpenAsset** panel appears in the 3D View sidebar.

Available operations:

- Refresh current `.blend` status.
- Check out and lock the current scene.
- Sync the workspace with overwrite protection.
- Validate and submit pending files.
- Revert source-control intent and release the scene lock.

The add-on never stores a JWT. It uses the authenticated `oad` CLI configuration and runs network/file operations on one bounded worker thread.
