from __future__ import annotations

import contextlib
from typing import Any, Callable


def invalidate_prompt_application(app: Any) -> None:
    if app is None:
        return
    invalidate = getattr(app, "invalidate", None)
    if callable(invalidate):
        invalidate()


def request_prompt_redraw(
    state: Any,
    *,
    tui_debug_fn: Callable[[str], None],
) -> None:
    invalidator = state.prompt_invalidator
    if invalidator is None:
        tui_debug_fn("prompt redraw skipped: no invalidator")
        return
    tui_debug_fn("prompt redraw requested")
    with contextlib.suppress(Exception):
        invalidator()


def history_text(state: Any) -> str:
    if not state.history_blocks:
        return "\n"
    return "\n" + "\n\n".join(state.history_blocks)


def refresh_history_view(state: Any) -> None:
    setter = state.history_setter
    if setter is None:
        return
    with contextlib.suppress(Exception):
        setter()


async def drain_background_task(task: Any) -> None:
    if task is None:
        return
    if not task.done():
        task.cancel()
    with contextlib.suppress(BaseException):
        await task
