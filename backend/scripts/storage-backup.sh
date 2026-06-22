#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_STORAGE_ROOT="${LOCAL_STORAGE_ROOT:-${ROOT_DIR}/storage}"
BACKUP_DIR="${STORAGE_BACKUP_DIR:-${ROOT_DIR}/storage-backups}"
RETENTION_DAYS="${STORAGE_BACKUP_RETENTION_DAYS:-14}"

if [[ ! -d "${LOCAL_STORAGE_ROOT}" ]]; then
  echo "storage root does not exist: ${LOCAL_STORAGE_ROOT}" >&2
  exit 1
fi

mkdir -p "${BACKUP_DIR}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
archive="${BACKUP_DIR}/wqn-storage-${timestamp}.tar.gz"
manifest="${BACKUP_DIR}/wqn-storage-${timestamp}.sha256"

tar -C "${LOCAL_STORAGE_ROOT}" -czf "${archive}" .
sha256sum "${archive}" >"${manifest}"

if [[ "${RETENTION_DAYS}" =~ ^[0-9]+$ ]] && [[ "${RETENTION_DAYS}" -gt 0 ]]; then
  find "${BACKUP_DIR}" -type f \
    \( -name 'wqn-storage-*.tar.gz' -o -name 'wqn-storage-*.sha256' \) \
    -mtime "+${RETENTION_DAYS}" -delete
fi

echo "backup: ${archive}"
echo "manifest: ${manifest}"
