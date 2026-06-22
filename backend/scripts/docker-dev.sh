#!/usr/bin/env bash
# Local-debug helper for the WQN backend image.
#
# Stops & removes the old backend container, deletes the old backend image,
# rebuilds it, and runs it against a local Postgres — a clean-rebuild loop for
# iterating on Rust code. The configuration mirrors docker-compose.yml and the
# currently running `wqn-backend` container (network `wqn-net`, storage volume,
# DATABASE_URL host `postgres`, host port ${BACKEND_PORT}).
#
# Unlike `docker compose up --build`, this script removes the previous image so
# repeated debug rebuilds don't accumulate dangling images. Postgres and its data
# volume are intentionally KEPT across runs; use `clean -v` or
# `docker compose down -v` to wipe the database.
#
# Usage:
#   scripts/docker-dev.sh             clean rebuild + run (alias of `up`)
#   scripts/docker-dev.sh up         stop+rm container, rm image, build, run
#   scripts/docker-dev.sh run        recreate the container from the current image (no rebuild)
#   scripts/docker-dev.sh logs       tail backend logs
#   scripts/docker-dev.sh status     show status of postgres + backend
#   scripts/docker-dev.sh stop       stop + remove the backend container (keep image/pg)
#   scripts/docker-dev.sh restart    bounce the backend container
#   scripts/docker-dev.sh clean      stop + rm backend container and image
#                                    (`clean -v` also drops postgres + both volumes incl. DB data)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ENV_FILE:-${ROOT_DIR}/.env}"

# ---- names (mirror docker-compose.yml / running containers) ----------------
IMAGE="${WQN_BACKEND_IMAGE:-wqn-backend:latest}"
CONTAINER="${WQN_BACKEND_CONTAINER:-wqn-backend}"
NETWORK="${WQN_NET:-wqn-net}"
PG_CONTAINER="${WQN_POSTGRES_CONTAINER:-wqn-postgres}"
PG_IMAGE="${WQN_POSTGRES_IMAGE:-postgres:16-alpine}"
PG_VOLUME="${WQN_POSTGRES_VOLUME:-wrong_question_notebook_rs_pg_data}"
STORAGE_VOLUME="${WQN_BACKEND_STORAGE_VOLUME:-wrong_question_notebook_rs_backend_storage}"

usage() {
  cat <<USAGE
Usage: scripts/docker-dev.sh <command>

Clean-rebuild loop for iterating on the WQN backend image. Configuration
mirrors docker-compose.yml and the running \`wqn-backend\` container.

Commands:
  (default) / up    stop+rm backend container, rm backend image, rebuild, run
  run               recreate the backend container from the current image (no rebuild)
  logs              tail backend logs
  status            show status of postgres + backend + image
  stop              stop + remove the backend container (keep image & postgres)
  restart           bounce the backend container
  clean             stop + rm backend container and image
                    (\`clean -v\` also drops postgres + both volumes incl. DB data)

Environment overrides:
  WQN_BACKEND_IMAGE            default: ${IMAGE}
  WQN_BACKEND_CONTAINER       default: ${CONTAINER}
  WQN_NET                     default: ${NETWORK}
  WQN_POSTGRES_CONTAINER      default: ${PG_CONTAINER}
  WQN_POSTGRES_IMAGE          default: ${PG_IMAGE}
  WQN_POSTGRES_VOLUME         default: ${PG_VOLUME}
  WQN_BACKEND_STORAGE_VOLUME  default: ${STORAGE_VOLUME}
  ENV_FILE                    default: ${ENV_FILE}
  plus all .env vars (BACKEND_PORT, AUTH_JWT_SECRET, ...)

Postgres and its data volume are KEPT across \`up\` runs.
USAGE
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required" >&2
    exit 1
  fi
}

container_exists() {
  docker ps -a --format '{{.Names}}' | grep -Fxq "$1"
}

container_running() {
  docker ps --format '{{.Names}}' | grep -Fxq "$1"
}

network_exists() {
  docker network inspect "${NETWORK}" >/dev/null 2>&1
}

# ---- load .env -------------------------------------------------------------
load_env() {
  if [[ ! -f "${ENV_FILE}" ]]; then
    echo "missing ${ENV_FILE} — start with: cp .env.example .env" >&2
    exit 1
  fi
  set -a
  # shellcheck disable=SC1090
  . "${ENV_FILE}"
  set +a

  : "${POSTGRES_USER:?POSTGRES_USER must be set in ${ENV_FILE}}"
  : "${POSTGRES_PASSWORD:?POSTGRES_PASSWORD must be set in ${ENV_FILE}}"
  : "${POSTGRES_DB:?POSTGRES_DB must be set in ${ENV_FILE}}"
  : "${AUTH_JWT_SECRET:?AUTH_JWT_SECRET must be set in ${ENV_FILE} — generate with: openssl rand -base64 48}"
  if [[ "${AUTH_JWT_SECRET}" == "change-me-please-run-openssl-rand-base64-48" ]]; then
    echo "AUTH_JWT_SECRET is still the placeholder — run: openssl rand -base64 48" >&2
    exit 1
  fi

  BACKEND_PORT="${BACKEND_PORT:-8080}"
  POSTGRES_PORT="${POSTGRES_PORT:-5432}"
  # Hostname `postgres` resolves on wqn-net: compose registers it as the service
  # alias; if we create Postgres ourselves, ensure_postgres adds the alias too.
  DATABASE_URL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}"
  CORS_ALLOWED_ORIGINS="${CORS_ALLOWED_ORIGINS:-http://localhost:3000,http://127.0.0.1:3000}"
  AUTH_COOKIE_SECURE="${AUTH_COOKIE_SECURE:-false}"
  AUTH_SESSION_TTL_SECONDS="${AUTH_SESSION_TTL_SECONDS:-2592000}"
  AUTH_COOKIE_NAME="${AUTH_COOKIE_NAME:-wqn_session}"
  DATABASE_MAX_CONNECTIONS="${DATABASE_MAX_CONNECTIONS:-10}"
}

# ---- ensure dependencies (network + postgres) ------------------------------
ensure_network() {
  if network_exists; then
    return 0
  fi
  echo "creating network ${NETWORK}"
  docker network create "${NETWORK}" >/dev/null
}

ensure_postgres() {
  if container_running "${PG_CONTAINER}"; then
    echo "${PG_CONTAINER} already running"
    return 0
  fi
  if container_exists "${PG_CONTAINER}"; then
    echo "starting ${PG_CONTAINER}"
    docker start "${PG_CONTAINER}" >/dev/null
  else
    echo "creating ${PG_CONTAINER} (image ${PG_IMAGE}, volume ${PG_VOLUME})"
    docker volume create "${PG_VOLUME}" >/dev/null
    docker run -d \
      --name "${PG_CONTAINER}" \
      --network "${NETWORK}" \
      --network-alias postgres \
      --restart unless-stopped \
      -e POSTGRES_USER="${POSTGRES_USER}" \
      -e POSTGRES_PASSWORD="${POSTGRES_PASSWORD}" \
      -e POSTGRES_DB="${POSTGRES_DB}" \
      -p "${POSTGRES_PORT}:5432" \
      -v "${PG_VOLUME}:/var/lib/postgresql/data" \
      "${PG_IMAGE}" >/dev/null
  fi

  echo "waiting for ${PG_CONTAINER} ..."
  for _ in $(seq 1 60); do
    if docker exec "${PG_CONTAINER}" pg_isready -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "Postgres did not become ready" >&2
  docker logs "${PG_CONTAINER}" >&2 || true
  exit 1
}

# ---- backend image / container lifecycle -----------------------------------
remove_backend_container() {
  if container_exists "${CONTAINER}"; then
    echo "removing old container ${CONTAINER}"
    docker rm -f "${CONTAINER}" >/dev/null
  fi
}

remove_backend_image() {
  local old_id
  old_id="$(docker images -q "${IMAGE}" 2>/dev/null | head -n1 || true)"
  if [[ -z "${old_id}" ]]; then
    return 0
  fi
  echo "removing old image ${IMAGE} (${old_id:0:12})"
  # Untag first, then drop the now-unreferenced image. Scoped to our image only
  # (does NOT prune other projects' dangling images).
  docker rmi -f "${IMAGE}" >/dev/null 2>&1 || true
  docker rmi "${old_id}" >/dev/null 2>&1 || true
}

build_image() {
  echo "building ${IMAGE}"
  docker build -t "${IMAGE}" "${ROOT_DIR}"
}

run_backend() {
  echo "running ${CONTAINER} from ${IMAGE}"
  docker run -d \
    --name "${CONTAINER}" \
    --network "${NETWORK}" \
    --network-alias wqn-backend \
    --network-alias backend \
    --restart unless-stopped \
    -p "${BACKEND_PORT}:8080" \
    -v "${STORAGE_VOLUME}:/app/storage" \
    -e DATABASE_URL="${DATABASE_URL}" \
    -e APP_BIND_ADDR=0.0.0.0:8080 \
    -e AUTH_JWT_SECRET="${AUTH_JWT_SECRET}" \
    -e AUTH_COOKIE_NAME="${AUTH_COOKIE_NAME}" \
    -e AUTH_COOKIE_SECURE="${AUTH_COOKIE_SECURE}" \
    -e AUTH_SESSION_TTL_SECONDS="${AUTH_SESSION_TTL_SECONDS}" \
    -e CORS_ALLOWED_ORIGINS="${CORS_ALLOWED_ORIGINS}" \
    -e DATABASE_MAX_CONNECTIONS="${DATABASE_MAX_CONNECTIONS}" \
    -e LOCAL_STORAGE_ROOT=/app/storage \
    "${IMAGE}" >/dev/null
}

wait_backend() {
  echo "waiting for backend on :${BACKEND_PORT}/healthz ..."
  if ! command -v curl >/dev/null 2>&1; then
    echo "curl not found; sleeping 3s — follow logs: scripts/docker-dev.sh logs" >&2
    sleep 3
    return 0
  fi
  for _ in $(seq 1 60); do
    if curl -sf "http://127.0.0.1:${BACKEND_PORT}/healthz" >/dev/null 2>&1; then
      echo "backend ready (http://127.0.0.1:${BACKEND_PORT})"
      return 0
    fi
    sleep 1
  done
  echo "backend did not become ready in time — recent logs:" >&2
  docker logs --tail 50 "${CONTAINER}" >&2 || true
  exit 1
}

print_urls() {
  echo
  echo "  API:      http://127.0.0.1:${BACKEND_PORT}/api"
  echo "  Swagger:  http://127.0.0.1:${BACKEND_PORT}/swagger-ui"
  echo "  OpenAPI:  http://127.0.0.1:${BACKEND_PORT}/api-docs/openapi.json"
  echo "  Health:   http://127.0.0.1:${BACKEND_PORT}/healthz"
  echo "  Logs:     scripts/docker-dev.sh logs"
}

# ---- subcommands -----------------------------------------------------------
cmd_up() {
  require_docker
  load_env
  ensure_network
  ensure_postgres
  remove_backend_container
  remove_backend_image
  build_image
  run_backend
  wait_backend
  print_urls
}

cmd_run() {
  require_docker
  load_env
  ensure_network
  ensure_postgres
  if ! docker image inspect "${IMAGE}" >/dev/null 2>&1; then
    echo "image ${IMAGE} not found — run \`scripts/docker-dev.sh up\` first" >&2
    exit 1
  fi
  remove_backend_container
  run_backend
  wait_backend
  print_urls
}

cmd_logs() {
  require_docker
  if ! container_exists "${CONTAINER}"; then
    echo "${CONTAINER} does not exist" >&2
    exit 1
  fi
  docker logs -f --tail 100 "${CONTAINER}"
}

cmd_status() {
  require_docker
  local any=0
  for c in "${PG_CONTAINER}" "${CONTAINER}"; do
    if container_running "${c}"; then
      docker ps --filter "name=^/${c}$" --format '{{.Names}}\t{{.Status}}\t{{.Ports}}'
    elif container_exists "${c}"; then
      docker ps -a --filter "name=^/${c}$" --format '{{.Names}}\t{{.Status}}'
      any=1
    else
      echo "${c}: not created"
      any=1
    fi
  done
  if docker image inspect "${IMAGE}" >/dev/null 2>&1; then
    docker images "${IMAGE}" --format 'image: {{.Repository}}:{{.Tag}}\t{{.ID}}\t{{.Size}}'
  else
    echo "image ${IMAGE}: not built"
    any=1
  fi
  return "${any}"
}

cmd_stop() {
  require_docker
  if container_running "${CONTAINER}"; then
    docker stop "${CONTAINER}" >/dev/null
    echo "${CONTAINER} stopped"
  elif container_exists "${CONTAINER}"; then
    echo "${CONTAINER} is not running"
  else
    echo "${CONTAINER} does not exist"
  fi
}

cmd_restart() {
  require_docker
  if container_running "${CONTAINER}"; then
    docker restart "${CONTAINER}" >/dev/null
    echo "${CONTAINER} restarted"
  elif container_exists "${CONTAINER}"; then
    docker start "${CONTAINER}" >/dev/null
    echo "${CONTAINER} started"
  else
    echo "${CONTAINER} does not exist — run \`scripts/docker-dev.sh up\`" >&2
    exit 1
  fi
}

cmd_clean() {
  require_docker
  remove_backend_container
  remove_backend_image
  echo "backend container + image removed (postgres + volumes kept)"
  if [[ "${1:-}" == "-v" ]]; then
    echo "dropping postgres container + volumes (DB data will be lost)"
    docker rm -f "${PG_CONTAINER}" >/dev/null 2>&1 || true
    docker volume rm "${PG_VOLUME}" "${STORAGE_VOLUME}" >/dev/null 2>&1 || true
  fi
}

case "${1:-up}" in
  up) cmd_up ;;
  run) cmd_run ;;
  logs) cmd_logs ;;
  status) cmd_status ;;
  stop) cmd_stop ;;
  restart) cmd_restart ;;
  clean) shift; cmd_clean "$@" ;;
  -h | --help | help) usage ;;
  *)
    usage >&2
    exit 2
    ;;
esac
