# Roadmap

## Recently Completed Foundation

- Production navigation hides Reviews, Stage, and shelving until their backend
  contracts exist; global admin claims now gate Admin navigation.
- Artist statuses use eight server-backed labels, and demo/offline state remains
  visibly distinct from connected data.
- Sync and submit support additive CLI integration protocol v2 progress streamed
  through Tauri while v1 remains compatible with existing host plug-ins.
- Submit is a single automatically validated sheet with protective conflict copy.
- Shared design/motion tokens, deterministic preview art, a laptop inspector
  drawer, and first-admin studio onboarding are in the desktop app.
- Ubuntu production Compose keeps data services private and terminates automatic
  HTTPS at Caddy.

## Next Production Hardening

- Add group-based and stream-specific permissions.
- Extend `Idempotency-Key` records to every remaining mutating JSON endpoint;
  multipart submit retries already compare content and replay original revisions.
- Add scheduled execution around the existing integrity verification and manual,
  race-safe orphan cleanup endpoints.
- Add S3-compatible object storage with multipart upload support.
- Add admin repair/report export workflows for failed storage verification.

## Scale And Workflow

- Add Merkle-style local workspace comparison for faster explicit full-status
  scans; server sync planning is already revision-acknowledgement based.
- Add recursive dependency sync and impact analysis on `asset_dependency_edges`.
- Add shelves, code/asset review workflows, and stream merge/copy operations.
- Add admin force unlock with audit trail and optional lease expiration.
- Add resumable large-file upload/download.

## Tooling

- Add licensed host automation matrices and host-specific dependency extractors
  to the existing Unreal, Unity, Blender, Maya, Houdini, Nuke, Adobe, and Resolve integrations.
- Publish versioned Rust and TypeScript SDK packages.
- Publish signed/notarized desktop and host artifacts from release CI.
