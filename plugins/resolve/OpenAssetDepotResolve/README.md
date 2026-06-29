# DaVinci Resolve Studio workflow integration

This sandboxed Electron workflow integration discovers selected Media Pool clips
through Resolve's JavaScript API and sends their source paths to the `oad`
integration protocol. Resolve project databases remain managed by Resolve; media,
graphics, LUTs, scripts, and exported project archives can live in OpenAsset
Depot workspaces.

## Install on macOS

1. Install and authenticate the `oad` CLI.
2. Run `sudo ./install-macos.sh` from this directory. The installer copies the
   current `WorkflowIntegration.node` from Resolve's installed developer SDK and
   does not redistribute that proprietary binary.
3. Restart **DaVinci Resolve Studio**.
4. Open **Workspace > Workflow Integrations > OpenAsset Depot**.
5. Choose the local OpenAsset workspace and configure the absolute CLI path if
   `oad` is not available on Resolve's `PATH`.

Select clips in the Media Pool, then Check Out, Add to Depot, Validate, or Revert
their pending intent. Save the project and media changes before submitting. Sync
Latest is workspace-wide and refuses to overwrite locally modified files.

Workflow Integration plug-ins require Resolve Studio. Blackmagic currently
supports these Electron plug-ins on macOS and Windows; Linux can use the Python
scripting bridge instead.
