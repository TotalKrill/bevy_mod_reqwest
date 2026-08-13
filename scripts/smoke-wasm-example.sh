#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_ADDR="${BEVY_MOD_REQWEST_TEST_SERVER_ADDR:-127.0.0.1:8090}"
WASM_ADDR="${WASM_SERVER_RUNNER_ADDRESS:-127.0.0.1:8084}"
CDP_PORT="${BEVY_MOD_REQWEST_CHROMIUM_CDP_PORT:-9222}"
BASE_URL="http://${SERVER_ADDR}"
WASM_URL="http://${WASM_ADDR}"
TMP="${TMPDIR:-/tmp}"
SERVER_LOG="${TMP}/bevy_mod_reqwest-cors-server-wasm.log"
RUNNER_LOG="${TMP}/bevy_mod_reqwest-wasm-runner.log"
CHROMIUM_LOG="${TMP}/bevy_mod_reqwest-chromium.log"
CHROMIUM_PROFILE="${TMP}/bevy_mod_reqwest-chromium-profile"
WATCHER="${TMP}/bevy_mod_reqwest-cdp-watch.js"

cleanup() {
  kill "${SERVER_PID:-}" "${RUNNER_PID:-}" "${CHROMIUM_PID:-}" 2>/dev/null || true
}
trap cleanup EXIT

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

need cargo
need chromium
need curl
need node
need wasm-server-runner

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

rm -rf "${CHROMIUM_PROFILE}"
chromium \
  --headless=new \
  --disable-gpu \
  --no-sandbox \
  --remote-debugging-port="${CDP_PORT}" \
  --user-data-dir="${CHROMIUM_PROFILE}" \
  about:blank >"${CHROMIUM_LOG}" 2>&1 &
CHROMIUM_PID=$!

BEVY_MOD_REQWEST_EXAMPLE_BASE_URL="${BASE_URL}" \
WASM_SERVER_RUNNER_ADDRESS="${WASM_ADDR}" \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-server-runner \
  cargo run --target wasm32-unknown-unknown --example minimal >"${RUNNER_LOG}" 2>&1 &
RUNNER_PID=$!

for _ in $(seq 1 180); do
  if curl -fsS "http://127.0.0.1:${CDP_PORT}/json/version" >/dev/null 2>&1 \
    && grep -q "starting webserver" "${RUNNER_LOG}" 2>/dev/null; then
    break
  fi
  if ! kill -0 "${RUNNER_PID}" 2>/dev/null; then
    cat "${RUNNER_LOG}" >&2 || true
    echo "wasm runner exited early" >&2
    exit 1
  fi
  sleep 1
done

cat >"${WATCHER}" <<'JS'
const url = process.argv[2];
const port = Number(process.argv[3]);
const timeoutMs = Number(process.argv[4]);
const expected = /code: 200 OK|local bevy_mod_reqwest CORS test server/i;
const http = require('http');

function requestJson(method, path) {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: '127.0.0.1', port, path, method }, res => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try { resolve(JSON.parse(data)); }
        catch { reject(new Error(`${method} ${path}: ${data}`)); }
      });
    });
    req.on('error', reject);
    req.end();
  });
}

let nextId = 0;
function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  const pending = new Map();
  ws.cdp = (method, params = {}) => {
    const id = ++nextId;
    ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => pending.set(id, { resolve, reject, method }));
  };
  ws.onmessage = event => {
    const msg = JSON.parse(event.data);
    if (msg.id && pending.has(msg.id)) {
      const pendingRequest = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? pendingRequest.reject(new Error(`${pendingRequest.method}: ${JSON.stringify(msg.error)}`)) : pendingRequest.resolve(msg.result);
      return;
    }
    ws.oncdpevent?.(msg);
  };
  return new Promise((resolve, reject) => {
    ws.onopen = () => resolve(ws);
    ws.onerror = reject;
  });
}

(async () => {
  const target = await requestJson('PUT', '/json/new?about:blank');
  const ws = await connect(target.webSocketDebuggerUrl);
  let matched = false;
  let failed = false;

  const emit = line => {
    console.log(line);
    if (expected.test(line)) matched = true;
    if (/panicked|uncaught|exception|test result: FAILED/i.test(line)) failed = true;
  };

  ws.oncdpevent = msg => {
    if (msg.method === 'Runtime.consoleAPICalled') {
      const text = msg.params.args.map(arg => arg.value ?? arg.description ?? arg.unserializableValue ?? '').join(' ');
      emit(`[console.${msg.params.type}] ${text}`);
    } else if (msg.method === 'Runtime.exceptionThrown') {
      emit(`[exception] ${msg.params.exceptionDetails.text}`);
    } else if (msg.method === 'Log.entryAdded') {
      emit(`[log.${msg.params.entry.level}] ${msg.params.entry.url || ''} ${msg.params.entry.text}`);
    } else if (msg.method === 'Network.loadingFailed') {
      emit(`[netfail] ${msg.params.errorText} ${msg.params.blockedReason || ''} ${msg.params.type}`);
    }
  };

  await ws.cdp('Runtime.enable');
  await ws.cdp('Log.enable');
  await ws.cdp('Network.enable');
  await ws.cdp('Page.enable');
  await ws.cdp('Page.navigate', { url });

  setTimeout(() => {
    console.log(`[cdp] matched=${matched} failed=${failed}`);
    process.exit(failed ? 1 : (matched ? 0 : 2));
  }, timeoutMs);
})().catch(error => {
  console.error(error);
  process.exit(1);
});
JS

node "${WATCHER}" "${WASM_URL}" "${CDP_PORT}" 20000

cat "${RUNNER_LOG}"
