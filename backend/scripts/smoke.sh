#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATABASE_URL="${DATABASE_URL:-postgres://wqn:wqn@127.0.0.1:55432/wqn}"
APP_BIND_ADDR="${APP_BIND_ADDR:-127.0.0.1:18081}"
AUTH_JWT_SECRET="${AUTH_JWT_SECRET:-dev-secret-dev-secret-dev-secret-dev-secret}"
AUTH_COOKIE_NAME="${AUTH_COOKIE_NAME:-wqn_session}"
SMOKE_GEMINI_API_KEY="${WQN_SMOKE_GEMINI_API_KEY:-}"
SMOKE_CRON_SECRET="${WQN_SMOKE_CRON_SECRET:-}"
SMOKE_STORAGE_SCAN_COMMAND="${WQN_SMOKE_STORAGE_SCAN_COMMAND:-true}"
LOCAL_STORAGE_ROOT="${LOCAL_STORAGE_ROOT:-$(mktemp -d /tmp/wqn-smoke-storage.XXXXXX)}"
BASE_URL="${WQN_SMOKE_BASE_URL:-http://${APP_BIND_ADDR}}"
START_SERVER="${WQN_SMOKE_EXTERNAL_SERVER:-0}"
POSTGRES_CONTAINER="${WQN_SMOKE_POSTGRES_CONTAINER:-wqn-postgres-smoke}"

SERVER_PID=""
WORK_DIR="$(mktemp -d /tmp/wqn-smoke.XXXXXX)"
SERVER_LOG="${WORK_DIR}/server.log"

cleanup() {
  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

wait_ready() {
  for _ in $(seq 1 60); do
    if curl -fsS "${BASE_URL}/readyz" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "server did not become ready; log follows:" >&2
  if [[ -f "${SERVER_LOG}" ]]; then
    tail -200 "${SERVER_LOG}" >&2 || true
  fi
  exit 1
}

request_json() {
  local method="$1"
  local path="$2"
  local token="$3"
  local body="$4"
  local expected="$5"
  local output="$6"
  local status

  if [[ -n "${token}" ]]; then
    status="$(
      curl -sS -o "${output}" -w '%{http_code}' \
        -X "${method}" "${BASE_URL}${path}" \
        -H "authorization: Bearer ${token}" \
        -H 'content-type: application/json' \
        -d "${body}"
    )"
  else
    status="$(
      curl -sS -o "${output}" -w '%{http_code}' \
        -X "${method}" "${BASE_URL}${path}" \
        -H 'content-type: application/json' \
        -d "${body}"
    )"
  fi

  if [[ "${status}" != "${expected}" ]]; then
    echo "${method} ${path} expected ${expected}, got ${status}" >&2
    cat "${output}" >&2
    exit 1
  fi
}

request_get() {
  local path="$1"
  local token="$2"
  local expected="$3"
  local output="$4"
  local status

  if [[ -n "${token}" ]]; then
    status="$(curl -sS -o "${output}" -w '%{http_code}' "${BASE_URL}${path}" -H "authorization: Bearer ${token}")"
  else
    status="$(curl -sS -o "${output}" -w '%{http_code}' "${BASE_URL}${path}")"
  fi

  if [[ "${status}" != "${expected}" ]]; then
    echo "GET ${path} expected ${expected}, got ${status}" >&2
    cat "${output}" >&2
    exit 1
  fi
}

request_get_cookie() {
  local path="$1"
  local cookie="$2"
  local expected="$3"
  local output="$4"
  local status

  status="$(curl -sS -o "${output}" -w '%{http_code}' "${BASE_URL}${path}" -H "cookie: ${cookie}")"

  if [[ "${status}" != "${expected}" ]]; then
    echo "GET ${path} with cookie expected ${expected}, got ${status}" >&2
    cat "${output}" >&2
    exit 1
  fi
}

run_sql() {
  local sql="$1"

  if command -v psql >/dev/null 2>&1; then
    psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 -c "${sql}" >/dev/null
    return 0
  fi

  if command -v docker >/dev/null 2>&1 && docker ps --format '{{.Names}}' | grep -Fxq "${POSTGRES_CONTAINER}"; then
    local user="wqn"
    local database="wqn"
    if [[ "${DATABASE_URL}" =~ ^postgres://([^:]+):[^@]+@[^/]+/([^?]+) ]]; then
      user="${BASH_REMATCH[1]}"
      database="${BASH_REMATCH[2]}"
    fi
    docker exec -i "${POSTGRES_CONTAINER}" psql -U "${user}" -d "${database}" -v ON_ERROR_STOP=1 -c "${sql}" >/dev/null
    return 0
  fi

  echo "cannot run SQL: install psql or start ${POSTGRES_CONTAINER}" >&2
  exit 1
}

require_cmd cargo
require_cmd curl
require_cmd jq
require_cmd sqlx

cd "${ROOT_DIR}"
echo "Running migrations against ${DATABASE_URL}"
sqlx migrate run --database-url "${DATABASE_URL}"

if [[ "${START_SERVER}" != "1" ]]; then
  echo "Starting API at ${APP_BIND_ADDR}"
  DATABASE_URL="${DATABASE_URL}" \
  APP_BIND_ADDR="${APP_BIND_ADDR}" \
  AUTH_JWT_SECRET="${AUTH_JWT_SECRET}" \
  AUTH_COOKIE_NAME="${AUTH_COOKIE_NAME}" \
  GEMINI_API_KEY="${SMOKE_GEMINI_API_KEY}" \
  CRON_SECRET="${SMOKE_CRON_SECRET}" \
  LOCAL_STORAGE_SCAN_COMMAND="${SMOKE_STORAGE_SCAN_COMMAND}" \
  LOCAL_STORAGE_ROOT="${LOCAL_STORAGE_ROOT}" \
  cargo run >"${SERVER_LOG}" 2>&1 &
  SERVER_PID="$!"
fi
wait_ready

request_get "/api/problems?search_text=%3Cscript%3E" "" 400 "${WORK_DIR}/suspicious-request.json"

smoke_id="$(date +%s%N)"
email="smoke-${smoke_id}@example.test"
password="password123"

signup_body="${WORK_DIR}/signup.json"
request_json POST /api/auth/sign-up "" \
  "{\"email\":\"${email}\",\"password\":\"${password}\",\"timezone\":\"Asia/Shanghai\"}" \
  200 "${signup_body}"
token="$(jq -r '.data.access_token' "${signup_body}")"
user_id="$(jq -r '.data.user.id' "${signup_body}")"
if [[ -z "${token}" || "${token}" == "null" ]]; then
  echo "sign-up response did not include access_token" >&2
  cat "${signup_body}" >&2
  exit 1
fi

request_json POST /api/auth/sign-up "" \
  "{\"email\":\"${email}\",\"password\":\"${password}\"}" \
  409 "${WORK_DIR}/duplicate-signup.json"
request_json POST /api/auth/login "" \
  "{\"email\":\"${email}\",\"password\":\"wrong-password\"}" \
  401 "${WORK_DIR}/failed-login.json"
request_json POST /api/auth/forgot-password "" \
  "{\"email\":\"${email}\"}" \
  500 "${WORK_DIR}/forgot-password-unconfigured.json"
jq -e '.error == "Password reset email service is not configured"' "${WORK_DIR}/forgot-password-unconfigured.json" >/dev/null

login_body="${WORK_DIR}/login.json"
login_headers="${WORK_DIR}/login.headers"
login_status="$(
  curl -sS -D "${login_headers}" -o "${login_body}" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/auth/login" \
    -H 'content-type: application/json' \
    -d "{\"email\":\"${email}\",\"password\":\"${password}\"}"
)"
if [[ "${login_status}" != "200" ]]; then
  echo "login expected 200, got ${login_status}" >&2
  cat "${login_body}" >&2
  exit 1
fi
if ! grep -qi "^set-cookie: ${AUTH_COOKIE_NAME}=" "${login_headers}" || ! grep -qi 'httponly' "${login_headers}"; then
  echo "login response did not set an HttpOnly ${AUTH_COOKIE_NAME} cookie" >&2
  cat "${login_headers}" >&2
  exit 1
fi
login_cookie="$(grep -i "^set-cookie: ${AUTH_COOKIE_NAME}=" "${login_headers}" | head -n 1 | sed -E 's/^[Ss]et-[Cc]ookie: ([^;]+).*/\1/' | tr -d '\r')"
request_get_cookie /api/auth/me "${login_cookie}" 200 "${WORK_DIR}/cookie-me.json"

request_get /api/auth/me "${token}" 200 "${WORK_DIR}/me-before-password.json"

password_body="${WORK_DIR}/password.json"
request_json PATCH /api/auth/password "${token}" \
  "{\"current_password\":\"${password}\",\"new_password\":\"new-password123\"}" \
  200 "${password_body}"
new_token="$(jq -r '.data.access_token' "${password_body}")"
request_get /api/auth/me "${token}" 401 "${WORK_DIR}/old-token-me.json"
request_get /api/auth/me "${new_token}" 200 "${WORK_DIR}/new-token-me.json"
token="${new_token}"

disabled_email="disabled-${email}"
disabled_body="${WORK_DIR}/disabled-signup.json"
request_json POST /api/auth/sign-up "" \
  "{\"email\":\"${disabled_email}\",\"password\":\"${password}\"}" \
  200 "${disabled_body}"
disabled_token="$(jq -r '.data.access_token' "${disabled_body}")"
disabled_user_id="$(jq -r '.data.user.id' "${disabled_body}")"
run_sql "update app_users set disabled_at = now() where id = '${disabled_user_id}'::uuid"
request_get /api/auth/me "${disabled_token}" 401 "${WORK_DIR}/disabled-me.json"
request_json POST /api/auth/login "" \
  "{\"email\":\"${disabled_email}\",\"password\":\"${password}\"}" \
  401 "${WORK_DIR}/disabled-login.json"

run_sql "update user_profiles set user_role = 'super_admin' where id = '${user_id}'::uuid"
request_json PATCH /api/admin/settings/site_announcement "${token}" \
  '{"value":{"enabled":true,"message":"Smoke announcement","type":"info"},"description":"Smoke announcement"}' \
  200 "${WORK_DIR}/admin-setting-upsert.json"
request_get /api/admin/settings "${token}" 200 "${WORK_DIR}/admin-settings.json"
jq -e '.settings | any(.key == "site_announcement" and .value.enabled == true)' "${WORK_DIR}/admin-settings.json" >/dev/null
request_json POST /api/admin/settings "${token}" \
  '{"key":"orphan_post","value":{"enabled":true}}' \
  405 "${WORK_DIR}/admin-settings-post.json"
request_json DELETE /api/admin/settings/site_announcement "${token}" \
  '{}' \
  405 "${WORK_DIR}/admin-settings-delete.json"

managed_email="managed-${email}"
managed_body="${WORK_DIR}/managed-signup.json"
request_json POST /api/auth/sign-up "" \
  "{\"email\":\"${managed_email}\",\"password\":\"${password}\"}" \
  200 "${managed_body}"
managed_token="$(jq -r '.data.access_token' "${managed_body}")"
managed_user_id="$(jq -r '.data.user.id' "${managed_body}")"
request_get /api/auth/me "${managed_token}" 200 "${WORK_DIR}/managed-me-active.json"
request_get "/api/admin/users?search=${managed_email}" "${token}" 200 "${WORK_DIR}/admin-users-managed-active.json"
jq -e --arg id "${managed_user_id}" \
  '.users | any(.id == $id and .is_active == true and .disabled_at == null)' \
  "${WORK_DIR}/admin-users-managed-active.json" >/dev/null
request_get "/api/admin/users/${managed_user_id}/content-statistics" "${token}" 200 "${WORK_DIR}/admin-managed-content-statistics.json"
jq -e '.data.subjects == 0 and .data.problems == 0 and .data.problem_sets == 0 and .data.attempts == 0' \
  "${WORK_DIR}/admin-managed-content-statistics.json" >/dev/null
request_json PATCH "/api/admin/users/${managed_user_id}/role" "${token}" \
  '{"role":"moderator"}' \
  200 "${WORK_DIR}/admin-managed-role-moderator.json"
jq -e '.data.user_role == "moderator"' \
  "${WORK_DIR}/admin-managed-role-moderator.json" >/dev/null
request_json PATCH "/api/admin/users/${managed_user_id}/role" "${token}" \
  '{"role":"user"}' \
  200 "${WORK_DIR}/admin-managed-role-user.json"
jq -e '.data.user_role == "user"' \
  "${WORK_DIR}/admin-managed-role-user.json" >/dev/null
request_json PATCH "/api/admin/users/${managed_user_id}" "${token}" \
  '{"is_active":false}' \
  200 "${WORK_DIR}/admin-patch-disable-managed.json"
jq -e --arg id "${managed_user_id}" \
  '.data.id == $id and .data.is_active == false and .data.disabled_at != null' \
  "${WORK_DIR}/admin-patch-disable-managed.json" >/dev/null
request_get /api/auth/me "${managed_token}" 401 "${WORK_DIR}/managed-me-patch-disabled.json"
request_json POST /api/auth/login "" \
  "{\"email\":\"${managed_email}\",\"password\":\"${password}\"}" \
  401 "${WORK_DIR}/managed-login-patch-disabled.json"
request_json PATCH "/api/admin/users/${managed_user_id}" "${token}" \
  '{"is_active":true}' \
  200 "${WORK_DIR}/admin-patch-restore-managed.json"
jq -e --arg id "${managed_user_id}" \
  '.data.id == $id and .data.is_active == true and .data.disabled_at == null' \
  "${WORK_DIR}/admin-patch-restore-managed.json" >/dev/null
request_get /api/auth/me "${managed_token}" 401 "${WORK_DIR}/managed-old-token-after-patch-restore.json"
managed_patch_login_body="${WORK_DIR}/managed-login-patch-restored.json"
request_json POST /api/auth/login "" \
  "{\"email\":\"${managed_email}\",\"password\":\"${password}\"}" \
  200 "${managed_patch_login_body}"
managed_token="$(jq -r '.data.access_token' "${managed_patch_login_body}")"
request_get /api/auth/me "${managed_token}" 200 "${WORK_DIR}/managed-me-patch-restored.json"

request_json PATCH "/api/admin/users/${managed_user_id}/toggle-active" "${token}" \
  '{}' \
  200 "${WORK_DIR}/admin-disable-managed.json"
jq -e --arg id "${managed_user_id}" \
  '.data.id == $id and .data.is_active == false and .data.disabled_at != null' \
  "${WORK_DIR}/admin-disable-managed.json" >/dev/null
request_get /api/auth/me "${managed_token}" 401 "${WORK_DIR}/managed-me-disabled.json"
request_json POST /api/auth/login "" \
  "{\"email\":\"${managed_email}\",\"password\":\"${password}\"}" \
  401 "${WORK_DIR}/managed-login-disabled.json"
request_get /api/admin/statistics "${token}" 200 "${WORK_DIR}/admin-statistics-after-disable.json"
jq -e '.statistics.total_users >= 3 and .statistics.active_users < .statistics.total_users' \
  "${WORK_DIR}/admin-statistics-after-disable.json" >/dev/null

request_json PATCH "/api/admin/users/${managed_user_id}/toggle-active" "${token}" \
  '{}' \
  200 "${WORK_DIR}/admin-restore-managed.json"
jq -e --arg id "${managed_user_id}" \
  '.data.id == $id and .data.is_active == true and .data.disabled_at == null' \
  "${WORK_DIR}/admin-restore-managed.json" >/dev/null
request_get /api/auth/me "${managed_token}" 401 "${WORK_DIR}/managed-old-token-after-restore.json"
managed_login_body="${WORK_DIR}/managed-login-restored.json"
request_json POST /api/auth/login "" \
  "{\"email\":\"${managed_email}\",\"password\":\"${password}\"}" \
  200 "${managed_login_body}"
managed_restored_token="$(jq -r '.data.access_token' "${managed_login_body}")"
request_get /api/auth/me "${managed_restored_token}" 200 "${WORK_DIR}/managed-me-restored.json"

request_json DELETE "/api/admin/users/${managed_user_id}" "${token}" \
  '{}' \
  200 "${WORK_DIR}/admin-delete-managed.json"
request_get /api/auth/me "${managed_restored_token}" 401 "${WORK_DIR}/managed-me-deleted.json"
request_json POST /api/auth/login "" \
  "{\"email\":\"${managed_email}\",\"password\":\"${password}\"}" \
  401 "${WORK_DIR}/managed-login-deleted.json"
request_get "/api/admin/users?search=${managed_email}" "${token}" 200 "${WORK_DIR}/admin-users-managed-deleted.json"
jq -e --arg id "${managed_user_id}" \
  '(.users | map(select(.id == $id)) | length) == 0' \
  "${WORK_DIR}/admin-users-managed-deleted.json" >/dev/null

other_email="other-${email}"
other_body="${WORK_DIR}/other-signup.json"
request_json POST /api/auth/sign-up "" \
  "{\"email\":\"${other_email}\",\"password\":\"${password}\"}" \
  200 "${other_body}"
other_token="$(jq -r '.data.access_token' "${other_body}")"

subject_body="${WORK_DIR}/subject.json"
request_json POST /api/subjects "${token}" \
  '{"name":"Smoke Math","color":"#3366ff","icon":"Calculator"}' \
  201 "${subject_body}"
subject_id="$(jq -r '.data.id' "${subject_body}")"

problem_body="${WORK_DIR}/problem.json"
request_json POST /api/problems "${token}" \
  "{\"subject_id\":\"${subject_id}\",\"title\":\"Smoke problem\",\"problem_type\":\"short\",\"status\":\"needs_review\"}" \
  201 "${problem_body}"
problem_id="$(jq -r '.data.id' "${problem_body}")"

session_body="${WORK_DIR}/session.json"
request_json POST /api/review-sessions/start-spaced "${token}" \
  "{\"subject_id\":\"${subject_id}\",\"session_size\":1}" \
  201 "${session_body}"
session_id="$(jq -r '.data.sessionId' "${session_body}")"
request_json PATCH "/api/review-sessions/${session_id}/progress" "${token}" \
  "{\"problemId\":\"${problem_id}\",\"wasCorrect\":true,\"elapsed_ms\":90000}" \
  200 "${WORK_DIR}/progress.json"

stats_body="${WORK_DIR}/statistics.json"
request_get /api/statistics "${token}" 200 "${stats_body}"
jq -e '
  .data.streaks.current_streak >= 1 and
  .data.streaks.longest_streak >= 1 and
  .data.sessionStats.avg_duration_ms == 90000 and
  .data.sessionStats.total_review_time_ms == 90000
' "${stats_body}" >/dev/null

attempt_body="${WORK_DIR}/attempt.json"
request_json POST /api/attempts "${token}" \
  "{\"problem_id\":\"${problem_id}\",\"submitted_answer\":{\"text\":\"smoke\"},\"is_correct\":false,\"selected_status\":\"wrong\",\"cause\":\"Smoke misconception\"}" \
  201 "${attempt_body}"
attempt_id="$(jq -r '.data.id' "${attempt_body}")"
run_sql "update attempts set created_at = '2024-01-01 16:30:00+00'::timestamptz where id = '${attempt_id}'::uuid"
request_get /api/statistics "${token}" 200 "${WORK_DIR}/statistics-after-boundary-attempt.json"
jq -e '
  .data.timezone == "Asia/Shanghai" and
  (.data.activityHeatmap | any(.activity_date == "2024-01-02" and .activity_count >= 1)) and
  (.data.activityHeatmap | all(.activity_date != "2024-01-01"))
' "${WORK_DIR}/statistics-after-boundary-attempt.json" >/dev/null
run_sql "
do \$\$
declare
  expected_next timestamptz := ((timezone('Asia/Shanghai', now())::date + 1)::timestamp at time zone 'Asia/Shanghai');
  schedule record;
begin
  select repetition_number, ease_factor, interval_days, next_review_at, last_reviewed_at
  into schedule
  from review_schedule
  where user_id = '${user_id}'::uuid and problem_id = '${problem_id}'::uuid;

  if not found then
    raise exception 'review schedule was not created';
  end if;
  if schedule.repetition_number <> 0
    or schedule.interval_days <> 1
    or abs(schedule.ease_factor - 2.5) > 0.0001
    or abs(extract(epoch from (schedule.next_review_at - expected_next))) > 2
    or schedule.last_reviewed_at is null
  then
    raise exception 'review schedule did not use local-midnight first-review policy: %', schedule;
  end if;
end;
\$\$;"
second_attempt_body="${WORK_DIR}/attempt-same-day.json"
request_json POST /api/attempts "${token}" \
  "{\"problem_id\":\"${problem_id}\",\"submitted_answer\":{\"text\":\"smoke retry\"},\"is_correct\":true,\"selected_status\":\"mastered\"}" \
  201 "${second_attempt_body}"
run_sql "
do \$\$
declare
  expected_next timestamptz := ((timezone('Asia/Shanghai', now())::date + 1)::timestamp at time zone 'Asia/Shanghai');
  schedule record;
begin
  select repetition_number, ease_factor, interval_days, next_review_at, last_reviewed_at
  into schedule
  from review_schedule
  where user_id = '${user_id}'::uuid and problem_id = '${problem_id}'::uuid;

  if not found then
    raise exception 'review schedule was not found after same-day review';
  end if;
  if schedule.repetition_number <> 0
    or schedule.interval_days <> 1
    or abs(schedule.ease_factor - 2.5) > 0.0001
    or abs(extract(epoch from (schedule.next_review_at - expected_next))) > 2
  then
    raise exception 'same-day review advanced SM-2 state unexpectedly: %', schedule;
  end if;
end;
\$\$;"
insight_body="${WORK_DIR}/insight-generate.json"
request_json POST /api/insights/generate "${token}" '{}' 201 "${insight_body}"
jq -e '
  (.data.headline | type == "string" and length > 0) and
  (.data.status == "completed") and
  (.data.weak_spots | type == "array") and
  (.data.topic_clusters | type == "object")
' "${insight_body}" >/dev/null
insight_status_body="${WORK_DIR}/insight-status.json"
request_get /api/insights/status "${token}" 200 "${insight_status_body}"
jq -e '
  .data.status == "completed" and
  (.data.latest.headline | type == "string" and length > 0) and
  (.data.digest.headline | type == "string" and length > 0)
' "${insight_status_body}" >/dev/null
cron_digest_body="${WORK_DIR}/cron-digests.json"
if [[ -n "${SMOKE_CRON_SECRET}" ]]; then
  cron_status="$(curl -sS -o "${cron_digest_body}" -w '%{http_code}' "${BASE_URL}/api/cron/generate-digests" -H "x-cron-secret: ${SMOKE_CRON_SECRET}")"
  if [[ "${cron_status}" != "200" ]]; then
    echo "cron digest expected 200, got ${cron_status}" >&2
    cat "${cron_digest_body}" >&2
    exit 1
  fi
else
  request_get /api/cron/generate-digests "" 200 "${cron_digest_body}"
fi
jq -e '.processed >= 1 and .completed >= 1' "${cron_digest_body}" >/dev/null

invalid_upload_file="${WORK_DIR}/upload.txt"
printf 'smoke upload\n' >"${invalid_upload_file}"
invalid_upload_status="$(
  curl -sS -o "${WORK_DIR}/invalid-upload.json" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/files/upload" \
    -H "authorization: Bearer ${token}" \
    -F role=problem \
    -F "problem_id=${problem_id}" \
    -F "file=@${invalid_upload_file};type=text/plain"
)"
if [[ "${invalid_upload_status}" != "400" ]]; then
  echo "text/plain upload expected 400, got ${invalid_upload_status}" >&2
  cat "${WORK_DIR}/invalid-upload.json" >&2
  exit 1
fi

large_file="${WORK_DIR}/large.pdf"
dd if=/dev/zero of="${large_file}" bs=1 count=0 seek=10485761 >/dev/null 2>&1
large_upload_status="$(
  curl -sS -o "${WORK_DIR}/large-upload.json" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/files/upload" \
    -H "authorization: Bearer ${token}" \
    -F role=problem \
    -F "problem_id=${problem_id}" \
    -F "file=@${large_file};type=application/pdf"
)"
if [[ "${large_upload_status}" != "400" ]]; then
  echo "oversized upload expected 400, got ${large_upload_status}" >&2
  cat "${WORK_DIR}/large-upload.json" >&2
  exit 1
fi

qr_invalid_session="${WORK_DIR}/qr-invalid-session.json"
request_json POST /api/qr-sessions "${token}" '{}' 200 "${qr_invalid_session}"
qr_invalid_session_id="$(jq -r '.data.sessionId' "${qr_invalid_session}")"
qr_invalid_token="$(jq -r '.data.token' "${qr_invalid_session}")"
qr_invalid_status="$(
  curl -sS -o "${WORK_DIR}/qr-invalid-upload.json" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/qr-upload/${qr_invalid_session_id}?token=${qr_invalid_token}" \
    -F "file=@${invalid_upload_file};type=text/plain;filename=qr.txt"
)"
if [[ "${qr_invalid_status}" != "400" ]]; then
  echo "QR text/plain upload expected 400, got ${qr_invalid_status}" >&2
  cat "${WORK_DIR}/qr-invalid-upload.json" >&2
  exit 1
fi
request_get "/api/qr-sessions/${qr_invalid_session_id}/status" "${token}" 200 "${WORK_DIR}/qr-invalid-status.json"
jq -e '.data.status == "pending" and .data.filePath == null' "${WORK_DIR}/qr-invalid-status.json" >/dev/null

qr_large_session="${WORK_DIR}/qr-large-session.json"
request_json POST /api/qr-sessions "${token}" '{}' 200 "${qr_large_session}"
qr_large_session_id="$(jq -r '.data.sessionId' "${qr_large_session}")"
qr_large_token="$(jq -r '.data.token' "${qr_large_session}")"
qr_large_status="$(
  curl -sS -o "${WORK_DIR}/qr-large-upload.json" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/qr-upload/${qr_large_session_id}?token=${qr_large_token}" \
    -F "file=@${large_file};type=application/pdf;filename=large.pdf"
)"
if [[ "${qr_large_status}" != "400" ]]; then
  echo "QR oversized upload expected 400, got ${qr_large_status}" >&2
  cat "${WORK_DIR}/qr-large-upload.json" >&2
  exit 1
fi
request_get "/api/qr-sessions/${qr_large_session_id}/status" "${token}" 200 "${WORK_DIR}/qr-large-status.json"
jq -e '.data.status == "pending" and .data.filePath == null' "${WORK_DIR}/qr-large-status.json" >/dev/null

qr_valid_session="${WORK_DIR}/qr-valid-session.json"
request_json POST /api/qr-sessions "${token}" '{}' 200 "${qr_valid_session}"
qr_valid_session_id="$(jq -r '.data.sessionId' "${qr_valid_session}")"
qr_valid_token="$(jq -r '.data.token' "${qr_valid_session}")"
qr_valid_file="${WORK_DIR}/qr-valid.png"
printf 'fake qr png bytes\n' >"${qr_valid_file}"
qr_valid_status="$(
  curl -sS -o "${WORK_DIR}/qr-valid-upload.json" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/qr-upload/${qr_valid_session_id}?token=${qr_valid_token}" \
    -F "file=@${qr_valid_file};type=image/png;filename=qr upload?.png"
)"
if [[ "${qr_valid_status}" != "200" ]]; then
  echo "QR image upload expected 200, got ${qr_valid_status}" >&2
  cat "${WORK_DIR}/qr-valid-upload.json" >&2
  exit 1
fi
request_get "/api/qr-sessions/${qr_valid_session_id}/status" "${token}" 200 "${WORK_DIR}/qr-valid-status.json"
qr_file_path="$(jq -r '.data.filePath' "${WORK_DIR}/qr-valid-status.json")"
jq -e --arg session "${qr_valid_session_id}" '
  .data.status == "uploaded" and
  .data.mimeType == "image/png" and
  (.data.filePath | contains("/qr/" + $session + "/qr_upload_.png"))
' "${WORK_DIR}/qr-valid-status.json" >/dev/null
request_json POST "/api/qr-sessions/${qr_valid_session_id}/consume" "${token}" \
  '{}' \
  200 "${WORK_DIR}/qr-consume.json"
jq -e --arg path "${qr_file_path}" \
  '.data.filePath == $path and .data.mimeType == "image/png"' \
  "${WORK_DIR}/qr-consume.json" >/dev/null
request_get "/api/files/${qr_file_path}" "${token}" 200 "${WORK_DIR}/qr-file.png"

upload_file="${WORK_DIR}/upload.jpg"
printf 'fake image bytes\n' >"${upload_file}"
upload_body="${WORK_DIR}/upload.json"
upload_status="$(
  curl -sS -o "${upload_body}" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/files/upload" \
    -H "authorization: Bearer ${token}" \
    -F role=problem \
    -F "problem_id=${problem_id}" \
    -F "file=@${upload_file};type=image/jpeg"
)"
if [[ "${upload_status}" != "200" ]]; then
  echo "file upload expected 200, got ${upload_status}" >&2
  cat "${upload_body}" >&2
  exit 1
fi
file_path="$(jq -r '.data.paths[0]' "${upload_body}")"
request_json PATCH "/api/problems/${problem_id}/assets" "${token}" \
  "{\"assets\":[{\"path\":\"${file_path}\",\"kind\":\"image\"}]}" \
  200 "${WORK_DIR}/problem-assets.json"
request_get "/api/files/${file_path}" "${token}" 200 "${WORK_DIR}/uploaded-file.txt"
request_get "/api/files/${file_path}" "${other_token}" 404 "${WORK_DIR}/private-file-other.txt"
request_json DELETE /api/files/delete "" "{\"path\":\"${file_path}\"}" 401 "${WORK_DIR}/anonymous-delete.json"
request_json DELETE /api/files/delete "${other_token}" "{\"path\":\"${file_path}\"}" 403 "${WORK_DIR}/other-delete.json"

request_json POST /api/problem-sets "${token}" \
  "{\"subject_id\":\"${subject_id}\",\"name\":\"Discover ${smoke_id} A\",\"sharing_level\":\"public\",\"problem_ids\":[\"${problem_id}\"]}" \
  201 "${WORK_DIR}/public-set.json"
public_set_id="$(jq -r '.data.id' "${WORK_DIR}/public-set.json")"
request_json PUT "/api/problem-sets/${public_set_id}" "${token}" \
  '{"is_listed":true,"discovery_subject":"Mathematics"}' \
  200 "${WORK_DIR}/public-set-listed.json"
request_get "/api/files/${file_path}" "" 200 "${WORK_DIR}/public-file.txt"

request_json POST /api/problem-sets "${token}" \
  "{\"subject_id\":\"${subject_id}\",\"name\":\"Discover ${smoke_id} B\",\"sharing_level\":\"public\",\"problem_ids\":[\"${problem_id}\"]}" \
  201 "${WORK_DIR}/public-set-b.json"
public_set_b_id="$(jq -r '.data.id' "${WORK_DIR}/public-set-b.json")"
request_json PUT "/api/problem-sets/${public_set_b_id}" "${token}" \
  '{"is_listed":true,"discovery_subject":"Mathematics"}' \
  200 "${WORK_DIR}/public-set-b-listed.json"

request_json POST /api/problem-sets "${token}" \
  "{\"subject_id\":\"${subject_id}\",\"name\":\"Discover ${smoke_id} C\",\"sharing_level\":\"public\",\"problem_ids\":[\"${problem_id}\"]}" \
  201 "${WORK_DIR}/public-set-c.json"
public_set_c_id="$(jq -r '.data.id' "${WORK_DIR}/public-set-c.json")"
request_json PUT "/api/problem-sets/${public_set_c_id}" "${token}" \
  '{"is_listed":true,"discovery_subject":"Mathematics"}' \
  200 "${WORK_DIR}/public-set-c-listed.json"

request_json POST "/api/problem-sets/${public_set_b_id}/like" "${other_token}" \
  '{}' \
  200 "${WORK_DIR}/public-set-b-like.json"

discover_q="$(jq -nr --arg q "Discover ${smoke_id}" '$q|@uri')"
request_get "/api/discover?limit=2&sort=newest&q=${discover_q}" "" 200 "${WORK_DIR}/discover-page-1.json"
jq -e '
  (.data.items | length) == 2
  and .data.data == .data.items
  and (.data.next_cursor | type == "string")
  and all(.data.items[]; .owner.display_name != null and .stats.problem_count >= 1)
' "${WORK_DIR}/discover-page-1.json" >/dev/null
discover_cursor="$(jq -r '.data.next_cursor | @uri' "${WORK_DIR}/discover-page-1.json")"
request_get "/api/discover?limit=2&sort=newest&q=${discover_q}&cursor=${discover_cursor}" "" 200 "${WORK_DIR}/discover-page-2.json"
jq -e --slurpfile first "${WORK_DIR}/discover-page-1.json" '
  ([.data.items[].id] - [$first[0].data.items[].id]) as $second_only
  | (.data.items | length) >= 1
  and ($second_only | length) == (.data.items | length)
' "${WORK_DIR}/discover-page-2.json" >/dev/null
request_get "/api/discover?limit=3&sort=most_liked&q=${discover_q}" "" 200 "${WORK_DIR}/discover-most-liked.json"
jq -e --arg id "${public_set_b_id}" '
  .data.items[0].id == $id
  and .data.items[0].like_count == 1
  and .data.items[0].stats.ranking_score >= 3
' "${WORK_DIR}/discover-most-liked.json" >/dev/null
request_get "/api/discover?cursor=not-a-cursor" "" 400 "${WORK_DIR}/discover-invalid-cursor.json"

limited_problem_body="${WORK_DIR}/limited-problem.json"
request_json POST /api/problems "${token}" \
  "{\"subject_id\":\"${subject_id}\",\"title\":\"Smoke limited problem\",\"problem_type\":\"short\",\"status\":\"needs_review\"}" \
  201 "${limited_problem_body}"
limited_problem_id="$(jq -r '.data.id' "${limited_problem_body}")"
limited_upload_file="${WORK_DIR}/limited-upload.png"
printf 'fake png bytes\n' >"${limited_upload_file}"
limited_upload_body="${WORK_DIR}/limited-upload.json"
limited_upload_status="$(
  curl -sS -o "${limited_upload_body}" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/files/upload" \
    -H "authorization: Bearer ${token}" \
    -F role=problem \
    -F "problem_id=${limited_problem_id}" \
    -F "file=@${limited_upload_file};type=image/png"
)"
if [[ "${limited_upload_status}" != "200" ]]; then
  echo "limited file upload expected 200, got ${limited_upload_status}" >&2
  cat "${limited_upload_body}" >&2
  exit 1
fi
limited_file_path="$(jq -r '.data.paths[0]' "${limited_upload_body}")"
request_json PATCH "/api/problems/${limited_problem_id}/assets" "${token}" \
  "{\"assets\":[{\"path\":\"${limited_file_path}\",\"kind\":\"image\"}]}" \
  200 "${WORK_DIR}/limited-problem-assets.json"
request_json POST /api/problem-sets "${token}" \
  "{\"subject_id\":\"${subject_id}\",\"name\":\"Smoke limited set\",\"sharing_level\":\"limited\",\"shared_with_emails\":[\"${other_email}\"],\"problem_ids\":[\"${limited_problem_id}\"]}" \
  201 "${WORK_DIR}/limited-set.json"
request_get "/api/files/${limited_file_path}" "" 404 "${WORK_DIR}/limited-file-anonymous.txt"
request_get "/api/files/${limited_file_path}" "${other_token}" 200 "${WORK_DIR}/limited-file-shared.txt"

request_json DELETE /api/files/delete "${token}" "{\"path\":\"${file_path}\"}" 200 "${WORK_DIR}/owner-delete.json"
request_json DELETE /api/files/delete "${token}" "{\"path\":\"${limited_file_path}\"}" 200 "${WORK_DIR}/owner-delete-limited.json"

avatar_file="${WORK_DIR}/avatar.jpg"
printf 'fake image bytes\n' >"${avatar_file}"
avatar_body="${WORK_DIR}/avatar.json"
avatar_status="$(
  curl -sS -o "${avatar_body}" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/profile/avatar" \
    -H "authorization: Bearer ${token}" \
    -F "file=@${avatar_file};type=image/jpeg"
)"
if [[ "${avatar_status}" != "200" ]]; then
  echo "avatar upload expected 200, got ${avatar_status}" >&2
  cat "${avatar_body}" >&2
  exit 1
fi
avatar_url="$(jq -r '.data.avatar_url' "${avatar_body}")"
request_get "${avatar_url}" "" 200 "${WORK_DIR}/avatar-response.jpg"

logout_headers="${WORK_DIR}/logout.headers"
logout_body="${WORK_DIR}/logout.json"
logout_status="$(
  curl -sS -D "${logout_headers}" -o "${logout_body}" -w '%{http_code}' \
    -X POST "${BASE_URL}/api/auth/logout" \
    -H "cookie: ${login_cookie}" \
    -H 'content-type: application/json' \
    -d '{}'
)"
if [[ "${logout_status}" != "200" ]]; then
  echo "logout expected 200, got ${logout_status}" >&2
  cat "${logout_body}" >&2
  exit 1
fi
if ! grep -qi "^set-cookie: ${AUTH_COOKIE_NAME}=;" "${logout_headers}" || ! grep -qi 'max-age=0' "${logout_headers}"; then
  echo "logout response did not clear ${AUTH_COOKIE_NAME}" >&2
  cat "${logout_headers}" >&2
  exit 1
fi

echo "Smoke passed for ${BASE_URL}"
