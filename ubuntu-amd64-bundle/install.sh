#!/usr/bin/env bash
set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${BUNDLE_DIR}/.env"
DOMAIN="${1:-}"

if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
  echo "Docker Engine with the Compose plugin is required." >&2
  exit 1
fi
if ! command -v sha256sum >/dev/null 2>&1; then
  echo "sha256sum is required to verify the image archives." >&2
  exit 1
fi

cd "${BUNDLE_DIR}/images"
sha256sum -c SHA256SUMS
for archive in ./*.tar.gz; do
  echo "Loading ${archive#./}"
  gzip -dc "${archive}" | docker load
done

if [[ ! -f "${ENV_FILE}" ]]; then
  if [[ -z "${DOMAIN}" ]]; then
    echo "Usage: ./install.sh depot.example.com" >&2
    exit 1
  fi
  if ! command -v openssl >/dev/null 2>&1; then
    echo "OpenSSL is required to generate deployment secrets." >&2
    exit 1
  fi
  umask 077
  POSTGRES_SECRET="$(openssl rand -hex 32)"
  JWT_SECRET="$(openssl rand -base64 48 | tr -d '\n')"
  {
    printf 'OAD_DOMAIN=%s\n' "${DOMAIN}"
    printf 'OAD_WEB_BIND=127.0.0.1\n'
    printf 'OAD_WEB_PORT=5173\n'
    printf 'OAD_JWT_SECRET=%s\n' "${JWT_SECRET}"
    printf 'OAD_JWT_TTL_SECONDS=86400\n'
    printf 'OAD_DATABASE_MAX_CONNECTIONS=20\n'
    printf 'OAD_MAX_UPLOAD_BYTES=21474836480\n'
    printf 'OAD_CHUNK_SIZE_BYTES=4194304\n'
    printf 'OAD_REQUEST_TIMEOUT_SECONDS=1800\n'
    printf 'OAD_RUN_MIGRATIONS=true\n'
    printf 'OAD_ALLOW_SIGNUPS=true\n'
    printf 'POSTGRES_DB=openasset\n'
    printf 'POSTGRES_USER=openasset\n'
    printf 'POSTGRES_PASSWORD=%s\n' "${POSTGRES_SECRET}"
    printf 'RUST_LOG=openasset_api=info,tower_http=info\n'
  } > "${ENV_FILE}"
  echo "Created ${ENV_FILE} with mode 600. Keep it in your secret backup."
fi

cd "${BUNDLE_DIR}"
docker compose --env-file .env config --quiet
docker compose --env-file .env up -d --pull never
docker compose --env-file .env ps

WEB_PORT="$(sed -n 's/^OAD_WEB_PORT=//p' .env | head -n 1)"
WEB_PORT="${WEB_PORT:-5173}"
WEB_BIND="$(sed -n 's/^OAD_WEB_BIND=//p' .env | head -n 1)"
WEB_BIND="${WEB_BIND:-127.0.0.1}"
echo
echo "OpenAsset Depot is published at http://${WEB_BIND}:${WEB_PORT}"
echo "Install nginx.conf after replacing depot.example.com, then enable HTTPS."
