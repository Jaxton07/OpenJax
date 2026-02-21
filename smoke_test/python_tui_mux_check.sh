#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

echo "[mux-check] python: $(python3 --version 2>&1)"
if command -v tmux >/dev/null 2>&1; then
  echo "[mux-check] tmux: $(tmux -V 2>&1)"
else
  echo "[mux-check] tmux: NOT INSTALLED"
fi
if command -v zellij >/dev/null 2>&1; then
  echo "[mux-check] zellij: $(zellij --version 2>&1)"
else
  echo "[mux-check] zellij: NOT INSTALLED"
fi

echo "[mux-check] running base smoke"
zsh smoke_test/python_tui_smoke.sh

echo "[mux-check] done"
echo "[mux-check] next: run manual checklist in docs/tui/python-tui-regression-checklist.md"
