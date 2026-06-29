# CLI

The CLI binary is `oad`.

```sh
oad login alice --password "correct horse battery staple"
oad depot create Game
oad stream create main --depot Game
oad workspace create alice-main --depot Game --stream main --path ~/work/Game
cd ~/work/Game
oad change create "Initial Unreal content"
oad add Content/Maps/Main.umap
oad lock Content/Maps/Main.umap --reason "layout pass"
oad submit
```

Workspace metadata lives under `.oad/`:

- `workspace.json`: workspace id, depot, stream, root, active changelist.
- `state.json`: synced file revisions and local BLAKE3 hashes.
- `pending.json`: local add/edit operations before submit.

Metadata writes use atomic replacement. On Unix the files are mode `0600`.
Workspace operations take an exclusive `.oad/operation.lock` so two host
processes cannot mutate local state concurrently.

Commands:

- `oad login`
- `oad depot create/list`
- `oad stream create/list`
- `oad workspace create/list/remove`
- `oad sync`
- `oad status`
- `oad add`
- `oad edit`
- `oad delete`
- `oad lock`
- `oad unlock`
- `oad change create`
- `oad submit`
- `oad revert`
- `oad history`
- `oad locks`
- `oad filetypes list`
- `oad adapters list`
- `oad adapters detect`
- `oad adapters scan`
- `oad adapters validate`
- `oad validate`

Host/desktop protocol commands:

```sh
oad --cwd /path/to/project integration context
oad --cwd /path/to/project integration pending
oad --cwd /path/to/project integration status Scenes/Shot.blend
oad --cwd /path/to/project integration checkout Scenes/Shot.blend
oad --cwd /path/to/project integration add Textures/New.psd
oad --cwd /path/to/project integration delete Textures/Old.psd
oad --cwd /path/to/project integration sync
oad --cwd /path/to/project integration validate Scenes/Shot.blend --adapter blender
oad --cwd /path/to/project integration submit --description "Lighting pass"
```

Every integration command prints one JSON envelope with
`protocol_version`, `ok`, `data`, and `error`. `OAD_TOKEN` and `OAD_USERNAME`
can provide nonpersistent process-scoped credentials for the native desktop app.
The normal `oad login` flow stores the CLI session under `~/.oad/config.json`.

Protocol v1 remains the default for existing host integrations. Callers that
need live progress can opt into additive v2:

```sh
oad --cwd /path/to/project integration --protocol-version 2 sync
```

V2 writes newline-delimited JSON. Zero or more `type: "progress"` messages are
followed by exactly one `type: "result"` message. Progress includes the operation,
phase, human-readable message, completed units, and total units; consumers must
ignore fields they do not recognize.
