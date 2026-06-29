# Adobe Creative Cloud panel

The CEP panel connects saved Photoshop, Premiere Pro, After Effects, Illustrator,
and InDesign documents to an OpenAsset Depot workspace. It runs `oad` as a child
process without a shell and never stores an API token inside Adobe.

## Install on macOS

1. Build and install the `oad` CLI, then sign in with `oad auth login`.
2. Run `./install-macos.sh` from this directory.
3. For an unsigned development extension, enable CEP debug mode for the CSXS
   version used by the host. Production packages must be signed with Adobe's
   ZXPSignCmd tooling.
4. Restart the Adobe application and choose **Window > Extensions (Legacy) >
   OpenAsset Depot**.
5. Enter the workspace root and the absolute `oad` path if it is not on the
   host application's `PATH`.

The current document must be saved beneath the workspace root. Check Out before
editing exclusive assets, Add to Depot for new documents, Validate before
submitting, and Sync Latest only after saving or closing host documents that may
be replaced.

Adobe UXP does not provide a general-purpose API for executing `oad` with
arguments and reading structured output. CEP remains the supported local bridge
for this integration; it is isolated to static local content and invokes a fixed
command allowlist.
