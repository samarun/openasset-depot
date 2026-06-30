# Deployment

The supported Compose topology is one Rust API writer, PostgreSQL 16, one durable
local object volume, and the web companion behind nginx. The native desktop and
DCC integrations connect to the API directly; they are not containers.

## Ubuntu production host

### Existing host Nginx

Use this topology when Nginx is already installed on the Ubuntu server. Docker
builds two application images:

- `openasset-depot-api:latest`: the Rust API.
- `openasset-depot-web:latest`: the React web build plus its internal API gateway.

PostgreSQL and the API have no host ports. The web gateway is bound only to
`127.0.0.1:5173`, so it can be reached by host Nginx but not directly from the
Internet. Set `OAD_WEB_BIND=0.0.0.0` only when direct LAN access is required,
and keep port 5173 blocked from the Internet.

On the Ubuntu server:

```sh
git clone https://github.com/samarun/openasset-depot.git
cd openasset-depot
./deploy/ubuntu-nginx-up.sh depot.example.com
```

The script generates `deploy/.env.ubuntu-nginx`, builds the Linux images for the
server's native architecture, starts PostgreSQL/API/web, and prints container
status. Install the supplied host Nginx site after replacing the example domain:

```sh
sudo cp deploy/nginx/openasset-depot.conf /etc/nginx/sites-available/openasset-depot
sudo sed -i 's/depot\.example\.com/your-domain.example/g' \
  /etc/nginx/sites-available/openasset-depot
sudo ln -sfn /etc/nginx/sites-available/openasset-depot \
  /etc/nginx/sites-enabled/openasset-depot.conf
sudo nginx -t
sudo systemctl reload nginx
```

Confirm that `/etc/nginx/nginx.conf` includes `/etc/nginx/sites-enabled/*.conf`,
then confirm the server block is active with
`sudo nginx -T | grep -n your-domain.example`.

Enable HTTPS with the certificate workflow already used on the server. With
Certbot's Nginx integration, that is typically:

```sh
sudo certbot --nginx -d your-domain.example
```

Verify the complete route through host Nginx:

```sh
curl -fsS https://your-domain.example/ready
```

The supplied Nginx configuration allows 20 GiB request bodies, disables request
buffering for streamed asset uploads, and uses 30-minute API timeouts. Keep those
limits aligned with `OAD_MAX_UPLOAD_BYTES` and `OAD_REQUEST_TIMEOUT_SECONDS`.

For an Intel/AMD server that should not build or pull anything, transfer the
entire `ubuntu-amd64-bundle/` directory by FTP. It contains compressed
`linux/amd64` archives for the API, web gateway, and PostgreSQL plus an
image-only Compose file. On the server, run:

```sh
cd ubuntu-amd64-bundle
chmod +x install.sh
./install.sh depot.example.com
```

For upgrades:

```sh
git pull --ff-only
./deploy/ubuntu-nginx-up.sh
```

Useful commands:

```sh
docker image ls 'openasset-depot-*'
docker compose -f deploy/docker-compose.ubuntu-nginx.yml \
  --env-file deploy/.env.ubuntu-nginx ps
docker compose -f deploy/docker-compose.ubuntu-nginx.yml \
  --env-file deploy/.env.ubuntu-nginx logs -f --tail=200
```

### Caddy-managed HTTPS

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
- `OAD_ALLOW_SIGNUPS`: permits public self-registration when `true`; new users
  still need an administrator-granted depot role. Disable it after onboarding.

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
