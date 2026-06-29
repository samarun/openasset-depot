# OpenAsset Depot for Nuke

Add this folder to `NUKE_PATH`. Nuke loads `init.py` and `menu.py`, adds an **OpenAsset** menu, and registers a dockable **OpenAsset Depot** panel.

The panel provides script status, checkout, sync, dependency/path validation, submit, and revert. Nuke Read/Write paths are validated by the server adapter; absolute paths outside the workspace are surfaced as warnings.
