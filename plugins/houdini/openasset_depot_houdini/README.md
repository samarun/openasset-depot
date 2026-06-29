# OpenAsset Depot for Houdini

Extract the packaged integration to a permanent location, then run `install.py --packages-dir <houdini-version-packages-directory>`. The installer atomically generates the absolute package registration; `packages/openasset_depot.json` is a marked example only. Open **Windows > Python Panel Editor**, install `python_panels/openasset_depot.pypanel`, then create an **OpenAsset Depot** pane.

The panel supports `.hip` scene status and checkout, selected HDA library checkout, safe sync, validation, submit, and revert. Long operations run on one worker and return to Houdini through `hdefereval`.
