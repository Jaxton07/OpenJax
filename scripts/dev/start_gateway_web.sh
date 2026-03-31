#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"
LOCAL_DEV_WORKDIR="${OPENJAX_LOCAL_DEV_WORKDIR:-$ROOT_DIR/.local-dev-test}"
mkdir -p "$LOCAL_DEV_WORKDIR"

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
API_KEYS="${OPENJAX_GATEWAY_API_KEYS:-${OPENJAX_API_KEYS:-}}"
cleanup_done=0

echo "[run-web-dev] starting gateway on ${GATEWAY_BIND}"
echo "[run-web-dev] gateway cwd: ${LOCAL_DEV_WORKDIR}"
if [ -n "$API_KEYS" ]; then
  (
    cd "$LOCAL_DEV_WORKDIR"
    OPENJAX_GATEWAY_BIND="$GATEWAY_BIND" OPENJAX_GATEWAY_API_KEYS="$API_KEYS" \
      cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p openjax-gateway
  ) &
else
  (
    cd "$LOCAL_DEV_WORKDIR"
    OPENJAX_GATEWAY_BIND="$GATEWAY_BIND" \
      cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p openjax-gateway
  ) &
fi
gateway_pid=$!

echo "[run-web-dev] starting web dev server on http://127.0.0.1:5173"
(cd ui/web && pnpm dev --host 127.0.0.1 --port 5173) &
web_pid=$!

terminate_tree() {
  local pid="$1"
  if ! kill -0 "$pid" 2>/dev/null; then
    return 0
  fi

  local children
  children="$(pgrep -P "$pid" 2>/dev/null || true)"
  if [ -n "$children" ]; then
    local child
    for child in $children; do
      terminate_tree "$child"
    done
  fi

  kill "$pid" 2>/dev/null || true
}

cleanup() {
  if [ "$cleanup_done" -eq 1 ]; then
    return 0
  fi
  cleanup_done=1

  echo
  echo "[run-web-dev] stopping processes..."
  terminate_tree "$gateway_pid"
  terminate_tree "$web_pid"
  wait "$gateway_pid" "$web_pid" 2>/dev/null || true
}

on_interrupt() {
  exit 130
}

trap on_interrupt INT TERM
trap cleanup EXIT

echo "[run-web-dev] ready"
echo "[run-web-dev] gateway: http://${GATEWAY_BIND}"
echo "[run-web-dev] web:     http://127.0.0.1:5173"
echo "[run-web-dev] default dev cwd: ${LOCAL_DEV_WORKDIR}"
echo "[run-web-dev] press Ctrl+C to stop both"

wait "$gateway_pid" "$web_pid"
