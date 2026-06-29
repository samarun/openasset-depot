# Backup And Restore

Back up PostgreSQL and object storage together. PostgreSQL contains users, depots, streams, workspaces, locks, changelists, revisions, audit logs, file type rules, and adapter graph metadata. Object storage contains immutable chunks and blob manifests.

## Backup

```sh
docker compose -f deploy/docker-compose.yml --env-file deploy/.env exec postgres \
  pg_dump -U openasset openasset > openasset.sql

docker run --rm \
  -v openasset-depot_openasset_objects:/objects:ro \
  -v "$PWD":/backup \
  alpine tar czf /backup/openasset-objects.tgz -C / objects
```

## Restore

Stop the API before restore so no writes occur while metadata and objects are being replaced.

```sh
docker compose -f deploy/docker-compose.yml --env-file deploy/.env stop openasset-api
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d postgres

docker compose -f deploy/docker-compose.yml --env-file deploy/.env exec -T postgres \
  psql -U openasset openasset < openasset.sql

docker run --rm \
  -v openasset-depot_openasset_objects:/objects \
  -v "$PWD":/backup \
  alpine sh -c "cd / && tar xzf /backup/openasset-objects.tgz"

docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d openasset-api
```

## Integrity Checks

- Blob and chunk names are BLAKE3 hashes.
- Blob manifests list ordered chunk hashes and sizes.
- File revisions point to immutable blob hashes in PostgreSQL.
- `GET /api/admin/storage/verify?limit=1000` verifies recent blob manifests,
  ordered chunks, byte counts, chunk hashes, and full BLAKE3 blob hashes.
- Run verification after backup restore and on a scheduled sample. Increase
  coverage in batches rather than placing all terabytes in one request.
- Keep database and object snapshots from the same write quiescence window. A
  database revision without its immutable object is not recoverable from metadata alone.
