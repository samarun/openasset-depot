# OpenAsset Depot Source Control for Unreal Engine

This is an Unreal Engine 5.4-5.6 source-control provider, not an editor toolbar shim. Copy the folder into `<Project>/Plugins/OpenAssetDepotSourceControl`, regenerate project files, and build the Editor target. Select **OpenAsset Depot** in Unreal's Source Control login window.

The provider maps Unreal operations to the versioned Rust CLI bridge:

- Connect and Update Status
- Check Out (`.uasset`, `.umap`, and other files)
- Mark for Add
- Delete with tombstone revision
- Revert and lock release
- Sync Latest with local-overwrite protection
- Check In / submit

Set `OAD_CLI` when `oad` is not on `PATH`. The project directory must contain `.oad/workspace.json`; credentials remain in the CLI configuration rather than Unreal project files.

The source targets the official Unreal `ISourceControlProvider` and `ISourceControlState` APIs. An Unreal Engine installation is required for binary compilation and editor automation tests.
