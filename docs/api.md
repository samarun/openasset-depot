# REST API

All endpoints except `/health`, `/ready`, `/api/users` for the race-protected first-user bootstrap, and `/api/auth/login` expect `Authorization: Bearer <jwt>`. After bootstrap, `/api/users` requires a system administrator.

## Health

- `GET /health`
- `GET /ready`

## Auth

- `POST /api/users`
- `POST /api/auth/login`
- `GET /api/auth/signup`
- `POST /api/auth/signup`
- `POST /api/auth/change-password`

`GET /api/auth/signup` reports whether controlled self-registration is enabled
and whether the next account will be the first system administrator. Later
self-signups require `OAD_ALLOW_SIGNUPS=true`, create non-admin users, and grant
no depot permissions. Password changes require a bearer token, the current
password, and a new password of at least eight characters.

## Core

- `POST /api/depots`
- `GET /api/depots`
- `GET /api/depots/{id}/permissions`
- `POST /api/depots/{id}/permissions`
- `POST /api/streams`
- `GET /api/streams`
- `POST /api/workspaces`
- `GET /api/workspaces`
- `DELETE /api/workspaces/{id}`

Depot permissions are user-level for the MVP:

- `read`: create/read a workspace and sync from the depot.
- `write`: read plus create streams and write through normal workspace operations.
- `admin`: write plus grant/list depot permissions.

System admins bypass depot permission checks. Depot owners are treated as depot admins.

`POST /api/depots`, `POST /api/streams`, `POST /api/workspaces`, and `POST /api/changelists` accept an optional `Idempotency-Key` header. Reusing the same key with the same request body returns the original response; reusing the key with a different body returns `409 Conflict`.

## Files, Locks, Changelists

- `POST /api/files/add`
- `POST /api/files/edit`
- `POST /api/files/delete`
- `POST /api/files/revert`
- `POST /api/files/lock`
- `POST /api/files/unlock`
- `POST /api/files/preview?workspace_id=<uuid>&path=<depot-path>&revision_number=<n>`
- `GET /api/files/preview?workspace_id=<uuid>&path=<depot-path>&revision_number=<n>`
- `GET /api/locks`
- `GET /api/locks/page?limit=250&before_created_at=<rfc3339>&before_id=<uuid>`
- `POST /api/changelists`
- `POST /api/changelists/{id}/submit`

Submit uses multipart form data:

- `workspace_id`: UUID text field.
- `file`: one or more streamed file parts. Each part filename is the normalized depot path.

Added/edited paths must each have exactly one content part; deleted paths must
not. Submit is atomic. Retrying a submitted changelist with the same content
returns its original revisions, while different retry content returns `409`.

`/api/locks/page` is the scalable lock-list contract. It supports optional
`stream_id`, `workspace_id`, and `user_id` filters, returns at most 1,000 rows,
and orders by `(created_at DESC, id DESC)`. Pass both returned cursor fields to
read the next page. `/api/locks` remains for protocol-v1 compatibility.

Revision previews accept an authenticated raw PNG, JPEG, or WebP request body
up to 8 MiB. Each preview is content-addressed, attached to one immutable file
revision, and streamed only to users with access through the supplied workspace.

## Creative Reviews

- `GET /api/reviews/media?workspace_id=<uuid>&path=<depot-path>&revision_number=<n>`
- `GET /api/reviews/proxy?workspace_id=<uuid>&path=<depot-path>&revision_number=<n>`
- `POST /api/reviews/proxy?workspace_id=<uuid>&path=<depot-path>&revision_number=<n>`
- `GET /api/reviews/comments?workspace_id=<uuid>&path=<depot-path>&revision_number=<n>`
- `POST /api/reviews/comments`
- `POST /api/reviews/comments/{id}/resolve`

Review media and proxy downloads are workspace-authorized, immutable, cacheable,
and support single HTTP byte ranges. Portable proxy uploads accept GLB, glTF,
FBX, MP4, or WebM bodies up to 512 MiB. One immutable proxy can be attached to
each asset revision.

Comments are revision-bound and accept optional `timecode_ms`, `frame_number`,
`parent_comment_id`, and a structured JSON `annotation`. Annotation payloads are
limited to 128 KiB. Resolve requests can resolve or reopen a comment; both
comment creation and resolution are recorded in the audit log.

## Sync And History

- `POST /api/sync/plan`
- `POST /api/sync/ack`
- `POST /api/sync/download`
- `GET /api/files/history?workspace_id=<uuid>&path=<depot-path>&limit=100&offset=0`

Sync plan accepts `path_prefix`, `after_path`, `paths`, `limit` (maximum 5,000),
`include_current`, and `force_full`. Normal clients use keyset pages and acknowledge
successfully installed revisions in batches. History defaults to `limit=100`,
clamps to a maximum of `1000`, and orders by ascending revision number.

## File Types And Validation

- `GET /api/filetypes`
- `POST /api/filetypes/match`
- `POST /api/validate`

Validation returns warnings/errors for lock-required file types, Unity `.meta` gaps, large files, generated cache paths, and invalid depot paths.

## DCC Adapters

- `GET /api/adapters`
- `POST /api/adapters/detect`
- `POST /api/adapters/scan`
- `POST /api/adapters/preview`
- `POST /api/adapters/metadata`
- `POST /api/adapters/validate`

Adapters expose app-specific metadata, rule-based dependency scans, and validation warnings while keeping the core server application-agnostic. Add
`workspace_id` to `/api/adapters/scan` to atomically replace that source file's
persisted graph edges for the workspace stream.

## Dependency Graph And Audit

- `GET /api/dependencies?stream_id=<uuid>&limit=250`
- `GET /api/audit?limit=250`

Dependency reads require depot read permission and optionally filter by
`source_file`. They page by `(scan_time DESC, id DESC)`. Audit reads are system
admin-only, support event/actor/stream/workspace filters, and page by
`(created_at DESC, id DESC)`. Cursor timestamp and UUID fields must always be
supplied together.

## Admin

- `GET /api/admin/storage/verify?limit=100`
- `POST /api/admin/storage/cleanup`

Storage verification is admin-only. It walks recent blob records, reads manifests, streams chunks through BLAKE3, and returns a summary plus failed blob reports without loading large files into memory.

Cleanup is admin-only and defaults to dry-run, a 24-hour minimum age, and a
10,000-object deletion cap. Its JSON body accepts `dry_run`,
`older_than_hours` (1-8,760), and `max_delete` (1-100,000). It scans in bounded
1,000-object batches and serializes against submit registration.
