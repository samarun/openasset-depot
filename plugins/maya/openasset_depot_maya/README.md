# OpenAsset Depot for Maya

Extract the packaged Maya archive into the studio module path so `OpenAssetDepot.mod` and `OpenAssetDepot/` are siblings. Load `openasset_depot_maya.py` in Plug-in Manager, then open **OpenAsset > Workspace**.

The modeless window supports scene status, checkout, sync, validation, submit, and revert. Operations run off the Maya UI thread and callbacks return through `maya.utils.executeDeferred`.

The authenticated Rust `oad` CLI must be available on the workstation. The packaged module includes the Python bridge.
