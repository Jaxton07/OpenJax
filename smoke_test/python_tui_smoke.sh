#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

export PYTHONPATH="python/openjax_sdk/src:python/openjax_tui/src"

if [[ -x "target/debug/openjaxd" ]]; then
  export OPENJAX_DAEMON_CMD="target/debug/openjaxd"
fi

run_tool_turn_case() {
  local case_name="$1"
  local view_mode="$2"
  local viewport_impl="$3"
  local log_file="$4"
  local tui_log_dir="/tmp/openjax_tui_smoke_${case_name}_logs"

  echo "[smoke] ${case_name}: tool turn view_mode=${view_mode} viewport=${viewport_impl}"
  python3 - "$log_file" "$view_mode" "$viewport_impl" "$tui_log_dir" <<'PY'
from __future__ import annotations

import os
import pty
import select
import subprocess
import sys
import time
import contextlib
from pathlib import Path


def _run_case(log_file: str, view_mode: str, viewport_impl: str, tui_log_dir: str) -> int:
    env = os.environ.copy()
    env["OPENJAX_TUI_INPUT_BACKEND"] = "prompt_toolkit"
    env["OPENJAX_TUI_VIEW_MODE"] = view_mode
    env["OPENJAX_TUI_HISTORY_VIEWPORT_IMPL"] = viewport_impl
    env["OPENJAX_TUI_LOG_DIR"] = tui_log_dir
    Path(tui_log_dir).mkdir(parents=True, exist_ok=True)

    master_fd, slave_fd = pty.openpty()
    proc = subprocess.Popen(
        ["python3", "-m", "openjax_tui"],
        stdin=slave_fd,
        stdout=slave_fd,
        stderr=slave_fd,
        env=env,
        text=False,
        close_fds=True,
    )
    os.close(slave_fd)

    transcript = bytearray(
        f"SMOKE_CASE_CONFIG view_mode={view_mode} viewport_impl={viewport_impl}\n".encode(
            "utf-8"
        )
    )
    started = False
    sent_exit = False
    tool_success_token = "tool list_dir 执行成功".encode("utf-8")
    deadline = time.monotonic() + 45.0

    while time.monotonic() < deadline:
        ready, _, _ = select.select([master_fd], [], [], 0.2)
        if ready:
            try:
                chunk = os.read(master_fd, 8192)
            except OSError:
                chunk = b""
            if chunk:
                transcript.extend(chunk)
            elif proc.poll() is not None:
                break

        if not started and b">_ OpenJax" in transcript:
            os.write(master_fd, b"tool:list_dir dir_path=.\r")
            started = True

        if started and not sent_exit and tool_success_token in transcript:
            os.write(master_fd, b"/exit\r")
            sent_exit = True

        if sent_exit and proc.poll() is not None:
            break

    if proc.poll() is None:
        if not sent_exit:
            with contextlib.suppress(OSError):
                os.write(master_fd, b"/exit\r")
        with contextlib.suppress(Exception):
            proc.wait(timeout=5)
    if proc.poll() is None:
        proc.terminate()
        with contextlib.suppress(Exception):
            proc.wait(timeout=5)
    if proc.poll() is None:
        proc.kill()
        with contextlib.suppress(Exception):
            proc.wait(timeout=5)

    os.close(master_fd)
    Path(log_file).write_text(transcript.decode("utf-8", errors="replace"), encoding="utf-8")
    if not started:
        return 3
    if not sent_exit:
        return 4
    return int(proc.returncode or 0)


if __name__ == "__main__":
    exit_code = _run_case(*sys.argv[1:])
    raise SystemExit(exit_code)
PY

  grep -Fq "SMOKE_CASE_CONFIG view_mode=${view_mode} viewport_impl=${viewport_impl}" "$log_file"
  grep -Fq "Read directory [started]" "$log_file"
  grep -Fq "Read directory [running]" "$log_file"
  grep -Fq "Read directory [completed]" "$log_file"
  grep -Fq "tool list_dir 执行成功" "$log_file"
  grep -Fq "openjax_tui exited" "$log_file"
  grep -Fq "backend=prompt_toolkit" "$tui_log_dir/openjax_tui.log"
}

echo "[smoke] case1: help + exit"
python3 -m openjax_tui <<'EOF' >/tmp/openjax_tui_smoke_case1.log
/help
/exit
EOF

grep -Fq "OpenJax TUI" /tmp/openjax_tui_smoke_case1.log
grep -Fq "commands:" /tmp/openjax_tui_smoke_case1.log
grep -Fq "openjax_tui exited" /tmp/openjax_tui_smoke_case1.log

run_tool_turn_case \
  "case2-pilot" \
  "live" \
  "pilot" \
  "/tmp/openjax_tui_smoke_case2_pilot.log"

run_tool_turn_case \
  "case3-textarea" \
  "live" \
  "textarea" \
  "/tmp/openjax_tui_smoke_case3_textarea.log"

echo "[smoke] openjax_tui smoke passed"
