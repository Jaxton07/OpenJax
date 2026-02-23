from __future__ import annotations

import contextlib
import os
import time
from typing import Any, Callable


def approval_bool_field(value: bool | None) -> str:
    if value is None:
        return "-"
    return "true" if value else "false"


def approval_text_field(value: str | None) -> str:
    text = (value or "-").strip()
    if not text:
        return "-"
    return "_".join(text.split())


def tui_log_approval_event(
    log_info_fn: Callable[[str], None],
    action: str,
    request_id: str | None = None,
    turn_id: str | None = None,
    target: str | None = None,
    approved: bool | None = None,
    resolved: bool | None = None,
    detail: str | None = None,
) -> None:
    log_info_fn(
        "approval_event "
        f"action={approval_text_field(action)} "
        f"request_id={approval_text_field(request_id)} "
        f"turn_id={approval_text_field(turn_id)} "
        f"target={approval_text_field(target)} "
        f"approved={approval_bool_field(approved)} "
        f"resolved={approval_bool_field(resolved)} "
        f"detail={approval_text_field(detail)}"
    )


def append_openjax_log_line(message: str, log_path: str) -> None:
    timestamp = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    with contextlib.suppress(OSError):
        os.makedirs(os.path.dirname(log_path), exist_ok=True)
        with open(log_path, "a", encoding="utf-8") as fh:
            fh.write(f"{timestamp} INFO python_tui {message}\n")


def log_startup_summary(
    state: Any,
    version: str,
    *,
    log_info_fn: Callable[[str], None],
    append_openjax_log_line_fn: Callable[[str], None],
    approval_text_field_fn: Callable[[str | None], str],
) -> None:
    summary = (
        "python_tui started "
        f"version={version} "
        f"cwd={os.getcwd()} "
        f"session={state.session_id or '-'} "
        f"backend={state.input_backend} "
        f"phase={state.turn_phase} "
        f"approvals={len(state.pending_approvals)} "
        f"input_reason={approval_text_field_fn(state.input_backend_reason)}"
    )
    log_info_fn(summary)
    append_openjax_log_line_fn(summary)
