# OpenAsset Depot

[![CI](https://github.com/samarun/openasset-depot/actions/workflows/ci.yml/badge.svg)](https://github.com/samarun/openasset-depot/actions/workflows/ci.yml)
[![Release](https://github.com/samarun/openasset-depot/actions/workflows/release.yml/badge.svg)](https://github.com/samarun/openasset-depot/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

OpenAsset Depot is an open-source centralized version-control and asset-management
system for game development, VFX, animation, virtual production, Unreal Engine,
Unity, Blender, Maya, Houdini, Nuke, Adobe Creative Cloud, and DaVinci Resolve.
It is built in Rust and does not use Git as its storage backend.

The repository contains a PostgreSQL-backed API, immutable BLAKE3 chunk storage,
the `oad` CLI, a native Tauri desktop app, typed Rust/TypeScript SDKs, and host
integrations. Core locking, permissions, validation, submit, sync, and audit
rules stay on the server; host plug-ins use the stable `oad integration` protocol.

Live demo: https://asset.arunsamuel.com (i'm runnong this on my home Linux System, there might be a downtime if there's a power outage)

username: admin
password: asset@1234

## What ships

- Rust API with PostgreSQL metadata, immutable BLAKE3 chunk storage, audit logs,
  permissions, validation, idempotent submit, locking, sync, and history.
- Native Tauri desktop app with role-aware navigation, first-run onboarding,
  streaming sync/submit progress, responsive asset inspection, and light/dark themes.
- `oad` CLI plus typed Rust and TypeScript SDKs.
- Integrations for Unreal, Unity, Blender, Maya, Houdini, Nuke, Adobe Creative
  Cloud, and DaVinci Resolve.
- Local Docker Compose and an Ubuntu production topology with automatic HTTPS.

## Screenshots

### Production dashboard

![OpenAsset Depot production dashboard](docs/screenshots/dashboard.jpg)

| Sign in | Submit validation |
| --- | --- |
| ![OpenAsset Depot sign-in screen](docs/screenshots/login.jpg) | ![OpenAsset Depot submit workflow](docs/screenshots/submit.jpg) |

### Responsive asset inspector

![OpenAsset Depot responsive asset inspector](docs/screenshots/inspector.jpg)

## Start the server

```sh
cp deploy/.env.example deploy/.env
openssl rand -base64 48
# Put the generated value in OAD_JWT_SECRET and replace every change-me value.
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d --build
```

The web companion is at `http://127.0.0.1:5173`, the API is at
`http://127.0.0.1:8080`, and readiness is reported by `/ready`.

Bootstrap the first admin exactly once:

```sh
curl -fsS http://127.0.0.1:8080/api/users \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"use-a-long-studio-password","display_name":"Depot Admin"}'
```

After bootstrap, only an authenticated system admin can create more users.

For an Internet-facing Ubuntu host with automatic HTTPS, point a domain at the
server and run `./deploy/ubuntu-up.sh depot.example.com`. The production Compose
file exposes only ports 80/443; see [Deployment](docs/deployment.md) for firewall,
bootstrap, upgrade, and backup instructions.

## Native desktop app

Build the signed-ready app payload and bundled CLI:

```sh
./scripts/build-desktop-release.sh
```

macOS local builds receive a valid ad-hoc signature. Set
`OAD_MACOS_SIGNING_IDENTITY="Developer ID Application: Studio Name (TEAMID)"`
and Apple's notarization environment variables for a distributable studio build.

On macOS the output is under
`desktop/src-tauri/target/<target>/release/bundle/` as an `.app` and DMG. Open the
native app, enter the API URL, and sign in. In **Admin**, create a depot and main
stream. From the workspace chooser, create a workspace in an existing or empty
project folder.

Daily artist flow:

1. **Sync Latest** before opening the project.
2. Use **Add Files** for new assets or **Lock / Check Out** before editing a
   lock-required asset.
3. Save in the DCC host. The Changes view reads the local pending list without a
   full workspace scan.
4. **Validate**, enter a submit description, and **Submit Changes**.
5. Use **Mark for Delete** for tracked assets. The local file is removed only
   after the tombstone submit succeeds.

`Revert Intent` removes the pending operation and releases its lock; it preserves
the local file so creative work is never silently destroyed.

## CLI

```sh
cargo build --release -p oad
./target/release/oad --server http://127.0.0.1:8080 login admin \
  --password 'use-a-long-studio-password'

oad depot create MyGame
oad stream create main --depot MyGame
oad workspace create artist-main --depot MyGame --stream main --path ~/work/MyGame
cd ~/work/MyGame
oad sync
```

Use `oad integration --help` for the machine-readable protocol used by the
desktop and host plug-ins. Workspace metadata is private, atomically replaced,
and stored under `.oad/`.

## Host integrations

Build all distributable integration archives and checksums:

```sh
./scripts/package-integrations.py
```

Artifacts are written to `dist/integrations/`. Installation and artist workflows
for Blender, Maya, Houdini, Nuke, Unreal, Unity, Adobe, and Resolve are documented
in [docs/integrations.md](docs/integrations.md).

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd desktop && npm test -- --run && npm run build
```

Run the disposable full API suite without touching a development database:

```sh
docker compose -p oad-test -f deploy/docker-compose.test.yml up -d --build
OAD_TEST_BASE_URL=http://127.0.0.1:18081 \
OAD_TEST_DATABASE_URL=postgres://openasset_test:isolated-test-password@127.0.0.1:55433/openasset_test \
cargo test -p openasset-api --test required_flows -- --ignored --test-threads=1
docker compose -p oad-test -f deploy/docker-compose.test.yml down -v
```

## Releases

`v*` tags run `.github/workflows/release.yml`. The workflow enforces one version
across server, CLI, desktop, SDKs, and host manifests; publishes Linux binaries,
eight integration packages, an SPDX SBOM, checksums, provenance attestations,
GHCR images, and a signed/notarized macOS app. Tagged desktop releases fail when
the required Apple signing/notarization secrets are absent.

## Documentation

- [Getting started](docs/getting-started.md)
- [Host integrations](docs/integrations.md)
- [Architecture](docs/architecture.md)
- [REST API](docs/api.md)
- [CLI](docs/cli.md)
- [Deployment](docs/deployment.md)
- [Backup and restore](docs/backup-restore.md)
- [Testing](docs/testing.md)
- [Known limitations](docs/known-limitations.md)
