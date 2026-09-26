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

### Idempotency

Mutating JSON endpoints accept an optional `Idempotency-Key` header. Reusing the
same key with the same request returns the original response; reusing it with a
different request returns `409 Conflict`. Keys are scoped to the authenticated
user, so one user cannot replay another user's response.

Covered endpoints:

| Endpoint | Operation |
|----------|-----------|
| `POST /api/depots` | `depot_create` |
| `POST /api/streams` | `stream_create` |
| `POST /api/workspaces` | `workspace_create` |
| `DELETE /api/workspaces/{id}` | `workspace_delete` |
| `POST /api/changelists` | `changelist_create` |
| `POST /api/files/add`, `/edit`, `/delete` | `file_op` |
| `POST /api/files/revert` | `file_revert` |
| `POST /api/files/lock` | `file_lock` |
| `POST /api/files/unlock` | `file_unlock` |
| `POST /api/admin/locks/force-unlock` | `file_force_unlock` |
| `POST /api/depots/{id}/permissions` | `depot_permission_grant` |
| `POST /api/depots/{id}/group-permissions` | `depot_group_permission_grant` |
| `DELETE /api/depots/{id}/group-permissions/{group_id}` | `depot_group_permission_revoke` |
| `POST /api/groups` | `group_create` |
| `POST /api/groups/{id}/members` | `group_member_add` |
| `DELETE /api/groups/{id}/members/{user_id}` | `group_member_remove` |
| `POST /api/reviews/comments` | `asset_review_comment_create` |
| `POST /api/reviews/comments/{id}/resolve` | `asset_review_comment_resolve` |
| `POST /api/sync/ack` | `sync_ack` |
| `POST /api/reviews/requests` | `review_request_create` |
| `POST /api/reviews/requests/{id}/decision` | `review_request_decision` |
| `POST /api/reviews/requests/{id}/close` | `review_request_close` |

Path parameters are folded into the hashed request, so one key cannot replay
across two depots, groups, or workspaces.

For `DELETE` routes a key converts the second attempt from `404 Not Found` into
a replay of the original success, which is what a retrying client needs.

`POST /api/changelists/{id}/submit` is multipart rather than JSON and does not
use this table: identical submit retries are detected by comparing uploaded
content, and the original revision result is replayed. `POST /api/users` relies
on the `username` unique constraint instead, because the first-user bootstrap
call has no authenticated actor to scope a key to.

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
- `POST /api/changelists/{id}/shelve`
- `GET /api/changelists/{id}/shelve`
- `DELETE /api/changelists/{id}/shelve`
- `POST /api/changelists/{id}/unshelve`
- `GET /api/shelves?workspace_id=<uuid>`
- `GET /api/shelves/content?changelist_id=<uuid>&path=<depot-path>`
- `GET /api/workspaces/{id}/collaborators`

Shelving uses the same multipart shape as submit but creates no revision and
needs no lock. Restoring puts the parked files back as pending operations.

`GET /api/workspaces/{id}/collaborators` lists people who can open the depot
(assigned members plus system admins) so a review can be assigned without an
admin-only permission listing.

Submit uses multipart form data:

- `workspace_id`: UUID text field.
- `file`: one or more streamed file parts. Each part filename is the normalized depot path.
- `staged`: optional JSON text field, `[{ "path": ..., "blob_hash": ... }]`, naming files already staged through a resumable upload session.

Added/edited paths must each have exactly one content part, supplied either as a
`file` part or a `staged` entry; deleted paths must not. Submit is atomic.
Retrying a submitted changelist with the same content returns its original
revisions, while different retry content returns `409`.

## Resumable Transfers

Both directions of a large transfer can continue after a dropped connection.

### Download

`POST /api/sync/download` advertises `Accept-Ranges: bytes` and honours a
`Range: bytes=<offset>-` request header, answering `206 Partial Content`. The CLI
keeps its partial file named by blob hash, so a partial download is always a
valid prefix of the blob still wanted — including across separate `oad sync`
runs after a crash.

### Upload

Submit is atomic and multipart, so an interrupted upload of a large plate or
level file previously meant re-sending every byte. An upload session stages one
file across as many requests as it takes:

- `POST /api/uploads` — body `{ workspace_id, path, size_bytes? }`. Returns the
  session, including `received_bytes`. Re-requesting the same workspace and path
  returns the existing open session rather than creating a second one, so a
  client that lost its upload id can still resume.
- `POST /api/uploads/{id}` — raw body, with `X-Upload-Offset: <byte count>`.
  Appends at that offset and returns the new `received_bytes`. The offset is
  verified against the staged length; a mismatch returns `409` naming the offset
  to resume from, because writing at the wrong place would corrupt the file
  while still producing a valid-looking blob.
- `GET /api/uploads/{id}` — returns `received_bytes` so a client can discover
  where to resume.
- `POST /api/uploads/{id}/finalize` — ingests the staged bytes into the
  content-addressed chunk store and returns `{ blob_hash, size_bytes }`. Calling
  it again replays the original hash. Submit then names that hash in its
  `staged` field.

Sessions are private to the user that opened them, since staged bytes are
unverified content. Deduplication, manifests, and revision commit are unchanged:
staging only decides how the bytes arrive.

The CLI stages files of 64 MiB or larger and sends smaller ones directly, since
a small file is cheaper to re-send than to negotiate a session for. Unfinished
sessions idle for 48 hours are reclaimed by the maintenance worker.

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
- `GET /api/reviews/requests?workspace_id=<uuid>&state=<open|approved|changes_requested|closed>&awaiting_me=<bool>`
- `POST /api/reviews/requests`
- `GET /api/reviews/requests/{id}?workspace_id=<uuid>`
- `POST /api/reviews/requests/{id}/decision`
- `POST /api/reviews/requests/{id}/close`

A review request targets one immutable file revision and at least one reviewer
who can already read the depot. The request stays `open` until every reviewer
approves, or becomes `changes_requested` as soon as one reviewer asks for
changes. Closing is the requester or a system admin abandoning the review
without a verdict. A later submit of the same path is a new revision and needs
a new review.

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
- `GET /api/dependencies/impact?workspace_id=<uuid>&path=<depot-path>`
- `GET /api/audit?limit=250`

`/api/dependencies/impact` reports both directions around one file: assets that
reference it (`required_by`) and files it references (`depends_on`). It is
workspace-authorized.

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
