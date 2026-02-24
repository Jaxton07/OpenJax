from __future__ import annotations

import asyncio
import contextlib
import os
import shutil
import signal
import sys
import traceback
from collections.abc import Callable
from typing import Any

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxProtocolError, OpenJaxResponseError
from openjax_sdk.models import EventEnvelope

from .approval import (
    approval_mode_active as _approval_mode_active,
    approval_toolbar_text as _approval_toolbar_text,
    focused_approval_id as _focused_approval_id,
    is_expired_approval_error as _is_expired_approval_error,
    pop_pending as _pop_pending,
    print_pending as _print_pending,
    resolve_approval_by_id as _resolve_approval_by_id,
    resolve_latest_approval as _resolve_latest_approval,
    toggle_approval_selection as _toggle_approval_selection,
    use_inline_approval_panel as _use_inline_approval_panel,
)
from .event_dispatch import print_event as _print_event
from .debug_utils import format_event_debug_line as _format_event_debug_line
from .event_handlers import (
    create_finalize_stream_line_if_turn_fn as _create_finalize_stream_line_if_turn_fn,
    create_print_tool_call_result_line_fn as _create_print_tool_call_result_line_fn,
    create_print_tool_summary_for_turn_fn as _create_print_tool_summary_for_turn_fn,
    create_record_tool_completed_fn as _create_record_tool_completed_fn,
    create_record_tool_started_fn as _create_record_tool_started_fn,
    create_render_assistant_delta_fn as _create_render_assistant_delta_fn,
    create_render_assistant_message_fn as _create_render_assistant_message_fn,
)
from .event_state_manager import EventStateCallbacks, EventStateManager
from .input_backend import configure_readline_keybindings as _configure_readline_keybindings
from .input_backend import normalize_input as _normalize_input
from .input_backend import select_input_backend_with_reason as _select_input_backend_with_reason
from .input_loops import (
    INPUT_REQUEST as _INPUT_REQUEST,
    INPUT_STOP as _INPUT_STOP,
    USER_PROMPT_PREFIX as _USER_PROMPT_PREFIX,
    InputLoopCallbacks,
    daemon_cmd_from_env as _daemon_cmd_from_env_impl,
    handle_user_line as _handle_user_line_impl,
    run_input_loop_basic as _run_input_loop_basic_impl,
)
from .prompt_runtime_loop import PromptToolkitComponents
from .prompt_runtime_loop import fallback_prompt_toolkit_to_basic as _fallback_prompt_toolkit_to_basic_impl
from .prompt_runtime_loop import run_prompt_toolkit_loop as _run_prompt_toolkit_loop
from .prompt_ui import build_prompt_key_bindings as _build_prompt_key_bindings_impl
from .prompt_ui import drain_background_task as _drain_background_task
from .prompt_ui import refresh_history_view as _refresh_history_view
from .prompt_ui import request_prompt_redraw as _request_prompt_redraw
from .session_logging import append_openjax_log_line as _append_openjax_log_line
from .session_logging import approval_text_field as _approval_text_field
from .session_logging import log_startup_summary as _log_startup_summary
from .session_logging import tui_log_approval_event as _tui_log_approval_event
from .startup_ui import _print_logo, _print_startup_card, _resolve_openjax_version
from .state import AppState, ApprovalRecord, ViewMode
from .status_animation import STATUS_ANIMATION_INTERVAL_S as _STATUS_ANIMATION_INTERVAL_S
from .status_animation import THINKING_STATUS_FRAMES as _THINKING_STATUS_FRAMES
from .status_animation import TOOL_WAIT_STATUS_FRAMES as _TOOL_WAIT_STATUS_FRAMES
from .status_animation import run_animation_ticker as _run_status_animation_ticker
from .status_animation import start_animation as _start_status_animation
from .status_animation import stop_animation as _stop_status_animation
from .status_animation import sync_animation_controller as _sync_status_animation_controller
from .tui_logging import _setup_tui_logger, _tui_debug, _tui_log_info
from .viewport_adapter import PilotHistoryViewportAdapter, TextAreaHistoryViewportAdapter
from .assistant_render import emit_ui_line as _emit_ui_line
from .assistant_render import finalize_stream_line as _finalize_stream_line

try:
    from prompt_toolkit import PromptSession
    from prompt_toolkit.application import Application
    from prompt_toolkit.application.run_in_terminal import run_in_terminal
    from prompt_toolkit.completion import Completer, Completion
    from prompt_toolkit.document import Document
    from prompt_toolkit.filters import Condition
    from prompt_toolkit.layout import ConditionalContainer, HSplit, Layout, Window
    from prompt_toolkit.layout.controls import FormattedTextControl
    from prompt_toolkit.layout.dimension import Dimension
    from prompt_toolkit.patch_stdout import patch_stdout
    from prompt_toolkit.styles import Style
    from prompt_toolkit.widgets import TextArea

    _prompt_toolkit_run_in_terminal: Callable[..., Any] | None = run_in_terminal
    _prompt_toolkit_dimension: Any | None = Dimension
    _prompt_toolkit_style: type[Style] | None = Style
    _prompt_toolkit_application: Any | None = Application
    _prompt_toolkit_text_area: type[TextArea] | None = TextArea
    _prompt_toolkit_document: type[Document] | None = Document
    _prompt_toolkit_layout: type[Layout] | None = Layout
    _prompt_toolkit_hsplit: type[HSplit] | None = HSplit
    _prompt_toolkit_window: type[Window] | None = Window
    _prompt_toolkit_formatted_text_control: type[FormattedTextControl] | None = (
        FormattedTextControl
    )
    _prompt_toolkit_condition: type[Condition] | None = Condition
    _prompt_toolkit_conditional_container: type[ConditionalContainer] | None = (
        ConditionalContainer
    )
    _prompt_toolkit_completer: type[Completer] | None = Completer
    _prompt_toolkit_completion: type[Completion] | None = Completion
    _prompt_toolkit_import_error: str | None = None
except Exception:  # pragma: no cover - optional dependency fallback
    PromptSession = None  # type: ignore[assignment]
    patch_stdout = None  # type: ignore[assignment]
    _prompt_toolkit_run_in_terminal = None
    _prompt_toolkit_dimension = None
    _prompt_toolkit_style = None
    _prompt_toolkit_application = None
    _prompt_toolkit_text_area = None
    _prompt_toolkit_document = None
    _prompt_toolkit_layout = None
    _prompt_toolkit_hsplit = None
    _prompt_toolkit_window = None
    _prompt_toolkit_formatted_text_control = None
    _prompt_toolkit_condition = None
    _prompt_toolkit_conditional_container = None
    _prompt_toolkit_completer = None
    _prompt_toolkit_completion = None
    _prompt_toolkit_import_error = "prompt_toolkit import failed"

try:
    from prompt_toolkit.key_binding import KeyBindings
except Exception:  # pragma: no cover - optional dependency fallback
    KeyBindings = None  # type: ignore[assignment]

_OPENJAX_ROOT_LOG = os.path.join(".openjax", "logs", "openjax.log")
_PRINT_TOOL_TURN_SUMMARY = False
_COMMAND_ROWS: tuple[str, ...] = (
    "text                submit turn",
    "/approve <id> y|n   resolve a specific approval",
    "y | n               resolve latest pending approval",
    "/pending            show pending approvals",
    "/help               show help",
    "/exit               exit",
)
_SLASH_COMMANDS: tuple[str, ...] = ("/approve", "/pending", "/help", "/exit")


def _prompt_toolkit_components() -> PromptToolkitComponents:
    return PromptToolkitComponents(
        prompt_session_cls=PromptSession,
        patch_stdout=patch_stdout,
        application_cls=_prompt_toolkit_application,
        text_area_cls=_prompt_toolkit_text_area,
        document_cls=_prompt_toolkit_document,
        layout_cls=_prompt_toolkit_layout,
        hsplit_cls=_prompt_toolkit_hsplit,
        window_cls=_prompt_toolkit_window,
        formatted_text_control_cls=_prompt_toolkit_formatted_text_control,
        condition_cls=_prompt_toolkit_condition,
        conditional_container_cls=_prompt_toolkit_conditional_container,
        dimension_cls=_prompt_toolkit_dimension,
        completer_cls=_prompt_toolkit_completer,
        completion_cls=_prompt_toolkit_completion,
        run_in_terminal_fn=_prompt_toolkit_run_in_terminal,
    )


async def _resolve_approval_by_id_wrapped(
    client: OpenJaxAsyncClient,
    state: AppState,
    approval_request_id: str,
    approved: bool,
) -> None:
    await _resolve_approval_by_id(
        client=client,
        state=state,
        approval_request_id=approval_request_id,
        approved=approved,
        use_inline_approval_panel_fn=_use_inline_approval_panel,
        pop_pending_fn=_pop_pending,
        is_expired_approval_error_fn=_is_expired_approval_error,
        log_approval_event_fn=lambda **kwargs: _tui_log_approval_event(_tui_log_info, **kwargs),
    )


def _create_input_loop_callbacks() -> InputLoopCallbacks:
    return InputLoopCallbacks(
        approval_mode_active=_approval_mode_active,
        focused_approval_id=_focused_approval_id,
        resolve_approval_by_id=_resolve_approval_by_id_wrapped,
        resolve_latest_approval=_resolve_latest_approval,
        use_inline_approval_panel=_use_inline_approval_panel,
        emit_ui_line=lambda state, text: _emit_ui_line(
            state,
            text,
            refresh_history_view_fn=_refresh_history_view,
        ),
        print_pending=_print_pending,
        tui_log_approval_event=lambda **kwargs: _tui_log_approval_event(_tui_log_info, **kwargs),
        sync_status_animation_controller=lambda state: _sync_status_animation_controller(
            state,
            request_prompt_redraw_fn=lambda: _request_prompt_redraw(state, tui_debug_fn=_tui_debug),
        ),
    )


async def _run_input_loop_basic(client: OpenJaxAsyncClient, state: AppState) -> None:
    callbacks = _create_input_loop_callbacks()
    await _run_input_loop_basic_impl(
        client,
        state,
        callbacks,
        handle_user_line_fn=lambda c, s, line: _handle_user_line_impl(c, s, line, callbacks, command_rows=_COMMAND_ROWS, slash_commands=_SLASH_COMMANDS),
        input_request=_INPUT_REQUEST,
        input_stop=_INPUT_STOP,
        user_prompt_prefix=_USER_PROMPT_PREFIX,
        active_state_getter=lambda: _active_state,
    )


async def _input_loop_basic(client: OpenJaxAsyncClient, state: AppState) -> None:
    await _run_input_loop_basic(client, state)


async def _handle_user_line(client: OpenJaxAsyncClient, state: AppState, line: str) -> bool:
    callbacks = _create_input_loop_callbacks()
    return await _handle_user_line_impl(
        client,
        state,
        line,
        callbacks,
        command_rows=_COMMAND_ROWS,
        slash_commands=_SLASH_COMMANDS,
    )


async def _fallback_prompt_toolkit_to_basic(
    client: OpenJaxAsyncClient,
    state: AppState,
    *,
    reason: str,
) -> None:
    await _fallback_prompt_toolkit_to_basic_impl(
        client,
        state,
        reason=reason,
        run_input_loop_basic_fn=_run_input_loop_basic,
        request_prompt_redraw_fn=lambda s: _request_prompt_redraw(s, tui_debug_fn=_tui_debug),
        finalize_stream_line_fn=_finalize_stream_line,
    )


async def _input_loop_prompt_toolkit(client: OpenJaxAsyncClient, state: AppState) -> None:
    key_bindings = _build_prompt_key_bindings(client, state)
    prompt_style = _build_prompt_style()
    await _run_prompt_toolkit_loop(
        client,
        state,
        components=_prompt_toolkit_components(),
        key_bindings=key_bindings,
        prompt_style=prompt_style,
        slash_commands=_SLASH_COMMANDS,
        user_prompt_prefix=_USER_PROMPT_PREFIX,
        divider_line_fn=_divider_line,
        handle_user_line_fn=_handle_user_line,
        fallback_to_basic_fn=lambda c, s, reason: _fallback_prompt_toolkit_to_basic(c, s, reason=reason),
        request_prompt_redraw_fn=lambda s: _request_prompt_redraw(s, tui_debug_fn=_tui_debug),
        drain_background_task_fn=_drain_background_task,
        tui_log_info_fn=_tui_log_info,
        tui_debug_fn=_tui_debug,
    )


async def run() -> None:
    _setup_tui_logger()
    input_backend, backend_reason = _select_input_backend_with_reason(
        prompt_session=PromptSession,
        patch_stdout=patch_stdout,
        key_bindings=KeyBindings,
        prompt_toolkit_import_error=_prompt_toolkit_import_error,
        stdin_is_tty=sys.stdin.isatty(),
        stdout_is_tty=sys.stdout.isatty(),
    )
    if input_backend == "basic":
        _configure_readline_keybindings()

    client = OpenJaxAsyncClient(daemon_cmd=_daemon_cmd_from_env_impl())
    state = AppState()
    state.set_view_mode(os.environ.get("OPENJAX_TUI_VIEW_MODE"))
    state.input_ready = asyncio.Event()
    state.input_ready.set()
    state.approval_interrupt = asyncio.Event()
    _set_active_state(state)

    await client.start()
    interrupted = False
    try:
        session_id = await client.start_session()
        version = _resolve_openjax_version()
        state.session_id = session_id
        state.input_backend = input_backend
        state.input_backend_reason = backend_reason
        await client.stream_events()
        _print_logo()
        _print_startup_card(version=version)
        _log_startup_summary(
            state,
            version=version,
            log_info_fn=_tui_log_info,
            append_openjax_log_line_fn=lambda msg: _append_openjax_log_line(msg, _OPENJAX_ROOT_LOG),
            approval_text_field_fn=_approval_text_field,
        )

        event_task = asyncio.create_task(_event_loop(client, state))
        try:
            if input_backend == "prompt_toolkit":
                await _input_loop_prompt_toolkit(client, state)
            else:
                await _input_loop_basic(client, state)
        except (KeyboardInterrupt, asyncio.CancelledError):
            interrupted = True
            if state.running:
                print("^C")
            state.running = False
            _ignore_sigint_during_shutdown()
        except Exception as err:
            interrupted = True
            state.running = False
            _tui_log_info(f"python_tui fatal_error type={type(err).__name__} message={err}")
            _tui_debug("python_tui fatal_traceback\n" + traceback.format_exc())
            print(f"[error] python_tui crashed: {err}")
            _ignore_sigint_during_shutdown()
        finally:
            state.running = False
            event_task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await event_task
    finally:
        if interrupted:
            _ignore_sigint_during_shutdown()
        _stop_status_animation(
            state,
            request_prompt_redraw_fn=lambda: _request_prompt_redraw(state, tui_debug_fn=_tui_debug),
        )
        await _shutdown_client_quietly(client, graceful=not interrupted)
        _finalize_stream_line(state)
        _set_active_state(None)
        _tui_log_info("python_tui exited")
        print("openjax_tui exited")


async def _shutdown_client_quietly(client: OpenJaxAsyncClient, graceful: bool = True) -> None:
    with contextlib.suppress(
        OpenJaxProtocolError,
        OpenJaxResponseError,
        ConnectionError,
        BrokenPipeError,
        RuntimeError,
        asyncio.CancelledError,
        TimeoutError,
    ):
        if graceful and client.session_id:
            await asyncio.wait_for(client.shutdown_session(), timeout=1.0)
    with contextlib.suppress(
        OpenJaxProtocolError,
        OpenJaxResponseError,
        ConnectionError,
        BrokenPipeError,
        RuntimeError,
        asyncio.CancelledError,
        TimeoutError,
    ):
        await client.stop()


def _ignore_sigint_during_shutdown() -> None:
    with contextlib.suppress(Exception):
        signal.signal(signal.SIGINT, signal.SIG_IGN)


async def _event_loop(client: OpenJaxAsyncClient, state: AppState) -> None:
    while state.running:
        try:
            evt = await client.next_event(timeout=0.5)
        except TimeoutError:
            continue
        except OpenJaxProtocolError as err:
            _finalize_stream_line(state)
            print(f"[error] event stream closed: {err}")
            state.running = False
            return
        _dispatch_event(evt, state)
        _apply_event_state_updates(state, evt)


def _dispatch_event(evt: EventEnvelope, state: AppState) -> None:
    if evt.event_type == "assistant_message" and evt.turn_id is not None:
        streamed = state.stream_text_by_turn.get(evt.turn_id, "")
        content = str(evt.payload.get("content", ""))
        if streamed and streamed != content:
            _tui_debug(
                "assistant content mismatch turn_id={turn} streamed_len={streamed_len} content_len={content_len}".format(
                    turn=evt.turn_id,
                    streamed_len=len(streamed),
                    content_len=len(content),
                )
            )

    _print_event(
        evt,
        state=state,
        print_tool_turn_summary=_PRINT_TOOL_TURN_SUMMARY,
        render_assistant_delta_fn=_create_render_assistant_delta_fn(state),
        render_assistant_message_fn=_create_render_assistant_message_fn(state),
        finalize_stream_line_if_turn_fn=_create_finalize_stream_line_if_turn_fn(state),
        record_tool_started_fn=_create_record_tool_started_fn(state),
        record_tool_completed_fn=_create_record_tool_completed_fn(state),
        print_tool_call_result_line_fn=_create_print_tool_call_result_line_fn(state),
        use_inline_approval_panel_fn=_use_inline_approval_panel,
        print_tool_summary_for_turn_fn=_create_print_tool_summary_for_turn_fn(state),
    )


def _build_event_state_callbacks(state: AppState) -> EventStateCallbacks:
    def _sync_animation() -> None:
        _sync_status_animation_controller(
            state,
            request_prompt_redraw_fn=lambda: _request_prompt_redraw(state, tui_debug_fn=_tui_debug),
        )

    def _request_redraw() -> None:
        _request_prompt_redraw(state, tui_debug_fn=_tui_debug)

    def _log_approval_event(**kwargs: Any) -> None:
        _tui_log_approval_event(_tui_log_info, **kwargs)

    def _pop_pending_by_request_id(request_id: str) -> None:
        _pop_pending(state, request_id)

    def _is_live_viewport_mode() -> bool:
        return state.view_mode == ViewMode.LIVE_VIEWPORT

    return EventStateCallbacks(
        sync_animation=_sync_animation,
        request_redraw=_request_redraw,
        log_approval_event=_log_approval_event,
        pop_pending=_pop_pending_by_request_id,
        use_inline_approval_panel=_use_inline_approval_panel,
        debug_log=_tui_debug,
        is_live_viewport_mode=_is_live_viewport_mode,
    )


def _apply_event_state_updates(state: AppState, evt: EventEnvelope) -> None:
    _tui_debug(_format_event_debug_line(evt))
    manager = EventStateManager(state, _build_event_state_callbacks(state))
    manager.apply_event_updates(evt)


def _daemon_cmd_from_env() -> list[str]:
    return _daemon_cmd_from_env_impl()


def _print_help() -> None:
    print("commands:")
    for row in _COMMAND_ROWS:
        print(f"  {row}")


def _print_status_bar(state: AppState) -> None:
    line = (
        f"[status] session={state.session_id or '-'}  backend={state.input_backend}  "
        f"phase={state.turn_phase}  approvals={len(state.pending_approvals)}"
    )
    print(line)
    if state.input_backend_reason:
        print(f"[input] {state.input_backend_reason}")
    print(_divider_line())


def _divider_line() -> str:
    columns = shutil.get_terminal_size(fallback=(100, 24)).columns
    return "─" * max(min(columns, 100), 24)


def _build_prompt_style() -> Any:
    if _prompt_toolkit_style is None:
        return None
    return _prompt_toolkit_style.from_dict(
        {
            "bottom-toolbar": "noreverse bg:default fg:default",
            "bottom-toolbar.text-area": "noreverse bg:default fg:default",
        }
    )


def _build_prompt_key_bindings(client: OpenJaxAsyncClient, state: AppState) -> Any:
    _ = client

    def _on_tab_non_approval(event: object) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        text = str(getattr(current_buffer, "text", "")).strip()
        if current_buffer is not None and text.startswith("/"):
            start_completion = getattr(current_buffer, "start_completion", None)
            if callable(start_completion):
                start_completion(select_first=False)

    return _build_prompt_key_bindings_impl(
        key_bindings_cls=KeyBindings,
        state=state,
        approval_mode_active_fn=_approval_mode_active,
        toggle_approval_selection_fn=_toggle_approval_selection,
        on_tab_non_approval_fn=_on_tab_non_approval,
    )


_active_state: AppState | None = None


def _set_active_state(state: AppState | None) -> None:
    global _active_state
    _active_state = state
