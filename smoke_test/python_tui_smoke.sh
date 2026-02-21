#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

export PYTHONPATH="python/openjax_sdk/src:python/openjax_tui/src"

if [[ -x "target/debug/openjaxd" ]]; then
  export OPENJAX_DAEMON_CMD="target/debug/openjaxd"
fi

echo "[smoke] case1: help + exit"
python3 -m openjax_tui <<'EOF' >/tmp/openjax_tui_smoke_case1.log
/help
/exit
EOF

grep -q "OpenJax TUI" /tmp/openjax_tui_smoke_case1.log
grep -q "commands:" /tmp/openjax_tui_smoke_case1.log
grep -q "openjax_tui exited" /tmp/openjax_tui_smoke_case1.log

echo "[smoke] case2: submit tool turn"
python3 -m openjax_tui <<'EOF' >/tmp/openjax_tui_smoke_case2.log
tool:list_dir dir_path=.
/exit
EOF

grep -q "you>" /tmp/openjax_tui_smoke_case2.log
grep -q "thinking" /tmp/openjax_tui_smoke_case2.log

echo "[smoke] openjax_tui smoke passed"
