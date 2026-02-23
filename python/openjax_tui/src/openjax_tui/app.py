from __future__ import annotations

import asyncio
import contextlib
import os
import queue
import re
import shutil
import signal
import sys
import time
from typing import Any, Callable

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxProtocolError, OpenJaxResponseError
from openjax_sdk.models import EventEnvelope
from .state import AppState, ApprovalRecord, ToolTurnStats
from .startup_ui import (
    _OPENJAX_LOGO_LONG,
    _OPENJAX_LOGO_SHORT,
    _OPENJAX_LOGO_TINY,
    _format_display_directory,
    _normalize_logo_block,
    _print_logo,
    _print_startup_card,
    _resolve_openjax_version,
    _select_logo,
    _supports_ansi_color,
    _text_block_width,
)
from .slash_commands import (
    build_slash_command_completer as _build_slash_command_completer,
    slash_command_candidates as _slash_command_candidates,
    slash_hint_fragments as _slash_hint_fragments,
    slash_hint_text as _slash_hint_text,
)
from .approval import (
    approval_mode_active as _approval_mode_active,
    approval_toolbar_fragments as _approval_toolbar_fragments,
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
from .input_backend import (
    configure_readline_keybindings as _configure_readline_keybindings,
    normalize_input as _normalize_input,
    select_input_backend_with_reason as _select_input_backend_with_reason,
    start_basic_input_worker as _start_basic_input_worker,
)
from .tui_logging import (
    _reset_tui_logger_for_tests,
    _setup_tui_logger,
    _tui_debug,
    _tui_log_info,
)
from .session_logging import (
    append_openjax_log_line as _append_openjax_log_line,
    approval_bool_field as _approval_bool_field,
    approval_text_field as _approval_text_field,
    log_startup_summary as _log_startup_summary,
    tui_log_approval_event as _tui_log_approval_event,
)
from .event_dispatch import print_event as _print_event
from .prompt_ui import (
    build_prompt_key_bindings as _build_prompt_key_bindings_impl,
    drain_background_task as _drain_background_task,
    history_text as _history_text,
    invalidate_prompt_application as _invalidate_prompt_application,
    refresh_history_view as _refresh_history_view,
    request_prompt_redraw as _request_prompt_redraw,
)
from .tool_runtime import (
    emit_ui_spacer as _emit_ui_spacer,
    print_tool_call_result_line as _print_tool_call_result_line,
    print_tool_summary_for_turn as _print_tool_summary_for_turn,
    record_tool_completed as _record_tool_completed,
    record_tool_started as _record_tool_started,
    status_bullet as _status_bullet,
)
from .assistant_render import (
    align_multiline as _align_multiline,
    emit_ui_line as _emit_ui_line,
    finalize_stream_line as _finalize_stream_line,
    finalize_stream_line_if_turn as _finalize_stream_line_if_turn,
    print_prefixed_block as _print_prefixed_block,
    render_assistant_delta as _render_assistant_delta,
    render_assistant_message as _render_assistant_message,
    tool_result_label as _tool_result_label,
)

try:
    from prompt_toolkit import PromptSession, print_formatted_text
    from prompt_toolkit.application import Application
    from prompt_toolkit.document import Document
    from prompt_toolkit.filters import Condition
    from prompt_toolkit.formatted_text import ANSI
    from prompt_toolkit.layout import ConditionalContainer, HSplit, Layout, Window
    from prompt_toolkit.layout.controls import FormattedTextControl
    from prompt_toolkit.patch_stdout import patch_stdout
    from prompt_toolkit.styles import Style
    from prompt_toolkit.widgets import TextArea
    from prompt_toolkit.completion import Completer, Completion
    _prompt_toolkit_print: Callable[[object], None] | None = print_formatted_text
    _prompt_toolkit_ansi: Callable[[str], object] | None = ANSI
    _prompt_toolkit_style: type[Style] | None = Style
    _prompt_toolkit_application: type[Application] | None = Application
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
    _prompt_toolkit_print = None
    _prompt_toolkit_ansi = None
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
    _key_bindings_import_error: str | None = None
except Exception:  # pragma: no cover - optional dependency fallback
    KeyBindings = None  # type: ignore[assignment]
    _key_bindings_import_error = "key bindings import failed"

_INPUT_REQUEST = object()
_INPUT_STOP = object()
_USER_PROMPT_PREFIX = "❯"
_ASSISTANT_PREFIX = "⏺"
_PREFIX_CONTINUATION = "  "
_ANSI_GREEN = "\x1b[32m"
_ANSI_RED = "\x1b[31m"
_ANSI_RESET = "\x1b[0m"
_PRINT_TOOL_TURN_SUMMARY = False
_OPENJAX_ROOT_LOG = os.path.join(".openjax", "logs", "openjax.log")
_COMMAND_ROWS: tuple[str, ...] = (
    "text                submit turn",
    "/approve <id> y|n   resolve a specific approval",
    "y | n               resolve latest pending approval",
    "/pending            show pending approvals",
    "/help               show help",
    "/exit               exit",
)
_SLASH_COMMANDS: tuple[str, ...] = ("/approve", "/pending", "/help", "/exit")


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
    daemon_cmd = _daemon_cmd_from_env()
    client = OpenJaxAsyncClient(daemon_cmd=daemon_cmd)
    state = AppState()
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
        finally:
            state.running = False
            event_task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await event_task
    finally:
        if interrupted:
            _ignore_sigint_during_shutdown()
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


async def _input_loop_basic(client: OpenJaxAsyncClient, state: AppState) -> None:
    if state.input_ready is None:
        raise RuntimeError("input gate is not initialized")

    line_queue: asyncio.Queue[str | None] = asyncio.Queue()
    request_queue: queue.Queue[object] = queue.Queue()
    _start_basic_input_worker(asyncio.get_running_loop(), request_queue, line_queue, input_request=_INPUT_REQUEST, input_stop=_INPUT_STOP, user_prompt_prefix=_USER_PROMPT_PREFIX, active_state_getter=lambda: _active_state, approval_mode_active=_approval_mode_active)

    while state.running:
        try:
            await state.input_ready.wait()
            request_queue.put_nowait(_INPUT_REQUEST)
            line = await line_queue.get()
        except KeyboardInterrupt:
            state.running = False
            raise
        except asyncio.CancelledError:
            state.running = False
            return

        if line is None:
            state.running = False
            return
        if not await _handle_user_line(client, state, line):
            return


async def _input_loop_prompt_toolkit(client: OpenJaxAsyncClient, state: AppState) -> None:
    if state.input_ready is None:
        raise RuntimeError("input gate is not initialized")
    if (
        PromptSession is None
        or patch_stdout is None
        or _prompt_toolkit_application is None
        or _prompt_toolkit_text_area is None
        or _prompt_toolkit_document is None
        or _prompt_toolkit_layout is None
        or _prompt_toolkit_hsplit is None
        or _prompt_toolkit_window is None
        or _prompt_toolkit_formatted_text_control is None
        or _prompt_toolkit_condition is None
        or _prompt_toolkit_conditional_container is None
    ):
        await _input_loop_basic(client, state)
        return

    state.approval_ui_enabled = True
    key_bindings = _build_prompt_key_bindings(client, state)
    prompt_style = _build_prompt_style()
    line_queue: asyncio.Queue[str] = asyncio.Queue()
    loop = asyncio.get_running_loop()

    history_control = _prompt_toolkit_formatted_text_control(
        lambda: (
            _prompt_toolkit_ansi(_history_text(state))
            if _prompt_toolkit_ansi is not None
            else _history_text(state)
        ),
        focusable=False,
        show_cursor=False,
    )
    history_view = _prompt_toolkit_window(
        content=history_control,
        always_hide_cursor=True,
        wrap_lines=True,
    )

    def _accept_input(buffer: Any) -> bool:
        text = str(getattr(buffer, "text", ""))
        buffer.text = ""
        if state.input_ready is not None and not state.input_ready.is_set():
            return True
        loop.call_soon_threadsafe(line_queue.put_nowait, text)
        return True

    slash_completer = _build_slash_command_completer(_SLASH_COMMANDS, _prompt_toolkit_completer, _prompt_toolkit_completion)
    input_view = _prompt_toolkit_text_area(
        prompt=f"{_USER_PROMPT_PREFIX} ",
        multiline=False,
        wrap_lines=False,
        accept_handler=_accept_input,
        completer=slash_completer,
        complete_while_typing=True,
    )
    slash_hint_panel = _prompt_toolkit_conditional_container(
        content=_prompt_toolkit_window(
            content=_prompt_toolkit_formatted_text_control(
                lambda: _slash_hint_fragments(
                    str(getattr(input_view.buffer, "text", "")), _SLASH_COMMANDS
                )
            ),
            dont_extend_height=True,
        ),
        filter=_prompt_toolkit_condition(
            lambda: bool(_slash_hint_text(str(getattr(input_view.buffer, "text", "")), _SLASH_COMMANDS))
        ),
    )

    approval_panel = _prompt_toolkit_conditional_container(
        content=_prompt_toolkit_window(
            content=_prompt_toolkit_formatted_text_control(
                lambda: _approval_toolbar_fragments(state, _divider_line())
            ),
            dont_extend_height=True,
        ),
        filter=_prompt_toolkit_condition(lambda: bool(_approval_toolbar_text(state, _divider_line()))),
    )

    root_container = _prompt_toolkit_hsplit(
        [
            history_view,
            _prompt_toolkit_window(height=1, char=" "),
            input_view,
            slash_hint_panel,
            approval_panel,
        ]
    )

    app = _prompt_toolkit_application(
        layout=_prompt_toolkit_layout(root_container, focused_element=input_view),
        key_bindings=key_bindings,
        style=prompt_style,
        full_screen=False,
    )

    state.prompt_invalidator = lambda: _invalidate_prompt_application(app)
    state.history_setter = state.prompt_invalidator
    _refresh_history_view(state)
    app_task: asyncio.Task[None] = asyncio.create_task(app.run_async())
    try:
        while state.running:
            if app_task.done():
                state.running = False
                return
            try:
                line = await asyncio.wait_for(line_queue.get(), timeout=0.2)
            except asyncio.TimeoutError:
                continue
            except EOFError:
                state.running = False
                return
            except KeyboardInterrupt:
                state.running = False
                raise
            except asyncio.CancelledError:
                state.running = False
                return

            if not await _handle_user_line(client, state, line):
                return
    finally:
        state.history_setter = None
        state.prompt_invalidator = None
        if getattr(app, "is_running", False):
            with contextlib.suppress(Exception):
                app.exit(result=None)
        await _drain_background_task(app_task)


async def _handle_user_line(client: OpenJaxAsyncClient, state: AppState, line: str) -> bool:
    async def _resolve_approval_by_id_wrapped(client: OpenJaxAsyncClient, state: AppState, approval_request_id: str, approved: bool) -> None:
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

    text = _normalize_input(line).strip()
    if not text:
        if _approval_mode_active(state):
            approved = state.approval_selected_action == "allow"
            await _resolve_latest_approval(client, state, approved=approved, focused_approval_id_fn=_focused_approval_id, resolve_approval_by_id_fn=_resolve_approval_by_id_wrapped)
        return True
    if text == "/exit":
        state.running = False
        return False
    if text == "/help":
        _print_help()
        return True
    if text == "/pending":
        _print_pending(state)
        return True

    if text in ("y", "n") and _approval_mode_active(state):
        await _resolve_latest_approval(client, state, approved=(text == "y"), focused_approval_id_fn=_focused_approval_id, resolve_approval_by_id_fn=_resolve_approval_by_id_wrapped)
        return True

    if text.startswith("/approve "):
        parts = text.split()
        if len(parts) != 3 or parts[2] not in ("y", "n"):
            print("usage: /approve <approval_request_id> <y|n>")
            return True
        await _resolve_approval_by_id_wrapped(
            client,
            state,
            parts[1],
            parts[2] == "y",
        )
        return True

    if _approval_mode_active(state):
        if _use_inline_approval_panel(state):
            focus_id = _focused_approval_id(state)
            record = state.pending_approvals.get(focus_id or "")
            _tui_log_approval_event(
                _tui_log_info,
                action="input_blocked",
                request_id=focus_id,
                turn_id=record.turn_id if record else None,
                target=record.target if record else None,
                approved=None,
                resolved=None,
                detail="pending_request",
            )
        else:
            print("[approval] pending request: use Enter/y/n/Tab/Esc or /approve <id> y|n")
        return True

    if state.input_backend == "prompt_toolkit":
        _emit_ui_line(state, f"{_USER_PROMPT_PREFIX} {text}", refresh_history_view_fn=_refresh_history_view)

    try:
        turn_id = await client.submit_turn(text)
        state.waiting_turn_id = turn_id
        state.turn_phase = "thinking"
        if state.input_ready is not None:
            state.input_ready.clear()
    except OpenJaxResponseError as err:
        print(f"[error] submit failed: {err.code} {err.message}")

    return True


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
        _tui_debug(
            f"event received type={evt.event_type} turn_id={evt.turn_id or '-'} payload_keys={sorted(evt.payload.keys())}"
        )
        _dispatch_event(evt, state)
        _apply_event_state_updates(state, evt)


def _emit_ui_line_for_state(current_state: AppState, text: str) -> None:
    _emit_ui_line(
        current_state,
        text,
        refresh_history_view_fn=_refresh_history_view,
    )


def _print_prefixed_block_for_state(
    current_state: AppState, prefix: str, content: str
) -> None:
    _print_prefixed_block(
        current_state,
        prefix,
        content,
        align_multiline_fn=lambda text: _align_multiline(text, _PREFIX_CONTINUATION),
        emit_ui_line_fn=_emit_ui_line_for_state,
    )


def _status_bullet_for_state(state: AppState, ok: bool) -> str:
    return _status_bullet(
        state=state,
        ok=ok,
        assistant_prefix=_ASSISTANT_PREFIX,
        ansi_green=_ANSI_GREEN,
        ansi_red=_ANSI_RED,
        ansi_reset=_ANSI_RESET,
        supports_ansi_color_fn=_supports_ansi_color,
    )


def _render_assistant_delta_for_state(state: AppState, turn: str, delta: str) -> None:
    _render_assistant_delta(
        state,
        turn,
        delta,
        assistant_prefix=_ASSISTANT_PREFIX,
        align_multiline_fn=lambda text: _align_multiline(text, _PREFIX_CONTINUATION),
        finalize_stream_line_fn=_finalize_stream_line,
        refresh_history_view_fn=_refresh_history_view,
    )


def _render_assistant_message_for_state(state: AppState, turn: str, content: str) -> None:
    _render_assistant_message(
        state,
        turn,
        content,
        assistant_prefix=_ASSISTANT_PREFIX,
        print_prefixed_block_fn=_print_prefixed_block_for_state,
        finalize_stream_line_fn=_finalize_stream_line,
    )


def _finalize_stream_line_if_turn_for_state(state: AppState, turn: str) -> None:
    _finalize_stream_line_if_turn(
        state,
        turn,
        finalize_stream_line_fn=_finalize_stream_line,
    )


def _record_tool_started_for_state(state: AppState, turn: str, tool_name: str) -> None:
    _record_tool_started(
        state,
        turn,
        tool_name,
        monotonic_fn=time.monotonic,
    )


def _record_tool_completed_for_state(
    state: AppState, turn: str, tool_name: str, ok: bool
) -> None:
    _record_tool_completed(
        state,
        turn,
        tool_name,
        ok,
        monotonic_fn=time.monotonic,
        tool_turn_stats_cls=ToolTurnStats,
    )


def _print_tool_call_result_line_for_state(
    current_state: AppState, tool_name: str, ok: bool, output: str
) -> None:
    _print_tool_call_result_line(
        current_state,
        tool_name,
        ok,
        output,
        status_bullet_fn=lambda current_ok: _status_bullet_for_state(current_state, current_ok),
        tool_result_label_fn=_tool_result_label,
        finalize_stream_line_fn=_finalize_stream_line,
        emit_ui_spacer_fn=_emit_ui_spacer,
        emit_ui_line_fn=_emit_ui_line_for_state,
    )


def _print_tool_summary_for_turn_for_state(current_state: AppState, turn: str) -> None:
    _print_tool_summary_for_turn(
        current_state,
        turn,
        status_bullet_fn=lambda ok: _status_bullet_for_state(current_state, ok),
        finalize_stream_line_fn=_finalize_stream_line,
        emit_ui_line_fn=_emit_ui_line_for_state,
    )


def _dispatch_event(evt: EventEnvelope, state: AppState) -> None:
    _print_event(
        evt,
        state=state,
        print_tool_turn_summary=_PRINT_TOOL_TURN_SUMMARY,
        render_assistant_delta_fn=lambda turn, delta: _render_assistant_delta_for_state(state, turn, delta),
        render_assistant_message_fn=lambda turn, content: _render_assistant_message_for_state(state, turn, content),
        finalize_stream_line_if_turn_fn=lambda turn: _finalize_stream_line_if_turn_for_state(state, turn),
        record_tool_started_fn=lambda turn, tool_name: _record_tool_started_for_state(state, turn, tool_name),
        record_tool_completed_fn=lambda turn, tool_name, ok: _record_tool_completed_for_state(
            state, turn, tool_name, ok
        ),
        print_tool_call_result_line_fn=_print_tool_call_result_line_for_state,
        use_inline_approval_panel_fn=_use_inline_approval_panel,
        print_tool_summary_for_turn_fn=_print_tool_summary_for_turn_for_state,
    )


def _apply_event_state_updates(state: AppState, evt: EventEnvelope) -> None:
    if evt.event_type == "approval_requested" and evt.turn_id:
        request_id = str(evt.payload.get("request_id", ""))
        if request_id:
            record = ApprovalRecord(
                turn_id=evt.turn_id,
                target=str(evt.payload.get("target", "")),
                reason=str(evt.payload.get("reason", "")),
            )
            state.pending_approvals[request_id] = record
            if request_id not in state.approval_order:
                state.approval_order.append(request_id)
            state.approval_focus_id = request_id
            state.approval_selected_action = "allow"
            _tui_log_approval_event(
                _tui_log_info,
                action="requested",
                request_id=request_id,
                turn_id=evt.turn_id,
                target=record.target,
                approved=None,
                resolved=None,
                detail="event_received",
            )
            if not _use_inline_approval_panel(state):
                print(
                    f"[approval] use /approve {request_id} y|n, quick y/n, or press Enter to confirm default allow"
                )
            state.turn_phase = "approval"
            if state.input_ready is not None:
                state.input_ready.set()
            if state.approval_interrupt is not None:
                state.approval_interrupt.set()
            _tui_debug(
                f"approval state updated request_id={request_id} pending={len(state.pending_approvals)}"
            )
            _request_prompt_redraw(state, tui_debug_fn=_tui_debug)
        return

    if evt.event_type == "approval_resolved":
        request_id = str(evt.payload.get("request_id", ""))
        record = state.pending_approvals.get(request_id)
        approved = evt.payload.get("approved")
        approved_bool = approved if isinstance(approved, bool) else None
        _tui_log_approval_event(
            _tui_log_info,
            action="resolved_event",
            request_id=request_id,
            turn_id=record.turn_id if record else evt.turn_id,
            target=record.target if record else None,
            approved=approved_bool,
            resolved=True,
            detail="event_received",
        )
        _pop_pending(state, request_id)
        if not state.pending_approvals:
            state.turn_phase = "thinking" if state.waiting_turn_id else "idle"
        if state.waiting_turn_id and not state.pending_approvals and state.input_ready is not None:
            state.input_ready.clear()
        _tui_debug(
            f"approval resolved request_id={request_id} pending={len(state.pending_approvals)} phase={state.turn_phase}"
        )
        _request_prompt_redraw(state, tui_debug_fn=_tui_debug)
        return

    if evt.event_type == "turn_completed" and evt.turn_id == state.waiting_turn_id:
        state.waiting_turn_id = None
        state.turn_phase = "idle"
        if state.input_ready is not None:
            state.input_ready.set()
        _tui_debug("turn completed; input gate reopened")
        _request_prompt_redraw(state, tui_debug_fn=_tui_debug)


def _daemon_cmd_from_env() -> list[str]:
    cmd = os.environ.get("OPENJAX_DAEMON_CMD")
    if not cmd:
        return ["cargo", "run", "-q", "-p", "openjaxd"]
    return cmd.split()


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
            # Keep default terminal background/foreground for toolbar rows.
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
