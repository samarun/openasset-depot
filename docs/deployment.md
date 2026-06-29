# Deployment

The supported Compose topology is one Rust API writer, PostgreSQL 16, one durable
local object volume, and the web companion behind nginx. The native desktop and
DCC integrations connect to the API directly; they are not containers.

## Ubuntu production host

The production topology adds Caddy as the only public container. PostgreSQL and
the Rust API stay on an internal Docker network, while Caddy provisions and
renews HTTPS certificates automatically.

Before deploying:

1. Point an `A`/`AAAA` DNS record such as `depot.example.com` at the Ubuntu server.
2. Allow inbound TCP 22, 80, and 443 and UDP 443 in the cloud firewall. Do not
   expose PostgreSQL or port 8080.
3. Install Docker Engine and the Docker Compose plugin, then clone this repository.
4. From the repository root, run:

```sh
./deploy/ubuntu-up.sh depot.example.com
```

The script creates `deploy/.env.production` with random database and JWT
secrets (mode 600), validates the Compose model, builds the images, and starts
the stack. Keep that file out of Git and include it in the studio secret backup.

Check startup and bootstrap the first administrator:

```sh
curl -fsS https://depot.example.com/ready
curl -fsS https://depot.example.com/api/users \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"use-a-long-studio-password","display_name":"Depot Admin"}'
```

The unauthenticated bootstrap works only while the user table is empty. For
upgrades, pull the intended revision and rerun the same command; Compose rebuilds
changed images while preserving named volumes:

```sh
git pull --ff-only
./deploy/ubuntu-up.sh
```

Operational commands:

```sh
docker compose -f deploy/docker-compose.production.yml --env-file deploy/.env.production ps
docker compose -f deploy/docker-compose.production.yml --env-file deploy/.env.production logs -f --tail=200
docker compose -f deploy/docker-compose.production.yml --env-file deploy/.env.production restart openasset-api
```

Back up both `openasset-depot_openasset_postgres` and
`openasset-depot_openasset_objects`. A database-only backup is not a complete
depot backup; see [backup and restore](backup-restore.md).

## Configure

```sh
cp deploy/.env.example deploy/.env
openssl rand -base64 48
```

Replace `POSTGRES_PASSWORD`, `OAD_DATABASE_URL`, and `OAD_JWT_SECRET`. The API
rejects short/example JWT secrets. Configure:

- `OAD_CORS_ALLOWED_ORIGINS`: comma-separated exact web/Tauri origins; wildcard
  origins are rejected.
- `OAD_MAX_UPLOAD_BYTES`: cumulative request limit.
- `OAD_CHUNK_SIZE_BYTES`: 64 KiB to 64 MiB; default 4 MiB.
- `OAD_REQUEST_TIMEOUT_SECONDS`: default 1,800 in Compose for large binary submits.
- `OAD_DATABASE_MAX_CONNECTIONS`: bounded PostgreSQL pool size.
- `OAD_JWT_TTL_SECONDS`: 60 seconds to 7 days.
- `OAD_RUN_MIGRATIONS`: set `false` when migrations are managed by deployment tooling.

Use HTTPS at a reverse proxy/load balancer outside localhost. Forward request IDs
and retain structured JSON logs. Add external IP-aware authentication throttling;
the application intentionally does not trust arbitrary forwarded client IPs.

## Start

```sh
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d --build
curl -fsS http://127.0.0.1:8080/health
curl -fsS http://127.0.0.1:8080/ready
```

Default ports are web `5173`, API `8080`, and PostgreSQL `5432`. MinIO and Redis
are explicitly future profiles and do not run in the supported local-storage
topology.

## Runtime behavior

- Migrations are versioned under `server/api/migrations`.
- API logs are structured JSON and include propagated/generated `x-request-id`.
- `/health` reports process liveness; `/ready` verifies PostgreSQL.
- Uploads/downloads are streamed with bounded buffers. Server temporaries older
  than 24 hours are cleaned during startup.
- SIGTERM/Ctrl-C initiates graceful Axum shutdown.
- Run only one API writer against a local object volume. Multi-node deployments
  require the future S3-compatible backend.

## Desktop distribution

`./scripts/build-desktop-release.sh` builds the CLI sidecar, runs UI tests, and
creates platform bundles. Local macOS builds receive a valid ad-hoc signature
and are verified before the script returns. For external distribution, set
`OAD_MACOS_SIGNING_IDENTITY` to the studio's Developer ID Application identity
and provide Tauri's Apple notarization credentials (`APPLE_ID`,
`APPLE_PASSWORD`, and `APPLE_TEAM_ID`, or the App Store Connect API-key
equivalents). Do not distribute an ad-hoc build outside managed studio machines.

The desktop CSP allows API connections but blocks objects, frames, remote
scripts, and arbitrary base URLs.
