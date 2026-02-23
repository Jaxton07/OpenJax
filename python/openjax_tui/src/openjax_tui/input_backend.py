from __future__ import annotations

import contextlib
import os
import queue
import re
import threading
from typing import Any, Callable


def select_input_backend_with_reason(
    *,
    prompt_session: Any,
    patch_stdout: Any,
    key_bindings: Any,
    prompt_toolkit_import_error: str | None,
    stdin_is_tty: bool,
    stdout_is_tty: bool,
) -> tuple[str, str]:
    env_backend = os.environ.get("OPENJAX_TUI_INPUT_BACKEND", "").lower()
    if env_backend == "basic":
        return "basic", "forced by OPENJAX_TUI_INPUT_BACKEND=basic"
    if env_backend == "prompt_toolkit":
        if prompt_session is not None and patch_stdout is not None:
            if key_bindings is None:
                return "prompt_toolkit", "forced by env; key bindings unavailable"
            return "prompt_toolkit", "forced by OPENJAX_TUI_INPUT_BACKEND=prompt_toolkit"
        return "basic", "env requested prompt_toolkit but dependency unavailable"
    if (
        prompt_session is not None
        and patch_stdout is not None
        and stdin_is_tty
        and stdout_is_tty
    ):
        if key_bindings is None:
            return "prompt_toolkit", "tty mode; key bindings unavailable"
        return "prompt_toolkit", "tty mode"

    reason_parts: list[str] = []
    if prompt_session is None or patch_stdout is None:
        reason_parts.append(prompt_toolkit_import_error or "prompt_toolkit unavailable")
    if not stdin_is_tty or not stdout_is_tty:
        reason_parts.append("stdin/stdout is not tty")
    if not reason_parts:
        reason_parts.append("fallback to basic")
    return "basic", "; ".join(reason_parts)


def start_basic_input_worker(
    loop: Any,
    request_queue: queue.Queue[object],
    line_queue: Any,
    *,
    input_request: object,
    input_stop: object,
    user_prompt_prefix: str,
    active_state_getter: Callable[[], Any],
    approval_mode_active: Callable[[Any], bool],
) -> None:
    def worker() -> None:
        while True:
            cmd = request_queue.get()
            if cmd is input_stop:
                return
            if cmd is not input_request:
                continue
            try:
                prompt_prefix = user_prompt_prefix
                active_state = active_state_getter()
                if active_state is not None and approval_mode_active(active_state):
                    prompt_prefix = "approval>"
                line = input(f"{prompt_prefix} ")
            except EOFError:
                line = None
            except KeyboardInterrupt:
                line = None

            with contextlib.suppress(RuntimeError):
                loop.call_soon_threadsafe(line_queue.put_nowait, line)
            if line is None:
                return

    threading.Thread(
        target=worker,
        name="openjax-tui-basic-input",
        daemon=True,
    ).start()


def normalize_input(text: str) -> str:
    # Strip common CSI/SS3 ANSI sequences (arrow keys, function keys).
    text = re.sub(r"\x1B\[[0-?]*[ -/]*[@-~]", "", text)
    text = re.sub(r"\x1BO[A-Za-z]", "", text)
    # Apply backspace/delete semantics if control chars were captured.
    out: list[str] = []
    for ch in text:
        if ch in ("\x08", "\x7f"):
            if out:
                out.pop()
            continue
        if ch.isprintable() or ch in ("\t", " "):
            out.append(ch)
    return "".join(out)


def configure_readline_keybindings() -> None:
    # macOS Python often uses libedit/readline; in tmux/zellij arrow keys may emit
    # CSI/SS3 sequences that are not bound by default, leading to "^[[D" artifacts.
    try:
        import readline  # type: ignore
    except Exception:
        return

    bindings = [
        r'"\e[A": previous-history',
        r'"\e[B": next-history',
        r'"\e[C": forward-char',
        r'"\e[D": backward-char',
        r'"\eOA": previous-history',
        r'"\eOB": next-history',
        r'"\eOC": forward-char',
        r'"\eOD": backward-char',
    ]
    for binding in bindings:
        with contextlib.suppress(Exception):
            readline.parse_and_bind(binding)
