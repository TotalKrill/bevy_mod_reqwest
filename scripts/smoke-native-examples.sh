#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_ADDR="${BEVY_MOD_REQWEST_TEST_SERVER_ADDR:-127.0.0.1:8090}"
BASE_URL="http://${SERVER_ADDR}"
SERVER_LOG="${TMPDIR:-/tmp}/bevy_mod_reqwest-cors-server-native.log"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" 2>/dev/null || true
  fi
}
trap cleanup EXIT

cd "${ROOT}"
"${ROOT}/scripts/cors-test-server.rs" "${SERVER_ADDR}" >"${SERVER_LOG}" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 120); do
  if curl -fsS "${BASE_URL}/random" >/dev/null 2>&1; then
    break
  fi
  if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
    cat "${SERVER_LOG}" >&2 || true
    echo "CORS test server exited early" >&2
    exit 1
  fi
  sleep 1
done

run_example() {
  local example="$1"
  local seconds="$2"
  local expected="$3"
  local log="${TMPDIR:-/tmp}/bevy_mod_reqwest-${example}-native.log"

  echo "smoke-running native example ${example}"
  set +e
  BEVY_MOD_REQWEST_EXAMPLE_BASE_URL="${BASE_URL}" timeout "${seconds}" cargo run --example "${example}" >"${log}" 2>&1
  local status=$?
  set -e

  cat "${log}"

  if [[ "${status}" != 124 && "${status}" != 0 ]]; then
    echo "example ${example} exited with unexpected status ${status}" >&2
    exit "${status}"
  fi

  if ! grep -E "${expected}" "${log}" >/dev/null; then
    echo "example ${example} did not log expected pattern: ${expected}" >&2
    exit 1
  fi
}

run_example minimal 8 'code: 200 OK|local bevy_mod_reqwest CORS test server'
run_example json 8 'Bored \{ activity: "Use the local bevy_mod_reqwest CORS test server"'
run_example post 6 'return data: Ok\(.*id..:101'
