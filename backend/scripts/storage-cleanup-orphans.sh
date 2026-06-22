#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATABASE_URL="${DATABASE_URL:-postgres://wqn:wqn@127.0.0.1:55432/wqn}"
LOCAL_STORAGE_ROOT="${LOCAL_STORAGE_ROOT:-${ROOT_DIR}/storage}"
POSTGRES_CONTAINER="${WQN_SMOKE_POSTGRES_CONTAINER:-wqn-postgres-smoke}"
APPLY="${APPLY:-0}"
WORK_DIR="$(mktemp -d /tmp/wqn-storage-cleanup.XXXXXX)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

run_copy_query() {
  local sql="$1"

  if command -v psql >/dev/null 2>&1; then
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -At -c "${sql}"
    return 0
  fi

  if command -v docker >/dev/null 2>&1 && docker ps --format '{{.Names}}' | grep -Fxq "${POSTGRES_CONTAINER}"; then
    local user="wqn"
    local database="wqn"
    if [[ "${DATABASE_URL}" =~ ^postgres://([^:]+):[^@]+@[^/]+/([^?]+) ]]; then
      user="${BASH_REMATCH[1]}"
      database="${BASH_REMATCH[2]}"
    fi
    docker exec -i "${POSTGRES_CONTAINER}" psql -U "${user}" -d "${database}" -v ON_ERROR_STOP=1 -At -c "${sql}"
    return 0
  fi

  echo "cannot query database: install psql or start ${POSTGRES_CONTAINER}" >&2
  exit 1
}

if [[ ! -d "${LOCAL_STORAGE_ROOT}" ]]; then
  echo "storage root does not exist: ${LOCAL_STORAGE_ROOT}" >&2
  exit 1
fi

referenced="${WORK_DIR}/referenced.txt"
actual="${WORK_DIR}/actual.txt"
orphans="${WORK_DIR}/orphans.txt"

run_copy_query "
with referenced as (
  select 'problem-uploads/' || (asset->>'path') as path
  from problems, jsonb_array_elements(coalesce(assets, '[]'::jsonb)) asset
  where nullif(asset->>'path', '') is not null
  union
  select 'problem-uploads/' || (asset->>'path') as path
  from problems, jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) asset
  where nullif(asset->>'path', '') is not null
  union
  select 'problem-uploads/' || file_path as path
  from qr_upload_sessions
  where file_path is not null
  union
  select 'avatars/' || split_part(avatar_url, '/api/files/avatars/', 2) as path
  from user_profiles
  where avatar_url like '/api/files/avatars/%'
)
select path from referenced where path is not null order by path;
" | sort -u >"${referenced}"

for bucket in problem-uploads avatars; do
  if [[ -d "${LOCAL_STORAGE_ROOT}/${bucket}" ]]; then
    find "${LOCAL_STORAGE_ROOT}/${bucket}" -type f -printf "${bucket}/%P\n"
  fi
done | sort -u >"${actual}"

comm -23 "${actual}" "${referenced}" >"${orphans}"

count="$(wc -l <"${orphans}" | tr -d ' ')"
if [[ "${count}" == "0" ]]; then
  echo "no orphan files found"
  exit 0
fi

echo "orphan files: ${count}"
cat "${orphans}"

if [[ "${APPLY}" != "1" ]]; then
  echo "dry run only; rerun with APPLY=1 to delete these files"
  exit 0
fi

while IFS= read -r relative_path; do
  rm -f "${LOCAL_STORAGE_ROOT}/${relative_path}"
done <"${orphans}"

find "${LOCAL_STORAGE_ROOT}/problem-uploads" "${LOCAL_STORAGE_ROOT}/avatars" -type d -empty -delete 2>/dev/null || true
echo "deleted orphan files: ${count}"
