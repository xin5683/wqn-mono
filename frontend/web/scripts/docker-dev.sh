#!/usr/bin/env bash
# Local-debug helper for the WQN frontend image.
#
# Stops & removes the old frontend container, deletes the old frontend image,
# rebuilds it, and runs it on the shared `wqn-net` network — a clean-rebuild loop
# for iterating on the Next.js standalone image. The configuration mirrors
# docker-compose.yml and the currently running `wqn-frontend` container (network
# `wqn-net`, host port ${FRONTEND_PORT}→3000, runtime envs WQN_API_BASE_URL /
# SITE_URL).
#
# Unlike `docker compose up --build`, this script removes the previous image so
# repeated debug rebuilds don't accumulate dangling images. The frontend is
# stateless (no volumes), so nothing is kept across runs.
#
# The frontend proxies /api/* to WQN_API_BASE_URL (default http://backend:8080),
# i.e. it expects the backend container — started with
# ../../wrong_question_notebook_rs/scripts/docker-dev.sh — to be running on
# wqn-net with the `backend` alias. If it isn't, the frontend still starts but
# API calls fail; this script warns in that case.
#
# Usage:
#   scripts/docker-dev.sh             clean rebuild + run (alias of `up`)
#   scripts/docker-dev.sh up         stop+rm container, rm image, build, run
#   scripts/docker-dev.sh run        recreate the container from the current image (no rebuild)
#   scripts/docker-dev.sh logs       tail frontend logs
#   scripts/docker-dev.sh status     show status of frontend + backend + image
#   scripts/docker-dev.sh stop       stop + remove the frontend container (keep image)
#   scripts/docker-dev.sh restart    bounce the frontend container
#   scripts/docker-dev.sh clean      stop + rm frontend container and image
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ENV_FILE:-${ROOT_DIR}/.env}"
BACKEND_DIR="${BACKEND_DIR:-$(cd "${ROOT_DIR}/../.." && pwd)/wrong_question_notebook_rs}"

# ---- names (mirror docker-compose.yml / running containers) ----------------
IMAGE="${WQN_FRONTEND_IMAGE:-wqn-frontend:latest}"
CONTAINER="${WQN_FRONTEND_CONTAINER:-wqn-frontend}"
NETWORK="${WQN_NET:-wqn-net}"
BACKEND_CONTAINER="${WQN_BACKEND_CONTAINER:-wqn-backend}"

usage() {
  cat <<USAGE
Usage: scripts/docker-dev.sh <command>

Clean-rebuild loop for iterating on the WQN frontend image. Configuration
mirrors docker-compose.yml and the running \`wqn-frontend\` container.

Commands:
  (default) / up    stop+rm frontend container, rm frontend image, rebuild, run
  run               recreate the frontend container from the current image (no rebuild)
  logs              tail frontend logs
  status            show status of frontend container + image (+ backend reachability)
  stop              stop + remove the frontend container (keep image)
  restart           bounce the frontend container
  clean             stop + rm frontend container and image

Environment overrides:
  WQN_FRONTEND_IMAGE     default: ${IMAGE}
  WQN_FRONTEND_CONTAINER default: ${CONTAINER}
  WQN_NET                default: ${NETWORK}
  WQN_BACKEND_CONTAINER  default: ${BACKEND_CONTAINER}
  FRONTEND_PORT          host port → 3000 (default: 3000)
  WQN_API_BASE_URL       backend URL baked at build + set at runtime
                         (default: http://backend:8080)
  SITE_URL               canonical site URL baked at build + set at runtime
                         (default: http://localhost:3000)
  NPM_REGISTRY           npm registry build arg; set to empty for the official
                         registry (default: https://registry.npmmirror.com/)
  ENV_FILE               default: ${ENV_FILE}

The frontend is stateless — nothing is kept across runs. It expects the backend
container (../../wrong_question_notebook_rs/scripts/docker-dev.sh) to be running
on wqn-net so http://backend:8080 resolves.
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

# ---- load .env (optional for the frontend) ---------------------------------
load_env() {
  if [[ -f "${ENV_FILE}" ]]; then
    set -a
    # shellcheck disable=SC1090
    . "${ENV_FILE}"
    set +a
  fi

  API_BASE_URL="${WQN_API_BASE_URL:-http://backend:8080}"
  SITE_URL="${SITE_URL:-http://localhost:3000}"
  FRONTEND_PORT="${FRONTEND_PORT:-3000}"
  # `-` (not `:-`) so an explicitly-empty NPM_REGISTRY disables the mirror.
  NPM_REGISTRY="${NPM_REGISTRY-https://registry.npmmirror.com/}"

  # Loopback addresses resolve to the container itself, not the backend — a
  # common footgun when the dev-oriented env.example leaks into the container.
  if [[ "${API_BASE_URL}" =~ ^https?://(localhost|127\.0\.0\.1)(:|$) ]]; then
    echo "WARNING: WQN_API_BASE_URL='${API_BASE_URL}' points at the host loopback." >&2
    echo "  Inside the container that is the container itself, not the backend." >&2
    echo "  For the wqn-net setup use WQN_API_BASE_URL=http://backend:8080" >&2
  fi
}

# ---- network + backend dependency ------------------------------------------
ensure_network() {
  if network_exists; then
    return 0
  fi
  echo "creating network ${NETWORK}"
  docker network create "${NETWORK}" >/dev/null
}

warn_if_backend_down() {
  if container_running "${BACKEND_CONTAINER}"; then
    return 0
  fi
  echo "NOTE: backend container '${BACKEND_CONTAINER}' is not running." >&2
  echo "  The frontend will start, but /api calls will fail (proxied to ${API_BASE_URL})." >&2
  echo "  Start it with: (cd ${BACKEND_DIR} && scripts/docker-dev.sh up)" >&2
}

# ---- frontend image / container lifecycle -----------------------------------
remove_frontend_container() {
  if container_exists "${CONTAINER}"; then
    echo "removing old container ${CONTAINER}"
    docker rm -f "${CONTAINER}" >/dev/null
  fi
}

remove_frontend_image() {
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
  docker build \
    --build-arg "NPM_REGISTRY=${NPM_REGISTRY}" \
    --build-arg "WQN_API_BASE_URL=${API_BASE_URL}" \
    --build-arg "SITE_URL=${SITE_URL}" \
    -t "${IMAGE}" \
    "${ROOT_DIR}"
}

run_frontend() {
  echo "running ${CONTAINER} from ${IMAGE}"
  docker run -d \
    --name "${CONTAINER}" \
    --network "${NETWORK}" \
    --network-alias wqn-frontend \
    --network-alias frontend \
    --restart unless-stopped \
    -p "${FRONTEND_PORT}:3000" \
    -e NODE_ENV=production \
    -e WQN_API_BASE_URL="${API_BASE_URL}" \
    -e SITE_URL="${SITE_URL}" \
    -e PORT=3000 \
    -e HOSTNAME=0.0.0.0 \
    "${IMAGE}" >/dev/null
}

wait_frontend() {
  echo "waiting for frontend on :${FRONTEND_PORT} ..."
  if ! command -v curl >/dev/null 2>&1; then
    echo "curl not found; sleeping 3s — follow logs: scripts/docker-dev.sh logs" >&2
    sleep 3
    return 0
  fi
  for _ in $(seq 1 60); do
    # robots.txt is a top-level static route (no locale redirect), so it's a
    # reliable readiness probe once the Next.js server is accepting traffic.
    if curl -sf -o /dev/null "http://127.0.0.1:${FRONTEND_PORT}/robots.txt" 2>/dev/null; then
      echo "frontend ready (http://127.0.0.1:${FRONTEND_PORT})"
      return 0
    fi
    sleep 1
  done
  echo "frontend did not become ready in time — recent logs:" >&2
  docker logs --tail 50 "${CONTAINER}" >&2 || true
  exit 1
}

print_urls() {
  echo
  echo "  App:      http://127.0.0.1:${FRONTEND_PORT}"
  echo "  Backend:  ${API_BASE_URL}"
  echo "  Logs:     scripts/docker-dev.sh logs"
}

# ---- subcommands -----------------------------------------------------------
cmd_up() {
  require_docker
  load_env
  ensure_network
  warn_if_backend_down
  remove_frontend_container
  remove_frontend_image
  build_image
  run_frontend
  wait_frontend
  print_urls
}

cmd_run() {
  require_docker
  load_env
  ensure_network
  warn_if_backend_down
  if ! docker image inspect "${IMAGE}" >/dev/null 2>&1; then
    echo "image ${IMAGE} not found — run \`scripts/docker-dev.sh up\` first" >&2
    exit 1
  fi
  remove_frontend_container
  run_frontend
  wait_frontend
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
  if container_running "${CONTAINER}"; then
    docker ps --filter "name=^/${CONTAINER}$" --format '{{.Names}}\t{{.Status}}\t{{.Ports}}'
  elif container_exists "${CONTAINER}"; then
    docker ps -a --filter "name=^/${CONTAINER}$" --format '{{.Names}}\t{{.Status}}'
    any=1
  else
    echo "${CONTAINER}: not created"
    any=1
  fi
  if docker image inspect "${IMAGE}" >/dev/null 2>&1; then
    docker images "${IMAGE}" --format 'image: {{.Repository}}:{{.Tag}}\t{{.ID}}\t{{.Size}}'
  else
    echo "image ${IMAGE}: not built"
    any=1
  fi
  if container_running "${BACKEND_CONTAINER}"; then
    echo "backend: ${BACKEND_CONTAINER} running (http://backend:8080 reachable)"
  else
    echo "backend: ${BACKEND_CONTAINER} not running — /api calls will fail"
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
  remove_frontend_container
  remove_frontend_image
  echo "frontend container + image removed (frontend is stateless — no volumes to clean)"
}

case "${1:-up}" in
  up) cmd_up ;;
  run) cmd_run ;;
  logs) cmd_logs ;;
  status) cmd_status ;;
  stop) cmd_stop ;;
  restart) cmd_restart ;;
  clean) cmd_clean ;;
  -h | --help | help) usage ;;
  *)
    usage >&2
    exit 2
    ;;
esac
