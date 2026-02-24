#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

print_mux_version() {
  local bin_name="$1"
  local version_args="$2"

  if ! command -v "$bin_name" >/dev/null 2>&1; then
    echo "[mux-check] ${bin_name}: NOT INSTALLED"
    return 0
  fi

  local version_output
  if version_output="$($bin_name $=version_args 2>&1)"; then
    echo "[mux-check] ${bin_name}: ${version_output}"
  else
    echo "[mux-check] ${bin_name}: INSTALLED (version check failed: ${version_output})"
  fi
}

echo "[mux-check] python: $(python3 --version 2>&1)"
print_mux_version "tmux" "-V"
print_mux_version "zellij" "--version"

echo "[mux-check] running base smoke"
zsh smoke_test/python_tui_smoke.sh

echo "[mux-check] done"
echo "[mux-check] next: run manual checklist in docs/tui/python-tui-regression-checklist.md"
