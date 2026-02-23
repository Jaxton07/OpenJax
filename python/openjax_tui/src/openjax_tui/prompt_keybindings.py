from __future__ import annotations

from typing import Any, Callable


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
