# OpenAsset Depot Architecture

OpenAsset Depot is a centralized version-control and asset-management system for large creative repositories. The core is application-agnostic; native host integrations translate artist actions into one stable CLI protocol while authoritative rules remain on the server.

## Layers

- API layer: Axum handlers, auth checks, request validation, upload/download streaming.
- Service layer: depot, stream, workspace, lock, changelist, submit, sync, and validation workflows.
- Repository layer: SQLx dynamic queries and explicit PostgreSQL transactions.
- Permission layer: user-level depot permission grants with owner/admin bypass.
- Storage layer: immutable content-addressed local object store using BLAKE3 chunks and blob manifests.
- File type layer: exact, directory, extension, fallback rule matching with precompiled glob/regex support.
- Adapter layer: app-specific DCC rules and rule-based scanners outside the core storage/submit logic.
- Audit layer: append-only database table for sensitive actions.
- CLI layer: `oad` command that stores workspace metadata under `.oad/`.
- Desktop layer: Tauri application with a bundled CLI sidecar for real local file operations.
- Host layer: Blender, Maya, Houdini, Nuke, Unreal, Unity, Adobe CEP, and Resolve Workflow Integration packages.

## SDLC Milestones

1. Requirements and architecture notes.
2. PostgreSQL schema and constraints.
3. Content-addressed storage engine.
4. Transactional locking engine.
5. Changelist and submit engine.
6. Sync/status engine.
7. File type and validation rules.
8. REST API.
9. CLI.
10. Docker Compose.
11. Tests.
12. Documentation.
13. Refactoring and cleanup.

## Important Data Structures

- Hash maps: CLI workspace state maps depot path to local metadata for `O(1)` status lookup.
- Hash sets: CLI validation/status uses sets for changed files, seen files, and Unity `.meta` detection.
- Ordered DB indexes: file history, path traversal, lock queries, changelist queries, and audit queries rely on PostgreSQL indexes.
- Permission maps: `depot_user_permissions` maps depot/user pairs to read, write, or admin roles for indexed access checks.
- Idempotency records: `idempotency_keys` stores request hashes and replayable JSON responses for retry-safe create endpoints.
- Directed graph: `asset_dependency_edges` stores source-to-target asset edges,
  scan confidence, status, and adapter ownership for indexed traversal.
- Append-only logs: `audit_events` is protected by a trigger and records login, lock, submit, revert, and validation events.
- Immutable blob/chunk records: blobs, chunks, blob manifests, and file revisions are immutable after insert.
- Workspace revision map: `workspace_file_states` maps path to acknowledged revision for indexed incremental sync.
- Merkle-ready layout: blob hashes plus ordered chunk hashes can evolve into workspace/depot Merkle comparison.

## Performance Notes

- Targeted status: `O(k log n + bytes changed)` for `k` requested local files; hash maps avoid pairwise comparisons.
- Full local status: `O(n + bytes hashed)` and is an explicit CLI operation, not used for UI pending discovery.
- Steady-state sync plan: `O(log n + c)` where `c` is changed paths, using `(workspace_id, depot_path)` acknowledgements and keyset pages.
- Initial sync: `O(n + bytes transferred)`, which is unavoidable for a new workspace, processed in bounded 1,000-file pages.
- Submit: `O(f + c)` where `f` is submitted files and `c` is chunks; no `O(n^2)` path comparisons.
- Lock/unlock: `O(log n)` indexed insert/update; uniqueness enforced by PostgreSQL.
- History: `O(log r + page)` through `(file_id, revision_number)` and stream/path indexes.
- Permission checks: `O(log p)` through depot/user permission indexes, with constant-time owner/admin bypass after depot lookup.
- Idempotency replay: `O(log k)` by unique key lookup.
- Storage verification: `O(c + b)` for checked blobs, where `c` is chunk count and `b` is bytes streamed.
- Lock and audit pages: `O(log n + page)` with descending timestamp/UUID keysets.
- Dependency pages: `O(log e + page)` through stream/source/scan-time indexes.
- Orphan cleanup: `O(o log r)` for `o` on-disk objects and batched indexed
  reference checks; memory remains `O(1000)` candidates.
- Rule matching: exact path `O(1)`, extension `O(1)`, directory rules `O(d)`, fallback `O(1)`.

## Concurrency

Exclusive locks are protected by case-insensitive partial unique indexes on active stream paths. Submit locks the changelist and file rows in one transaction before creating revisions, workspace acknowledgements, and lock releases. Create endpoints use `Idempotency-Key`; identical multipart submit retries return the original revision result. Failed submits roll back metadata and may leave only unreferenced immutable objects, which the admin cleanup endpoint can remove after its age threshold.

Submit holds a shared storage-maintenance lease from upload through metadata
commit. Orphan cleanup requires the exclusive lease, preventing a deduplicated
object from disappearing between existence detection and revision commit.
