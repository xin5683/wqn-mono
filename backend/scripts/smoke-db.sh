#!/usr/bin/env bash
set -euo pipefail

NAME="${WQN_SMOKE_POSTGRES_CONTAINER:-wqn-postgres-smoke}"
IMAGE="${WQN_SMOKE_POSTGRES_IMAGE:-postgres:18-alpine}"
PORT="${WQN_SMOKE_POSTGRES_PORT:-55432}"
USER="${WQN_SMOKE_POSTGRES_USER:-wqn}"
PASSWORD="${WQN_SMOKE_POSTGRES_PASSWORD:-wqn}"
DATABASES="${WQN_SMOKE_POSTGRES_DATABASES:-wqn wqn_migrate_smoke}"

usage() {
  cat <<USAGE
Usage: scripts/smoke-db.sh <start|status|stop|restart>

Environment overrides:
  WQN_SMOKE_POSTGRES_CONTAINER  default: ${NAME}
  WQN_SMOKE_POSTGRES_IMAGE      default: ${IMAGE}
  WQN_SMOKE_POSTGRES_PORT       default: ${PORT}
  WQN_SMOKE_POSTGRES_USER       default: ${USER}
  WQN_SMOKE_POSTGRES_PASSWORD   default: wqn
  WQN_SMOKE_POSTGRES_DATABASES  default: "wqn wqn_migrate_smoke"
USAGE
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required" >&2
    exit 1
  fi
}

container_exists() {
  docker ps -a --format '{{.Names}}' | grep -Fxq "${NAME}"
}

container_running() {
  docker ps --format '{{.Names}}' | grep -Fxq "${NAME}"
}

wait_ready() {
  for _ in $(seq 1 60); do
    if docker exec "${NAME}" pg_isready -U "${USER}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "Postgres did not become ready" >&2
  docker logs "${NAME}" >&2 || true
  exit 1
}

ensure_databases() {
  for database in ${DATABASES}; do
    docker exec -e PGPASSWORD="${PASSWORD}" "${NAME}" \
      psql -U "${USER}" -d postgres -v ON_ERROR_STOP=1 \
      -tc "select 1 from pg_database where datname = '${database}'" |
      grep -q 1 ||
      docker exec -e PGPASSWORD="${PASSWORD}" "${NAME}" \
        createdb -U "${USER}" "${database}"
  done
}

start() {
  require_docker
  if container_running; then
    echo "${NAME} already running on port ${PORT}"
    ensure_databases
    return 0
  fi

  if container_exists; then
    docker start "${NAME}" >/dev/null
  else
    docker run -d \
      --name "${NAME}" \
      -e POSTGRES_USER="${USER}" \
      -e POSTGRES_PASSWORD="${PASSWORD}" \
      -e POSTGRES_DB="${USER}" \
      -p "${PORT}:5432" \
      "${IMAGE}" >/dev/null
  fi

  wait_ready
  ensure_databases
  echo "${NAME} running on port ${PORT}"
}

status() {
  require_docker
  if container_running; then
    docker ps --filter "name=^/${NAME}$" --format '{{.Names}} {{.Status}} {{.Ports}}'
  elif container_exists; then
    docker ps -a --filter "name=^/${NAME}$" --format '{{.Names}} {{.Status}}'
    exit 1
  else
    echo "${NAME} does not exist"
    exit 1
  fi
}

stop() {
  require_docker
  if container_running; then
    docker stop "${NAME}" >/dev/null
    echo "${NAME} stopped"
  else
    echo "${NAME} is not running"
  fi
}

case "${1:-}" in
  start)
    start
    ;;
  status)
    status
    ;;
  stop)
    stop
    ;;
  restart)
    stop
    start
    ;;
  -h | --help | help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
