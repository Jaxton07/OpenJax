from __future__ import annotations

import asyncio
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


def build_prompt_key_bindings(
    *,
    key_bindings_cls: Any,
    state: Any,
    approval_mode_active_fn: Callable[[Any], bool],
    toggle_approval_selection_fn: Callable[[Any], None],
    on_tab_non_approval_fn: Callable[[Any], None],
) -> Any:
    if key_bindings_cls is None:
        return None
    kb = key_bindings_cls()

    @kb.add("tab", eager=True)
    def _toggle_action(event: object) -> None:
        if not approval_mode_active_fn(state):
            on_tab_non_approval_fn(event)
            return
        toggle_approval_selection_fn(state)
        app = getattr(event, "app", None)
        if app is not None:
            app.invalidate()

    @kb.add("up", eager=True)
    def _toggle_action_up(event: object) -> None:
        app = getattr(event, "app", None)
        if not approval_mode_active_fn(state):
            return
        toggle_approval_selection_fn(state)
        if app is not None:
            app.invalidate()

    @kb.add("down", eager=True)
    def _toggle_action_down(event: object) -> None:
        app = getattr(event, "app", None)
        if not approval_mode_active_fn(state):
            return
        toggle_approval_selection_fn(state)
        if app is not None:
            app.invalidate()

    @kb.add("enter", eager=True)
    def _enter_resolve(event: object) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        current_text = getattr(current_buffer, "text", "")
        if not (approval_mode_active_fn(state) and not str(current_text).strip()):
            validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
            if callable(validate_and_handle):
                validate_and_handle()
            return
        if current_buffer is None:
            return
        current_buffer.text = ""
        validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
        if callable(validate_and_handle):
            validate_and_handle()

    @kb.add("escape", eager=True)
    def _escape_reject(event: object) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        current_text = getattr(current_buffer, "text", "")
        if not (approval_mode_active_fn(state) and not str(current_text).strip()):
            return
        if current_buffer is None:
            return
        state.approval_selected_action = "deny"
        current_buffer.text = ""
        validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
        if callable(validate_and_handle):
            validate_and_handle()

    @kb.add("y", eager=True)
    def _quick_yes(event: object) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        current_text = getattr(current_buffer, "text", "")
        if not (approval_mode_active_fn(state) and not str(current_text).strip()):
            return
        if current_buffer is None:
            return
        state.approval_selected_action = "allow"
        current_buffer.text = ""
        validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
        if callable(validate_and_handle):
            validate_and_handle()

    @kb.add("n", eager=True)
    def _quick_no(event: object) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        current_text = getattr(current_buffer, "text", "")
        if not (approval_mode_active_fn(state) and not str(current_text).strip()):
            return
        if current_buffer is None:
            return
        state.approval_selected_action = "deny"
        current_buffer.text = ""
        validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
        if callable(validate_and_handle):
            validate_and_handle()

    return kb
