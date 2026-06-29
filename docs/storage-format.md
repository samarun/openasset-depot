# Storage Format

The local object store is content-addressed and immutable.

```text
objects/
  chunks/ab/cd/<chunk_blake3>
  blobs/ef/12/<blob_blake3>.json
  uploads/tmp/<upload_id>.upload
```

Files are streamed into bounded buffers. Each chunk receives a BLAKE3 hash and is written once. The whole-file BLAKE3 hash becomes the blob hash. Blob manifests record ordered chunk references:

```json
{
  "hash": "file blake3",
  "size_bytes": 123,
  "chunks": [
    { "hash": "chunk blake3", "size_bytes": 4194304 }
  ]
}
```

PostgreSQL stores the durable metadata in `blobs`, `chunks`, and `blob_chunks`. File revisions point to immutable blob hashes.

## Integrity Verification

Admins can verify recent blob records without loading full files into memory:

```sh
curl -s "http://127.0.0.1:8080/api/admin/storage/verify?limit=100" \
  -H "authorization: Bearer $TOKEN"
```

Verification reads each blob manifest, streams chunk contents through BLAKE3, checks chunk hashes/sizes, recomputes the whole-blob hash, and reports failed blobs.

## Orphan Cleanup

Always inspect a dry run first:

```sh
curl -s http://127.0.0.1:8080/api/admin/storage/cleanup \
  -H "authorization: Bearer $TOKEN" \
  -H "content-type: application/json" \
  -d '{"dry_run":true,"older_than_hours":24}'
```

After backup verification, repeat with `dry_run:false`. Cleanup only considers
objects older than the requested age, checks references in PostgreSQL in bounded
batches, caps deletion count, and is mutually exclusive with submit object
registration. Every run is audited.

## Backup

Back up both PostgreSQL and object storage. Losing either one makes the depot incomplete.

```sh
docker compose -f deploy/docker-compose.yml --env-file deploy/.env exec postgres pg_dump -U openasset openasset > openasset.sql
docker run --rm -v openasset-depot_openasset_objects:/objects -v "$PWD":/backup alpine tar czf /backup/openasset-objects.tgz /objects
```

## Restore

Restore the database dump and object volume before starting the API.

```sh
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d postgres
docker compose -f deploy/docker-compose.yml --env-file deploy/.env exec -T postgres psql -U openasset openasset < openasset.sql
docker run --rm -v openasset-depot_openasset_objects:/objects -v "$PWD":/backup alpine sh -c "cd / && tar xzf /backup/openasset-objects.tgz"
```
