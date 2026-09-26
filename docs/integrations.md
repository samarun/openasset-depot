# Creative Host Integrations

For first-admin creation, artist accounts, CLI login, workspace setup, and
step-by-step Blender/Maya/Unreal use, see the [administrator and artist user
guide](user-guide.md).

All host integrations call the bundled or installed `oad` binary without a shell.
Artists sign in and connect a project folder once in the desktop app; that local
session is then available to the creative plug-ins without a terminal login.
Python hosts use streaming protocol version 2 for live operation progress, while
the existing protocol remains compatible with other hosts. Integrations bound
stdout/stderr to 4 MiB, apply operation timeouts, and keep JWTs out of project
files. Run `./scripts/package-integrations.py`
and verify `dist/integrations/SHA256SUMS` before studio distribution.

## Shared workflow

- The project must be inside a desktop/CLI-created workspace containing
  `.oad/workspace.json`.
- Sync before opening scenes that may be replaced.
- Check out exclusive scene/package files before editing.
- Add new files, validate, enter a useful description, and submit.
- Revert removes source-control intent and releases the lock but preserves the
  local creative file.
- Actions show human-readable progress such as **Checking out scene**,
  **Downloading files**, **Validating**, and **Submitting changes**.

## Integration protocol

`oad integration --protocol-version N <command>` writes line-delimited JSON to
stdout. Version 1 emits a single result object. Version 2 is additive: it emits
zero or more `progress` messages followed by exactly one `result` message, and
tags every message with a `type` field so readers can dispatch on it.

A version 2 progress message looks like this:

```json
{
  "protocol_version": 2,
  "type": "progress",
  "operation": "sync",
  "phase": "transferring",
  "message": "Downloading Content/Maps/Main.umap",
  "completed": 70,
  "total": 100,
  "path": "Content/Maps/Main.umap",
  "files_completed": 12,
  "bytes_completed": 5242880
}
```

`operation`, `phase`, `message`, `completed`, and `total` are always present.
The transfer detail fields — `path`, `files_completed`, `files_total`,
`bytes_completed`, `bytes_total` — are **omitted whenever the CLI does not know
the value**. In particular a paged sync omits `files_total` and `bytes_total`
until it has finished discovering work, so consumers must render a running count
rather than inventing a denominator. Readers should ignore unknown fields so
that later additions stay backward compatible.

### Cancel policy

Sync and submit are safe to cancel only at defined boundaries:

- **Safe:** between sync pages. Each page is applied to the workspace, persisted
  to `.oad/state.json`, and acknowledged to the server before the next page is
  requested, so an interrupted sync resumes cleanly from the last acknowledged
  page.
- **Unsafe:** during a multipart submit upload. The changelist submit is a
  single atomic server transaction; killing the client mid-upload aborts the
  request server-side and leaves the changelist open with pending files intact,
  but no partial revision is ever created.
- A workspace operation lock (`.oad/operation.lock`) prevents a second CLI or
  desktop operation from racing an in-flight one.

Dismissing the desktop progress panel only hides it; the underlying operation
continues to run.

## Blender

Install `openasset-depot-blender-0.1.2.zip` through **Edit > Preferences >
Add-ons > Install from Disk**, enable **OpenAsset Depot**, and configure an
absolute CLI path in the add-on preferences if needed. The **OpenAsset** tab in
the 3D View sidebar operates on the saved `.blend` scene.

The panel supports status, checkout, safe workspace sync, validation, submit,
and revert. Save the `.blend` under the workspace before checkout. The packaged
add-on vendors the dependency-free Python bridge; it does not require `pip`.

Host verification: registration and teardown pass in Blender 5.1.1. The source
uses the standard Blender add-on API and remains compatible with current 4.x/5.x
Python add-on loading.

## Maya

Extract `openasset-depot-maya-0.1.0.zip` into a studio Maya module directory so
`OpenAssetDepot.mod` and `OpenAssetDepot/` are siblings. Add that directory to
`MAYA_MODULE_PATH`, restart Maya, and load `openasset_depot_maya.py` in **Plug-in
Manager**. Open **OpenAsset > Workspace**.

The modeless window operates on the current `.ma` or `.mb`; network and file
operations run off the Maya UI thread and return through
`maya.utils.executeDeferred`. The archive includes the shared bridge under the
module's `scripts` path.

## Houdini

Extract `openasset-depot-houdini-0.1.0.zip` to a permanent tools directory, then
register it for the desired Houdini version:

```sh
python3 openasset_depot_houdini/install.py \
  --packages-dir "$HOME/Library/Preferences/houdini/20.5/packages"
```

On Windows, pass the matching `%USERPROFILE%/Documents/houdini<version>/packages`
directory. Restart Houdini, install `python_panels/openasset_depot.pypanel` in
the Python Panel Editor, and create an **OpenAsset Depot** pane.

The panel handles the `.hip` scene and selected HDA library files. Work is
serialized through one worker and callbacks return through `hdefereval`.

## Nuke

Extract `openasset-depot-nuke-0.1.0.zip` and add the extracted
`openasset_depot_nuke` directory to `NUKE_PATH`, or copy its contents into the
studio `.nuke` package location. Restart Nuke and open **OpenAsset > Workspace**
or the dockable **OpenAsset Depot** panel.

The panel operates on the saved `.nk` script. Server adapter validation reports
absolute Read/Write paths outside the workspace and missing plates as creative
warnings.

## Unreal Engine

Extract `openasset-depot-unreal-0.1.0.zip` into
`<Project>/Plugins/OpenAssetDepotSourceControl`, regenerate project files, and
build the Editor target. In Unreal, choose **Revision Control > Connect to
Revision Control**, then select **OpenAsset Depot**.

The provider maps Connect, Update Status, Check Out, Mark for Add, Delete,
Revert, Sync, and Check In to the shared protocol. It runs commands on the Unreal
thread pool, cancels bounded child processes during provider shutdown, and
guards cached state with a mutex.

Source compatibility was checked against the installed Unreal Engine 5.5
`ISourceControlProvider` and `ISourceControlState` interfaces. On this build
machine, Unreal Automation Tool cannot compile any Mac target because UE 5.5
rejects Xcode/macOS SDK 26.4 as newer than its maximum supported 16.9 SDK. Use a
UE-supported toolchain or build the plug-in on Windows; this is an engine
toolchain gate, not a plug-in runtime fallback.

## Unity

In **Window > Package Manager**, choose **Add package from tarball** and select
`com.openassetdepot.unity-0.1.0.tgz`. Open **Window > OpenAsset Depot** and set
the CLI path under **Project Settings > OpenAsset Depot**.

The Unity 2022.3+ editor package integrates selected assets and the active scene,
pairs `.meta` files, checks lock-required assets before save, creates tombstones
before delete, records moves as delete-old/add-new, and adds Project window state
indicators. Editor logic uses a bounded no-shell child process and includes NUnit
tests for argument quoting and checkout file selection.

## Adobe Creative Cloud

Extract `openasset-depot-adobe-cep-0.1.0.zip` into the Adobe CEP extensions
directory, or run `install-macos.sh` from the source tree. Production studio
packages should be signed with Adobe's ZXPSignCmd certificate flow. Unsigned
development panels require CEP PlayerDebugMode for the host's CSXS version.

Restart Photoshop, Premiere Pro, After Effects, Illustrator, or InDesign and
open **Window > Extensions (Legacy) > OpenAsset Depot**. Configure the workspace
root and absolute CLI path. The panel discovers the saved active document/project
through ExtendScript and exposes checkout, add, sync, validate, submit, and
revert.

CEP is used because current UXP host APIs do not provide a general mechanism to
run `oad` with arguments and capture its structured output. The extension loads
only static local content and uses a fixed command allowlist.

## DaVinci Resolve Studio

Extract `openasset-depot-resolve-0.1.0.zip`. On macOS, run the included installer
with administrator rights from a machine that has Resolve Studio's developer
files:

```sh
sudo ./com.openassetdepot.resolve/install-macos.sh
```

The installer copies the current Blackmagic-provided `WorkflowIntegration.node`;
OpenAsset Depot does not redistribute that binary. Restart Resolve Studio and
open **Workspace > Workflow Integrations > OpenAsset Depot**.

Select Media Pool clips, choose the workspace, then checkout/add/validate/submit
their underlying source files. Resolve project libraries remain in Resolve's
database; media, graphics, LUTs, scripts, and exported `.drp` archives are the
assets versioned by OpenAsset Depot. The plug-in follows Resolve 19.0.2+'s
sandboxed Electron/context-isolation model and configures a 30-second Resolve API
timeout.
