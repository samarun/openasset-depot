# Roadmap

## Recently Completed Foundation

- Production navigation includes Shelves and Reviews now that their backend
  contracts exist; Stage orchestration remains hidden. Global admin claims
  still gate Admin navigation.
- Artist statuses use eight server-backed labels, and demo/offline state remains
  visibly distinct from connected data.
- Sync and submit support additive CLI integration protocol v2 progress streamed
  through Tauri while v1 remains compatible with existing host plug-ins.
- Submit is a single automatically validated sheet with protective conflict copy.
- Shared design/motion tokens, deterministic preview art, a laptop inspector
  drawer, and first-admin studio onboarding are in the desktop app.
- Ubuntu production Compose keeps data services private and terminates automatic
  HTTPS at Caddy.
- Group-based depot permissions resolve through `effective_depot_permissions`,
  and OIDC single sign-on provisions accounts and syncs group membership.
- Every mutating JSON endpoint accepts `Idempotency-Key`; multipart submit
  retries compare uploaded content and replay the original revisions.
- Uploads and downloads resume: chunked upload sessions with offset tracking on
  the server, `Range` requests on read, and CLI resume on both directions.
- S3-compatible object storage sits behind the same storage trait as the local
  filesystem, selected by `OAD_STORAGE_BACKEND` and built with the `s3` feature.
- Integrity verification runs on a schedule alongside the manual endpoint, and
  admins can force-unlock a file with a mandatory reason and an audit record
  naming the previous holder.
- Pending work can be shelved and restored without creating a revision.
- Review requests assign named reviewers to an immutable revision; the inspector
  reports dependency impact in both directions.
- The browser review runtime is verified with Blender-authored animated GLB and
  FBX assets: direct playback, a GLB proxy attached to an FBX revision,
  timecoded comments, annotation persistence, and comment-to-frame seeking all
  pass against the real API. See [review runtime verification](review-runtime-verification.md).
- Review-proxy uploads use purpose-isolated resumable sessions, and the retained
  raw-body endpoint streams through bounded buffers for older integrations.
- Review proxies preserve exact rational source frame rate and timeline start;
  legacy media no longer receives an invented 24 fps label.
- Project-owned GLB, embedded glTF, and FBX fixtures run in Chromium WebGL CI
  with decoder, animation, console-error, and canvas-pixel assertions.

## Next Production Hardening

- Add admin repair/report export workflows for failed storage verification.
- Extend integrity sweeps from recent-blob sampling to full coverage over time,
  so old blobs cannot go indefinitely unverified.
- Add orphan cleanup for S3 deployments, which currently rely on bucket
  lifecycle rules; cleanup only walks the local filesystem today.
- Add lock lease expiration so abandoned checkouts release without an admin.
- Let the browser build complete single sign-on, and let an existing password
  account link to an SSO identity instead of colliding with it.
- Auto-provision depot roles from identity-provider groups; membership syncs
  today, but an admin must still create the group and grant its permissions.
- Route browser uploads through the resumable upload session API; only the CLI
  and native desktop paths use it today.

## Scale And Workflow

- Add Merkle-style local workspace comparison for faster explicit full-status
  scans; server sync planning is already revision-acknowledgement based.
- Add recursive dependency sync on `asset_dependency_edges`.
- Add stream merge/copy operations and stage orchestration.
- Add review comparison modes (A/B, wipe, onion skin), threaded replies and
  mentions, and a studio preview-generation queue for formats browsers cannot
  decode directly, including USD, Alembic, EXR sequences, and DCC-native scenes.

## Tooling

- Add host-specific dependency extractors to the existing Unreal, Unity,
  Blender, Maya, Houdini, Nuke, Adobe, and Resolve integrations. Licensed Maya,
  Houdini, and Nuke smoke tests already run on self-hosted runners when
  `OAD_LICENSED_HOST_RUNNERS` is set; Blender runs on every push.
- Publish versioned Rust and TypeScript SDK packages.
- Publish signed/notarized desktop and host artifacts from release CI.
