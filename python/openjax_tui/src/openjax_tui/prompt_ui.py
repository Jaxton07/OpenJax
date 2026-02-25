from __future__ import annotations

import asyncio
import contextlib
import time
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

    def _set_approval_flash(message: str) -> None:
        state.approval_flash_message = message
        state.approval_flash_until = time.monotonic() + 1.6
        previous_handle = getattr(state, "approval_flash_clear_handle", None)
        if previous_handle is not None:
            with contextlib.suppress(Exception):
                previous_handle.cancel()

        def _clear_flash() -> None:
            if time.monotonic() < state.approval_flash_until:
                return
            state.approval_flash_message = ""
            state.approval_flash_until = 0.0
            state.approval_flash_clear_handle = None
            invalidator = getattr(state, "prompt_invalidator", None)
            if callable(invalidator):
                with contextlib.suppress(Exception):
                    invalidator()

        with contextlib.suppress(RuntimeError):
            loop = asyncio.get_running_loop()
            state.approval_flash_clear_handle = loop.call_later(1.7, _clear_flash)

        invalidator = getattr(state, "prompt_invalidator", None)
        if callable(invalidator):
            with contextlib.suppress(Exception):
                invalidator()

    def _submit_approval_without_losing_input(event: object, flash_message: str) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        if current_buffer is None:
            return
        existing_text = str(getattr(current_buffer, "text", ""))
        current_buffer.text = ""
        validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
        if callable(validate_and_handle):
            validate_and_handle()
        current_buffer.text = existing_text
        _set_approval_flash(flash_message)

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
        if not approval_mode_active_fn(state):
            validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
            if callable(validate_and_handle):
                validate_and_handle()
            return
        flash_message = (
            "Approved" if state.approval_selected_action == "allow" else "Rejected"
        )
        _submit_approval_without_losing_input(event, flash_message)

    def _insert_newline(event: object) -> None:
        if approval_mode_active_fn(state):
            return
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        insert_text = getattr(current_buffer, "insert_text", None)
        if callable(insert_text):
            insert_text("\n")

    for key in ("s-enter", "c-j"):
        with contextlib.suppress(Exception):
            kb.add(key, eager=True)(_insert_newline)

    @kb.add("escape", eager=True)
    def _escape_reject(event: object) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        if not approval_mode_active_fn(state):
            return
        if current_buffer is None:
            return
        state.approval_selected_action = "deny"
        _submit_approval_without_losing_input(event, "Rejected")

    return kb
