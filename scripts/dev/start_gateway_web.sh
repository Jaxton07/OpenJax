#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[run-web-dev] missing cargo"
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "[run-web-dev] missing pnpm"
  exit 1
fi

if [ ! -d "ui/web/node_modules" ]; then
  echo "[run-web-dev] ui/web/node_modules not found, running pnpm install..."
  (cd ui/web && pnpm install)
fi

GATEWAY_BIND="${OPENJAX_GATEWAY_BIND:-127.0.0.1:8765}"
API_KEYS="${OPENJAX_GATEWAY_API_KEYS:-${OPENJAX_API_KEYS:-dev-key}}"

echo "[run-web-dev] starting gateway on ${GATEWAY_BIND}"
OPENJAX_GATEWAY_BIND="$GATEWAY_BIND" OPENJAX_GATEWAY_API_KEYS="$API_KEYS" \
  cargo run -p openjax-gateway &
gateway_pid=$!

echo "[run-web-dev] starting web dev server on http://127.0.0.1:5173"
(cd ui/web && pnpm dev --host 127.0.0.1 --port 5173) &
web_pid=$!

cleanup() {
  echo
  echo "[run-web-dev] stopping processes..."
  kill "$gateway_pid" "$web_pid" 2>/dev/null || true
  wait "$gateway_pid" "$web_pid" 2>/dev/null || true
}

trap cleanup INT TERM EXIT

echo "[run-web-dev] ready"
echo "[run-web-dev] gateway: http://${GATEWAY_BIND}"
echo "[run-web-dev] web:     http://127.0.0.1:5173"
echo "[run-web-dev] press Ctrl+C to stop both"

wait "$gateway_pid" "$web_pid"
