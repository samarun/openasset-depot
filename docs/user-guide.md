# OpenAsset Depot User Guide

This guide uses `https://asset.arunsamuel.com` as the server URL. If HTTPS is
not configured yet, use `http://asset.arunsamuel.com` temporarily on the trusted
LAN, then switch every workstation to HTTPS before production use. Passwords and
session tokens must not be sent over untrusted plain HTTP.

## Accounts: there is no default password

OpenAsset Depot deliberately ships with **no default username or password**.
The first user is created exactly once through the API and is automatically made
a system administrator. The password supplied in that request becomes the real
password; passwords are stored as Argon2 hashes.

Create the first administrator after `/ready` reports success:

```sh
ADMIN_PASSWORD="$(openssl rand -base64 24)"

curl -fsS http://127.0.0.1:5173/api/users \
  -H 'content-type: application/json' \
  -d "{\"username\":\"admin\",\"password\":\"$ADMIN_PASSWORD\",\"display_name\":\"Depot Administrator\"}" \
  && printf '\nSave this admin password now: %s\n' "$ADMIN_PASSWORD"
```

Run this command on the Ubuntu server and save the generated password in the
studio password manager. Text such as `REPLACE-WITH-...` in examples is an
instructional placeholder and must never be entered as a password. The password
must contain at least eight characters; the command above generates a stronger
random value. A successful response contains the new user ID and
`"is_admin":true`. If an unauthenticated request returns `401`, the first user
already exists. Do not delete the PostgreSQL volume to reset a password; that
would delete depot metadata. This version does not yet provide a self-service
password-reset screen.

Open `https://asset.arunsamuel.com` in a browser and sign in with the account
just created. The Server field should be the origin above, without `/api`.

After login, **Change Password** is under **Settings > Account Security**. A
second **Sign out** action remains visible in the top bar from every page.

## Create a depot, stream, users, and permissions

After signing in as the administrator, use **Start Studio Setup** or **Admin** to
create a depot such as `StudioAssets` and a stream named `main`. A depot is the
top-level production; a stream is the shared line of revisions.

The current web Admin page creates depots and streams. Additional users and
their depot roles are currently provisioned through the REST API. Install `jq`
on an administrator machine for the following example:

```sh
BASE_URL=https://asset.arunsamuel.com

ADMIN_TOKEN="$(
  curl -fsS "$BASE_URL/api/auth/login" \
    -H 'content-type: application/json' \
    -d '{"username":"admin","password":"YOUR-ADMIN-PASSWORD"}' |
    jq -r .token
)"

ARTIST_ID="$(
  curl -fsS "$BASE_URL/api/users" \
    -H "authorization: Bearer $ADMIN_TOKEN" \
    -H 'content-type: application/json' \
    -d '{
      "username":"artist1",
      "password":"REPLACE-WITH-ANOTHER-UNIQUE-PASSWORD",
      "display_name":"Artist One",
      "is_admin":false
    }' |
    jq -r .id
)"

DEPOT_ID="$(
  curl -fsS "$BASE_URL/api/depots" \
    -H "authorization: Bearer $ADMIN_TOKEN" |
    jq -r '.[] | select(.name == "StudioAssets") | .id'
)"

curl -fsS "$BASE_URL/api/depots/$DEPOT_ID/permissions" \
  -H "authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' \
  -d "{\"user_id\":\"$ARTIST_ID\",\"role\":\"write\"}"
```

Available depot roles are:

- `read`: create a workspace and sync assets.
- `write`: read, check out, add, delete, and submit assets.
- `admin`: write plus manage that depot's permissions.

System administrators can access every depot. Normal artists usually need the
`write` role.

The login screen exposes **Sign up** when `OAD_ALLOW_SIGNUPS=true`. Self-created
accounts are never system administrators and have no depot access until an
administrator grants a role. After onboarding, set `OAD_ALLOW_SIGNUPS=false` in
the server `.env` and recreate the API container.

## Browser uploads and desktop sync

The web Asset Browser can select one or more local files, validate them, and
submit them as one server changelist. Choose **Upload Files**, review the submit
sheet, enter a description, and submit. Selecting the same depot path again
creates an edit rather than a second asset. Server lock rules still apply;
lock-required DCC packages should normally be checked out and submitted from
their host integration.

Browser and desktop synchronization is server-mediated:

1. A browser upload becomes an immutable depot revision.
2. Desktop/DCC workspaces choose **Sync Latest** to download and verify it.
3. A desktop/DCC submit becomes another depot revision.
4. The browser chooses **Refresh Depot** to display it.

The browser does not continuously mirror an arbitrary local project directory.
Browsers cannot safely retain unrestricted filesystem access, so persistent
project-folder synchronization remains a desktop/CLI responsibility.

## Creative review, playback, comments, and annotations

Select an asset in **Assets**, then choose **Review & Annotate** in the inspector.
Review feedback is stored against the selected immutable revision, so a note on
version 4 never silently moves to version 5.

The review studio supports:

- Native browser playback for PNG/JPEG/WebP/GIF images, MP4/WebM video, and
  MP3/WAV/OGG audio.
- Interactive FBX, GLB, and glTF review with orbit controls, animation selection,
  play/pause/loop, previous/next 24 fps frame stepping, elapsed time, wireframe,
  and skeleton overlays.
- Comments captured at the current media time and frame, with one-click seeking
  back to that feedback point.
- Pen, highlighter, rectangle, and arrow sketches with adjustable color and size.
- Per-comment sketch visibility plus resolve and reopen actions.

Native `.blend`, `.ma`, `.mb`, `.hip`, `.uasset`, and `.umap` files are proprietary
DCC containers and cannot safely execute inside a web browser. Their submitted
camera/viewport preview remains reviewable as a still. For interactive review,
attach a portable GLB, FBX, MP4, or WebM proxy to the same revision:

```sh
oad --cwd /absolute/path/to/MyProject integration review-proxy \
  Scenes/Shot.blend --media /tmp/Shot-review.glb
```

The Blender integration creates an animated GLB proxy automatically after a
successful submit when **Generate interactive 3D review proxy** is enabled.
Maya, Houdini, Unreal, and other studio integrations can use the command above
with a host-generated playblast or model export. The proxy is immutable and
stored in the same BLAKE3 chunk store as the source revision.

## Prepare each artist workstation

The normal artist setup does not require a terminal:

1. Open the native OpenAsset Depot desktop app and sign in.
2. Choose **Connect Project Folder**.
3. Select the production and choose the existing Blender, Maya, Houdini, Nuke,
   or Unreal project folder.
4. Choose **Create & Start**.

The desktop app creates the private `.oad` workspace metadata and securely
shares its session with installed creative plug-ins. On Unix, the local session
file is restricted to the current user with mode `0600`. Passwords and tokens
are never written into project files. Signing out of the desktop app clears the
shared plug-in token. On macOS, plug-ins automatically find the CLI bundled in
`OpenAsset Depot.app`; artists do not configure a binary path.

The `oad` CLI remains available for build machines, automation, troubleshooting,
and technical users. It is the engine behind the UI rather than a required
artist workflow.

### Optional CLI setup

Build the CLI on each workstation, or distribute a trusted build made for that
operating system:

```sh
cargo build --release -p oad
sudo install -m 0755 target/release/oad /usr/local/bin/oad
oad --version
```

On Windows, build `target\release\oad.exe`, put it in a studio tools directory,
and add that directory to `PATH`. A CLI built for the Ubuntu server cannot be
copied to Windows or macOS; each operating system needs its own binary.

If the desktop app is not installed, log in once as the artist:

```sh
oad --server https://asset.arunsamuel.com login artist1 \
  --password 'THE-ARTIST-PASSWORD'
```

The server URL, username, and session token are stored in `~/.oad/config.json`.
With the default server configuration the token expires after 24 hours; run the
login command again when a host reports that authentication is required.

## Create the local workspace

Every artist and build machine should have its own workspace. The workspace root
is the Blender, Maya, or Unreal project root and contains a private `.oad`
metadata directory.

Use **Connect Project Folder** in the desktop app for the normal setup. The
equivalent advanced CLI flow is:

```sh
oad workspace create artist1-main \
  --depot StudioAssets \
  --stream main \
  --path /absolute/path/to/MyProject

cd /absolute/path/to/MyProject
oad integration context
oad sync
```

`oad integration context` should report the server, username, workspace, and
`"authenticated":true`. Create the workspace with the native desktop app or CLI,
not only the browser: host plug-ins require the local `.oad/workspace.json`.

For the first import, add source assets with the desktop app or CLI and submit a
meaningful initial changelist:

```sh
cd /absolute/path/to/MyProject
oad change create "Initial project import"
oad add Scenes/Opening.blend
oad add SourceAssets/Character.ma
oad submit
```

Add source files and project metadata, but exclude generated caches and build
outputs. Examples include Unreal `DerivedDataCache`, `Intermediate`, `Saved`,
and `Binaries` directories.

## Shared daily workflow

1. Run **Sync Latest** before opening the scene or Unreal project.
2. Check out a tracked binary scene/package before editing it. This creates the
   pending edit and obtains an exclusive lock.
3. Save normally in the creative application.
4. Validate the work and resolve errors.
5. Enter a useful description and submit changes.
6. The successful submit creates immutable revisions and releases their locks.

Every desktop and creative-host action shows a live status. Sync and submit
include phase and percentage updates; shorter actions such as checkout, add,
validate, and revert show their current operation and a clear completion or
recovery message.

**Revert Intent** removes the pending source-control operation and releases
the lock, but intentionally preserves the local creative file. It is not an
“erase my edits” command.

## Blender

The packaged add-on is:

`dist/integrations/openasset-depot-blender-0.1.2.zip`

1. In Blender, open **Edit > Preferences > Add-ons**.
2. Choose **Install from Disk** and select the ZIP without extracting it.
3. Enable **OpenAsset Depot**.
4. Install the platform-specific add-on package to use its bundled `oad` CLI.
   Leave both preference overrides blank; they are only needed for development
   or a nonstandard server configuration. The URL saved by `oad login` is used.
5. Save the `.blend` file somewhere inside the initialized workspace.
6. In the 3D View press **N**, then open the **OpenAsset** tab.

For an existing tracked scene, select **Refresh Status**, then **Check Out**
before editing. Save, **Validate**, enter a description, and **Submit Changes**.

If the scene was modified before checkout, **Submit Changes** verifies that the
workspace is current, obtains the exclusive lock, marks the tracked scene for
edit, and submits it automatically. If a newer revision or another artist's
lock exists, the panel stops with a human-readable next action.

For a brand-new `.blend`, save it inside the workspace and select **Add to
Depot**, then submit. **Submit Changes** also detects an untracked current scene
and marks it for addition automatically. The equivalent CLI command is:

```sh
oad --cwd /absolute/path/to/MyProject integration add Scenes/NewShot.blend
```

The Blender panel submits all pending files in the workspace. Sync before
opening Blender when possible; if a sync replaces the currently open `.blend`
on disk, reopen the scene before continuing.

Platform packages such as `openasset-depot-blender-macos-arm64-0.1.2.zip`
include the matching CLI. The CLI and server fields are advanced overrides and
normally remain blank. With browser previews enabled, Blender renders a small
camera image and attaches it to the immutable submitted revision without adding
a sidecar file to the workspace. It also exports an animated GLB review proxy,
allowing browser orbit, animation playback, frame stepping, comments, and sketch
annotations without exposing or converting the original `.blend` file.

## Maya

The packaged module is:

`dist/integrations/openasset-depot-maya-0.1.0.zip`

1. Extract the archive into a Maya module directory. The extracted
   `OpenAssetDepot.mod` file and `OpenAssetDepot/` directory must be siblings.
2. Ensure their parent directory is listed in `MAYA_MODULE_PATH`.
3. Restart Maya and open **Windows > Settings/Preferences > Plug-in Manager**.
4. Load `openasset_depot_maya.py`; enable Auto load for studio workstations.
5. Open **OpenAsset > Workspace**.
6. Use **Settings** if Maya cannot find `oad`, or to override the server URL.

Save the `.ma` or `.mb` inside the workspace. For a tracked scene, use **Check
Out Scene**, save, validate, enter a description, and submit. Add a brand-new
scene through the desktop app or CLI first:

```sh
oad --cwd /absolute/path/to/MyProject integration add Scenes/NewCharacter.ma
```

Network and file operations run outside Maya's UI thread, but wait for the panel
operation to finish before closing the scene or Maya.

## Unreal Engine

The packaged source plug-in is:

`dist/integrations/openasset-depot-unreal-0.1.0.zip`

It supports Unreal Engine 5.4–5.6 and must be compiled against the installed
engine; the archive is source code, not a universal precompiled binary.

1. Close Unreal Editor.
2. Extract the archive so the project contains
   `<Project>/Plugins/OpenAssetDepotSourceControl/OpenAssetDepotSourceControl.uplugin`.
3. Ensure `<Project>/.oad/workspace.json` already exists.
4. Regenerate project files and build the project's Editor target using an
   Unreal-supported Visual Studio/Xcode toolchain.
5. If `oad` is not on `PATH`, define `OAD_CLI` as its absolute path before
   starting Unreal Editor.
6. Open the project and choose **Revision Control > Connect to Revision Control**.
7. Select **OpenAsset Depot**.

The provider maps Unreal's **Update Status**, **Check Out**, **Mark for Add**,
**Delete**, **Revert**, **Sync**, and **Check In** operations to OpenAsset Depot.
Check out `.uasset` and `.umap` packages before editing. Use Unreal's normal
**Mark for Add** for new packages and **Check In** to submit with a description.

Do not add generated Unreal directories such as `DerivedDataCache`,
`Intermediate`, `Saved`, or `Binaries`. Sync before launching the Editor so
loaded packages are not stale in memory.

## Troubleshooting host integrations

Run these checks from the artist workstation, replacing the project path:

```sh
curl -fsS https://asset.arunsamuel.com/ready
oad --server https://asset.arunsamuel.com login artist1 --password 'PASSWORD'
oad --cwd /absolute/path/to/MyProject integration context
oad --cwd /absolute/path/to/MyProject integration status
```

- **CLI not found:** put `oad` on `PATH`, set the plug-in's CLI path, or set
  `OAD_CLI` for Unreal.
- **Workspace metadata not found:** create the workspace at the actual project
  root; do not copy another artist's `.oad` directory.
- **Not logged in / unauthorized:** log in again and confirm the user has a
  depot role.
- **File is outside the workspace:** save or move it below the workspace root.
- **Checked out elsewhere:** the other artist must submit or revert to release
  the exclusive lock.
- **Needs Sync:** sync before checkout; OpenAsset refuses to overwrite local
  pending or modified files.
- **Validation failed:** resolve reported missing dependencies, generated paths,
  missing Unity `.meta` pairs, or lock requirements before submit.

The packaged archives are accompanied by `dist/integrations/SHA256SUMS`; verify
them before distributing the tools to artist workstations.
