#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ROOT_DIR}/deploy/.env.ubuntu-nginx"
COMPOSE_FILE="${ROOT_DIR}/deploy/docker-compose.ubuntu-nginx.yml"
DOMAIN="${1:-}"

if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
  echo "Docker Engine with the Compose plugin is required." >&2
  exit 1
fi

if [[ ! -f "${ENV_FILE}" ]]; then
  if [[ -z "${DOMAIN}" ]]; then
    echo "Usage: ./deploy/ubuntu-nginx-up.sh depot.example.com" >&2
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
    printf 'OAD_API_IMAGE=openasset-depot-api:latest\n'
    printf 'OAD_WEB_IMAGE=openasset-depot-web:latest\n'
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
  echo "Created ${ENV_FILE} with mode 600. Keep it out of Git and in your secret backup."
fi

cd "${ROOT_DIR}"
docker compose -f "${COMPOSE_FILE}" --env-file "${ENV_FILE}" config --quiet
docker compose -f "${COMPOSE_FILE}" --env-file "${ENV_FILE}" up -d --build
docker compose -f "${COMPOSE_FILE}" --env-file "${ENV_FILE}" ps

WEB_PORT="$(sed -n 's/^OAD_WEB_PORT=//p' "${ENV_FILE}" | head -n 1)"
WEB_PORT="${WEB_PORT:-5173}"
WEB_BIND="$(sed -n 's/^OAD_WEB_BIND=//p' "${ENV_FILE}" | head -n 1)"
WEB_BIND="${WEB_BIND:-127.0.0.1}"
echo
echo "Docker is serving OpenAsset Depot at http://${WEB_BIND}:${WEB_PORT}"
echo "Configure host Nginx with deploy/nginx/openasset-depot.conf, then enable HTTPS."
