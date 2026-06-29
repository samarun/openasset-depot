#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ROOT_DIR}/deploy/.env.production"
COMPOSE_FILE="${ROOT_DIR}/deploy/docker-compose.production.yml"
DOMAIN="${1:-}"

if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
  echo "Docker Engine with the Compose plugin is required." >&2
  exit 1
fi

if [[ ! -f "${ENV_FILE}" ]]; then
  if [[ -z "${DOMAIN}" ]]; then
    echo "Usage: ./deploy/ubuntu-up.sh depot.example.com" >&2
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
    printf 'OAD_JWT_SECRET=%s\n' "${JWT_SECRET}"
    printf 'OAD_JWT_TTL_SECONDS=86400\n'
    printf 'OAD_DATABASE_MAX_CONNECTIONS=20\n'
    printf 'OAD_MAX_UPLOAD_BYTES=21474836480\n'
    printf 'OAD_CHUNK_SIZE_BYTES=4194304\n'
    printf 'OAD_REQUEST_TIMEOUT_SECONDS=1800\n'
    printf 'OAD_RUN_MIGRATIONS=true\n'
    printf 'POSTGRES_DB=openasset\n'
    printf 'POSTGRES_USER=openasset\n'
    printf 'POSTGRES_PASSWORD=%s\n' "${POSTGRES_SECRET}"
    printf 'RUST_LOG=openasset_api=info,tower_http=info\n'
  } > "${ENV_FILE}"
  echo "Created ${ENV_FILE} with mode 600. Keep it off Git and in your secret backup."
fi

cd "${ROOT_DIR}"
docker compose -f "${COMPOSE_FILE}" --env-file "${ENV_FILE}" config --quiet
docker compose -f "${COMPOSE_FILE}" --env-file "${ENV_FILE}" up -d --build
docker compose -f "${COMPOSE_FILE}" --env-file "${ENV_FILE}" ps

DEPLOYED_DOMAIN="$(sed -n 's/^OAD_DOMAIN=//p' "${ENV_FILE}" | head -n 1)"
echo
echo "OpenAsset Depot is starting at https://${DEPLOYED_DOMAIN}"
echo "After /ready responds, bootstrap the first admin using the command in docs/deployment.md."
