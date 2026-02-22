from __future__ import annotations

import asyncio
import contextlib
from dataclasses import dataclass, field
import logging
from logging.handlers import RotatingFileHandler
import os
import queue
import re
import shutil
import signal
import sys
import threading
import time
from collections import deque
from pathlib import Path
from typing import Any, Callable

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxProtocolError, OpenJaxResponseError
from openjax_sdk.models import EventEnvelope

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
_TUI_LOGGER_NAME = "openjax_tui"
_TUI_LOG_FILENAME = "openjax_tui.log"
_TUI_LOG_MAX_BYTES_DEFAULT = 2 * 1024 * 1024
_TUI_LOG_BACKUP_COUNT = 5
_PRINT_TOOL_TURN_SUMMARY = False
_tui_logger: logging.Logger | None = None
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


class AppState:
    def __init__(self) -> None:
        self.running = True
        self.pending_approvals: dict[str, ApprovalRecord] = {}
        self.approval_order: deque[str] = deque()
        self.approval_focus_id: str | None = None
        self.approval_ui_enabled = False
        self.approval_selected_action = "allow"
        self.stream_turn_id: str | None = None
        self.stream_text_by_turn: dict[str, str] = {}
        self.waiting_turn_id: str | None = None
        self.input_ready: asyncio.Event | None = None
        self.approval_interrupt: asyncio.Event | None = None
        self.session_id: str | None = None
        self.input_backend: str = "basic"
        self.input_backend_reason: str = ""
        self.turn_phase: str = "idle"
        self.tool_turn_stats: dict[str, ToolTurnStats] = {}
        self.active_tool_starts: dict[tuple[str, str], list[float]] = {}
        self.prompt_invalidator: Callable[[], None] | None = None
        self.history_blocks: list[str] = []
        self.stream_block_index: int | None = None
        self.history_setter: Callable[[], None] | None = None


@dataclass
class ToolTurnStats:
    calls: int = 0
    ok_count: int = 0
    fail_count: int = 0
    known_duration_ms: int = 0
    tools: set[str] = field(default_factory=set)


@dataclass
class ApprovalRecord:
    turn_id: str
    target: str
    reason: str
    status: str = "pending"


_LOGO_GLYPHS: dict[str, tuple[str, ...]] = {
    "O": (
        " █████ ",
        "██   ██",
        "██   ██",
        "██   ██",
        "██   ██",
        " █████ ",
    ),
    "P": (
        "██████ ",
        "██   ██",
        "██████ ",
        "██     ",
        "██     ",
        "██     ",
    ),
    "E": (
        "███████",
        "██     ",
        "█████  ",
        "██     ",
        "██     ",
        "███████",
    ),
    "N": (
        "██   ██",
        "███  ██",
        "████ ██",
        "██ ████",
        "██  ███",
        "██   ██",
    ),
    "J": (
        "   ████",
        "    ██ ",
        "    ██ ",
        "    ██ ",
        "██  ██ ",
        " ████  ",
    ),
    "A": (
        "  ███  ",
        " ██ ██ ",
        "██   ██",
        "███████",
        "██   ██",
        "██   ██",
    ),
    "X": (
        "██   ██",
        " ██ ██ ",
        "  ███  ",
        "  ███  ",
        " ██ ██ ",
        "██   ██",
    ),
}


def _compose_logo(word: str, letter_spacing: int) -> str:
    glyphs = [_LOGO_GLYPHS[ch] for ch in word if ch in _LOGO_GLYPHS]
    if not glyphs:
        return ""

    height = max((len(glyph) for glyph in glyphs), default=0)
    normalized_glyphs: list[list[str]] = []
    for glyph in glyphs:
        glyph_width = max((len(row) for row in glyph), default=0)
        rows = [row.ljust(glyph_width) for row in glyph]
        if len(rows) < height:
            rows.extend([" " * glyph_width] * (height - len(rows)))
        normalized_glyphs.append(rows)

    spacer = " " * max(letter_spacing, 1)
    lines: list[str] = []
    for row_idx in range(height):
        lines.append(spacer.join(rows[row_idx] for rows in normalized_glyphs).rstrip())
    return "\n".join(lines)


_OPENJAX_LOGO_LONG = _compose_logo("OPENJAX", letter_spacing=2)
_OPENJAX_LOGO_SHORT = _compose_logo("OPENJAX", letter_spacing=1)
_OPENJAX_LOGO_TINY = "OPENJAX"


async def run() -> None:
    _setup_tui_logger()
    input_backend, backend_reason = _select_input_backend_with_reason()
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
        _log_startup_summary(state, version=version)

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
    _start_basic_input_worker(asyncio.get_running_loop(), request_queue, line_queue)

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

    slash_completer = _build_slash_command_completer()
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
                    str(getattr(input_view.buffer, "text", ""))
                )
            ),
            dont_extend_height=True,
        ),
        filter=_prompt_toolkit_condition(
            lambda: bool(_slash_hint_text(str(getattr(input_view.buffer, "text", ""))))
        ),
    )

    approval_panel = _prompt_toolkit_conditional_container(
        content=_prompt_toolkit_window(
            content=_prompt_toolkit_formatted_text_control(
                lambda: _approval_toolbar_fragments(state)
            ),
            dont_extend_height=True,
        ),
        filter=_prompt_toolkit_condition(lambda: bool(_approval_toolbar_text(state))),
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
    text = _normalize_input(line).strip()
    if not text:
        if _approval_mode_active(state):
            approved = state.approval_selected_action == "allow"
            await _resolve_latest_approval(client, state, approved=approved)
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
        await _resolve_latest_approval(client, state, approved=(text == "y"))
        return True

    if text.startswith("/approve "):
        parts = text.split()
        if len(parts) != 3 or parts[2] not in ("y", "n"):
            print("usage: /approve <approval_request_id> <y|n>")
            return True
        await _resolve_approval_by_id(
            client=client,
            state=state,
            approval_request_id=parts[1],
            approved=parts[2] == "y",
        )
        return True

    if _approval_mode_active(state):
        if _use_inline_approval_panel(state):
            focus_id = _focused_approval_id(state)
            record = state.pending_approvals.get(focus_id or "")
            _tui_log_approval_event(
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
        _emit_ui_line(f"{_USER_PROMPT_PREFIX} {text}")

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
        _print_event(evt)
        _apply_event_state_updates(state, evt)


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
            _request_prompt_redraw(state)
        return

    if evt.event_type == "approval_resolved":
        request_id = str(evt.payload.get("request_id", ""))
        record = state.pending_approvals.get(request_id)
        approved = evt.payload.get("approved")
        approved_bool = approved if isinstance(approved, bool) else None
        _tui_log_approval_event(
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
        _request_prompt_redraw(state)
        return

    if evt.event_type == "turn_completed" and evt.turn_id == state.waiting_turn_id:
        state.waiting_turn_id = None
        state.turn_phase = "idle"
        if state.input_ready is not None:
            state.input_ready.set()
        _tui_debug("turn completed; input gate reopened")
        _request_prompt_redraw(state)


def _invalidate_prompt_application(app: Any) -> None:
    if app is None:
        return
    invalidate = getattr(app, "invalidate", None)
    if callable(invalidate):
        invalidate()


def _request_prompt_redraw(state: AppState) -> None:
    invalidator = state.prompt_invalidator
    if invalidator is None:
        _tui_debug("prompt redraw skipped: no invalidator")
        return
    _tui_debug("prompt redraw requested")
    with contextlib.suppress(Exception):
        invalidator()


def _history_text(state: AppState) -> str:
    if not state.history_blocks:
        return "\n"
    return "\n" + "\n\n".join(state.history_blocks)


def _refresh_history_view(state: AppState) -> None:
    setter = state.history_setter
    if setter is None:
        return
    with contextlib.suppress(Exception):
        setter()


async def _drain_background_task(task: asyncio.Task[Any] | None) -> None:
    if task is None:
        return
    if not task.done():
        task.cancel()
    with contextlib.suppress(BaseException):
        await task


def _tui_debug_enabled() -> bool:
    flag = os.environ.get("OPENJAX_TUI_DEBUG", "")
    return flag.lower() in {"1", "true", "yes", "on"}


def _tui_debug(message: str) -> None:
    if not _tui_debug_enabled():
        return
    logger = _tui_logger
    if logger is not None:
        logger.debug(message)


def _tui_log_info(message: str) -> None:
    logger = _tui_logger
    if logger is not None:
        logger.info(message)


def _approval_bool_field(value: bool | None) -> str:
    if value is None:
        return "-"
    return "true" if value else "false"


def _approval_text_field(value: str | None) -> str:
    text = (value or "-").strip()
    if not text:
        return "-"
    return "_".join(text.split())


def _tui_log_approval_event(
    action: str,
    request_id: str | None = None,
    turn_id: str | None = None,
    target: str | None = None,
    approved: bool | None = None,
    resolved: bool | None = None,
    detail: str | None = None,
) -> None:
    _tui_log_info(
        "approval_event "
        f"action={_approval_text_field(action)} "
        f"request_id={_approval_text_field(request_id)} "
        f"turn_id={_approval_text_field(turn_id)} "
        f"target={_approval_text_field(target)} "
        f"approved={_approval_bool_field(approved)} "
        f"resolved={_approval_bool_field(resolved)} "
        f"detail={_approval_text_field(detail)}"
    )


def _setup_tui_logger() -> logging.Logger | None:
    global _tui_logger
    if _tui_logger is not None:
        return _tui_logger

    log_dir = os.environ.get("OPENJAX_TUI_LOG_DIR", os.path.join(".openjax", "logs"))
    max_bytes = _parse_log_max_bytes(
        os.environ.get("OPENJAX_TUI_LOG_MAX_BYTES", ""),
        _TUI_LOG_MAX_BYTES_DEFAULT,
    )
    log_path = os.path.join(log_dir, _TUI_LOG_FILENAME)

    with contextlib.suppress(OSError):
        os.makedirs(log_dir, exist_ok=True)

    logger = logging.getLogger(_TUI_LOGGER_NAME)
    logger.setLevel(logging.DEBUG)
    logger.propagate = False
    for handler in list(logger.handlers):
        logger.removeHandler(handler)
        with contextlib.suppress(Exception):
            handler.close()

    try:
        handler = RotatingFileHandler(
            log_path,
            maxBytes=max_bytes,
            backupCount=_TUI_LOG_BACKUP_COUNT,
            encoding="utf-8",
        )
    except OSError as err:
        print(f"[warn] failed to initialize tui logger at {log_path}: {err}", file=sys.stderr)
        _tui_logger = None
        return None

    handler.setLevel(logging.DEBUG)
    handler.setFormatter(
        logging.Formatter("%(asctime)s %(levelname)s %(name)s %(message)s")
    )
    logger.addHandler(handler)
    logger.info(
        "tui logger initialized path=%s max_bytes=%s backups=%s",
        log_path,
        max_bytes,
        _TUI_LOG_BACKUP_COUNT,
    )
    _tui_logger = logger
    return logger


def _parse_log_max_bytes(raw: str, fallback: int) -> int:
    with contextlib.suppress(ValueError):
        value = int(raw.strip())
        if value > 0:
            return value
    return fallback


def _reset_tui_logger_for_tests() -> None:
    global _tui_logger
    logger = _tui_logger
    if logger is None:
        return
    for handler in list(logger.handlers):
        logger.removeHandler(handler)
        with contextlib.suppress(Exception):
            handler.close()
    _tui_logger = None


def _print_event(evt: EventEnvelope) -> None:
    state = _active_state
    turn = evt.turn_id or "-"
    t = evt.event_type
    if t == "assistant_delta":
        _render_assistant_delta(turn, str(evt.payload.get("content_delta", "")))
        return
    if t == "assistant_message":
        content = str(evt.payload.get("content", ""))
        _render_assistant_message(turn, content)
        return
    if t == "tool_call_started":
        _finalize_stream_line_if_turn(turn)
        _record_tool_started(turn, str(evt.payload.get("tool_name", "")))
        return
    if t == "tool_call_completed":
        _finalize_stream_line_if_turn(turn)
        tool_name = str(evt.payload.get("tool_name", ""))
        ok = bool(evt.payload.get("ok"))
        output = str(evt.payload.get("output", ""))
        _record_tool_completed(
            turn=turn,
            tool_name=tool_name,
            ok=ok,
        )
        _print_tool_call_result_line(tool_name=tool_name, ok=ok, output=output)
        return
    if t == "approval_requested":
        _finalize_stream_line_if_turn(turn)
        if state is None or not _use_inline_approval_panel(state):
            print(
                "[approval] id={rid} target={target} reason={reason}".format(
                    rid=evt.payload.get("request_id"),
                    target=evt.payload.get("target"),
                    reason=evt.payload.get("reason"),
                )
            )
        return
    if t == "approval_resolved":
        _finalize_stream_line_if_turn(turn)
        if state is None or not _use_inline_approval_panel(state):
            print(
                f"[approval] id={evt.payload.get('request_id')} approved={evt.payload.get('approved')}"
            )
        return
    if t == "turn_completed":
        _finalize_stream_line_if_turn(turn)
        if _PRINT_TOOL_TURN_SUMMARY:
            _print_tool_summary_for_turn(turn)
        if state is not None:
            state.tool_turn_stats.pop(turn, None)
        return
    if t == "turn_started":
        return
    print(f"[{t}]")


def _daemon_cmd_from_env() -> list[str]:
    cmd = os.environ.get("OPENJAX_DAEMON_CMD")
    if not cmd:
        return ["cargo", "run", "-q", "-p", "openjaxd"]
    return cmd.split()


def _select_input_backend() -> str:
    backend, _ = _select_input_backend_with_reason()
    return backend


def _select_input_backend_with_reason() -> tuple[str, str]:
    env_backend = os.environ.get("OPENJAX_TUI_INPUT_BACKEND", "").lower()
    if env_backend == "basic":
        return "basic", "forced by OPENJAX_TUI_INPUT_BACKEND=basic"
    if env_backend == "prompt_toolkit":
        if PromptSession is not None and patch_stdout is not None:
            if KeyBindings is None:
                return "prompt_toolkit", "forced by env; key bindings unavailable"
            return "prompt_toolkit", "forced by OPENJAX_TUI_INPUT_BACKEND=prompt_toolkit"
        return "basic", "env requested prompt_toolkit but dependency unavailable"
    if (
        PromptSession is not None
        and patch_stdout is not None
        and sys.stdin.isatty()
        and sys.stdout.isatty()
    ):
        if KeyBindings is None:
            return "prompt_toolkit", "tty mode; key bindings unavailable"
        return "prompt_toolkit", "tty mode"

    reason_parts: list[str] = []
    if PromptSession is None or patch_stdout is None:
        reason_parts.append(_prompt_toolkit_import_error or "prompt_toolkit unavailable")
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        reason_parts.append("stdin/stdout is not tty")
    if not reason_parts:
        reason_parts.append("fallback to basic")
    return "basic", "; ".join(reason_parts)


def _start_basic_input_worker(
    loop: asyncio.AbstractEventLoop,
    request_queue: queue.Queue[object],
    line_queue: asyncio.Queue[str | None],
) -> None:
    def worker() -> None:
        while True:
            cmd = request_queue.get()
            if cmd is _INPUT_STOP:
                return
            if cmd is not _INPUT_REQUEST:
                continue
            try:
                prompt_prefix = _USER_PROMPT_PREFIX
                active_state = _active_state
                if active_state is not None and _approval_mode_active(active_state):
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


def _log_startup_summary(state: AppState, version: str) -> None:
    summary = (
        "python_tui started "
        f"version={version} "
        f"cwd={os.getcwd()} "
        f"session={state.session_id or '-'} "
        f"backend={state.input_backend} "
        f"phase={state.turn_phase} "
        f"approvals={len(state.pending_approvals)} "
        f"input_reason={_approval_text_field(state.input_backend_reason)}"
    )
    _tui_log_info(summary)
    _append_openjax_log_line(summary)


def _append_openjax_log_line(message: str) -> None:
    timestamp = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    log_path = _OPENJAX_ROOT_LOG
    with contextlib.suppress(OSError):
        os.makedirs(os.path.dirname(log_path), exist_ok=True)
        with open(log_path, "a", encoding="utf-8") as fh:
            fh.write(f"{timestamp} INFO python_tui {message}\n")


def _resolve_openjax_version() -> str:
    env_version = os.environ.get("OPENJAX_VERSION", "").strip()
    if env_version:
        return env_version

    cargo_path = Path(__file__).resolve().parents[4] / "Cargo.toml"
    if not cargo_path.exists():
        return "dev"

    in_workspace_package = False
    with contextlib.suppress(OSError):
        with cargo_path.open("r", encoding="utf-8") as fh:
            for raw_line in fh:
                line = raw_line.strip()
                if line.startswith("[") and line.endswith("]"):
                    in_workspace_package = line == "[workspace.package]"
                    continue
                if not in_workspace_package:
                    continue
                match = re.match(r'^version\s*=\s*"([^"]+)"$', line)
                if match:
                    return match.group(1)
    return "dev"


def _slash_command_candidates(text_before_cursor: str) -> list[str]:
    if not text_before_cursor.startswith("/"):
        return []
    token = text_before_cursor.split(maxsplit=1)[0]
    if not token:
        return list(_SLASH_COMMANDS)
    return [cmd for cmd in _SLASH_COMMANDS if cmd.startswith(token)]


def _slash_hint_text(input_text: str) -> str:
    candidates = _slash_command_candidates(input_text)
    if not candidates:
        return ""
    joined = "  ".join(candidates)
    return f" commands: {joined}"


def _slash_hint_fragments(input_text: str) -> Any:
    text = _slash_hint_text(input_text)
    if not text:
        return ""
    return [("bg:default fg:default noreverse", text)]


def _build_slash_command_completer() -> Any:
    if _prompt_toolkit_completer is None or _prompt_toolkit_completion is None:
        return None

    completion_cls = _prompt_toolkit_completion

    class _SlashCompleter(_prompt_toolkit_completer):
        def get_completions(self, document: Any, complete_event: Any) -> Any:
            _ = complete_event
            text_before_cursor = str(getattr(document, "text_before_cursor", ""))
            token = text_before_cursor.split(maxsplit=1)[0]
            for command in _slash_command_candidates(text_before_cursor):
                yield completion_cls(
                    command,
                    start_position=-len(token),
                )

    return _SlashCompleter()


def _format_display_directory(path: str) -> str:
    home = os.path.expanduser("~")
    if path == home:
        return "~"
    prefix = home + os.sep
    if path.startswith(prefix):
        return "~/" + path[len(prefix) :]
    return path


def _print_startup_card(version: str) -> None:
    model = os.environ.get("OPENJAX_MODEL", "(default)")
    directory = _format_display_directory(os.getcwd())
    rows = [
        f">_ OpenJax TUI (v{version})",
        "",
        f"model:     {model}",
        f"directory: {directory}",
    ]
    content_width = max((len(row) for row in rows), default=0) + 2
    top = "╭" + ("─" * content_width) + "╮"
    bottom = "╰" + ("─" * content_width) + "╯"
    print(top)
    for row in rows:
        print(f"│ {row.ljust(content_width - 1)}│")
    print(bottom)
    print()


def _divider_line() -> str:
    columns = shutil.get_terminal_size(fallback=(100, 24)).columns
    return "─" * max(min(columns, 100), 24)


def _text_block_width(text: str) -> int:
    return max((len(line) for line in text.splitlines()), default=0)


def _normalize_logo_block(text: str) -> str:
    lines = text.splitlines()
    while lines and not lines[0].strip():
        _ = lines.pop(0)
    while lines and not lines[-1].strip():
        _ = lines.pop()

    if not lines:
        return ""

    non_empty = [line for line in lines if line.strip()]
    common_indent = min(
        (len(line) - len(line.lstrip(" ")) for line in non_empty), default=0
    )
    normalized = [line[common_indent:].rstrip() for line in lines]
    return "\n".join(normalized)


def _select_logo(columns: int) -> str:
    long_width = _text_block_width(_normalize_logo_block(_OPENJAX_LOGO_LONG))
    short_width = _text_block_width(_normalize_logo_block(_OPENJAX_LOGO_SHORT))
    if columns >= long_width:
        return _OPENJAX_LOGO_LONG
    if columns >= short_width:
        return _OPENJAX_LOGO_SHORT
    return _OPENJAX_LOGO_TINY


def _print_logo() -> None:
    columns = shutil.get_terminal_size(fallback=(100, 24)).columns
    plain_logo = _normalize_logo_block(_select_logo(columns))
    logo = plain_logo
    if _supports_ansi_color():
        logo = _apply_horizontal_gradient(logo)
    print(logo)
    subtitle = "OPENJAX TERMINAL"
    subtitle_padding = max((_text_block_width(plain_logo) - len(subtitle)) // 2, 0)
    print(" " * subtitle_padding + subtitle)
    print()


def _supports_ansi_color() -> bool:
    if os.environ.get("NO_COLOR"):
        return False
    if not sys.stdout.isatty():
        return False
    term = os.environ.get("TERM", "")
    if term == "dumb":
        return False
    return True


def _apply_horizontal_gradient(text: str) -> str:
    lines = text.splitlines()
    if not lines:
        return text

    width = max((len(line) for line in lines), default=0)
    if width <= 1:
        return text

    start = (98, 157, 255)
    end = (255, 120, 180)

    def lerp(a: int, b: int, t: float) -> int:
        return int(round(a + (b - a) * t))

    rendered_lines: list[str] = []
    for line in lines:
        rendered_chars: list[str] = []
        for idx, ch in enumerate(line):
            if ch.isspace():
                rendered_chars.append(ch)
                continue
            t = idx / (width - 1)
            r = lerp(start[0], end[0], t)
            g = lerp(start[1], end[1], t)
            b = lerp(start[2], end[2], t)
            rendered_chars.append(f"\x1b[38;2;{r};{g};{b}m{ch}")
        rendered_chars.append("\x1b[0m")
        rendered_lines.append("".join(rendered_chars))

    return "\n".join(rendered_lines)


def _print_pending(state: AppState) -> None:
    if not state.pending_approvals:
        print("[approval] no pending approvals")
        return
    print("[approval] pending:")
    for request_id in list(state.approval_order):
        record = state.pending_approvals.get(request_id)
        if record and record.status == "pending":
            focus_marker = "*" if request_id == state.approval_focus_id else " "
            print(
                f" {focus_marker} {request_id} (turn:{record.turn_id}) target={record.target or '-'}"
            )


def _use_inline_approval_panel(state: AppState) -> bool:
    return state.input_backend == "prompt_toolkit" and state.approval_ui_enabled


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


async def _resolve_latest_approval(
    client: OpenJaxAsyncClient, state: AppState, approved: bool
) -> None:
    focus_id = _focused_approval_id(state)
    if focus_id:
        await _resolve_approval_by_id(client, state, focus_id, approved)
        return

    while state.approval_order:
        approval_request_id = state.approval_order[-1]
        if approval_request_id in state.pending_approvals:
            await _resolve_approval_by_id(client, state, approval_request_id, approved)
            return
        state.approval_order.pop()
    print("[approval] no pending approvals")


async def _resolve_approval_by_id(
    client: OpenJaxAsyncClient,
    state: AppState,
    approval_request_id: str,
    approved: bool,
) -> None:
    record = state.pending_approvals.get(approval_request_id)
    if not record or record.status != "pending":
        print(f"[approval] request not found: {approval_request_id}")
        return
    try:
        ok = await client.resolve_approval(
            turn_id=record.turn_id,
            request_id=approval_request_id,
            approved=approved,
            reason="approved_by_tui" if approved else "rejected_by_tui",
        )
        _tui_log_approval_event(
            action="resolved_submit",
            request_id=approval_request_id,
            turn_id=record.turn_id,
            target=record.target,
            approved=approved,
            resolved=ok,
            detail="client_submit",
        )
        if not _use_inline_approval_panel(state):
            print(f"[approval] resolved: {approval_request_id} ok={ok}")
        _pop_pending(state, approval_request_id)
    except OpenJaxResponseError as err:
        if _is_expired_approval_error(err):
            _tui_log_approval_event(
                action="resolve_expired",
                request_id=approval_request_id,
                turn_id=record.turn_id,
                target=record.target,
                approved=approved,
                resolved=False,
                detail=err.code,
            )
            print(f"[approval] auto-denied (expired): {approval_request_id}")
            _pop_pending(state, approval_request_id)
            return
        _tui_log_approval_event(
            action="resolve_failed",
            request_id=approval_request_id,
            turn_id=record.turn_id,
            target=record.target,
            approved=approved,
            resolved=False,
            detail=err.code,
        )
        print(f"[approval] resolve failed: {err.code} {err.message}")


def _pop_pending(state: AppState, approval_request_id: str) -> None:
    state.pending_approvals.pop(approval_request_id, None)
    with contextlib.suppress(ValueError):
        state.approval_order.remove(approval_request_id)
    if state.approval_focus_id == approval_request_id:
        state.approval_focus_id = _focused_approval_id(state)


def _focused_approval_id(state: AppState) -> str | None:
    if state.approval_focus_id and state.approval_focus_id in state.pending_approvals:
        return state.approval_focus_id
    while state.approval_order:
        approval_request_id = state.approval_order[-1]
        record = state.pending_approvals.get(approval_request_id)
        if record and record.status == "pending":
            state.approval_focus_id = approval_request_id
            return approval_request_id
        state.approval_order.pop()
    state.approval_focus_id = None
    return None


def _approval_mode_active(state: AppState) -> bool:
    return state.turn_phase == "approval" and _focused_approval_id(state) is not None


def _input_prompt_prefix(state: AppState) -> str:
    if _use_inline_approval_panel(state):
        return _USER_PROMPT_PREFIX
    if _approval_mode_active(state):
        return "approval>"
    return _USER_PROMPT_PREFIX


def _approval_pending_ids(state: AppState) -> list[str]:
    ids: list[str] = []
    for approval_request_id in state.approval_order:
        record = state.pending_approvals.get(approval_request_id)
        if record and record.status == "pending":
            ids.append(approval_request_id)
    return ids


def _move_approval_focus(state: AppState, step: int) -> None:
    pending_ids = _approval_pending_ids(state)
    if not pending_ids:
        state.approval_focus_id = None
        return
    focus_id = _focused_approval_id(state)
    if focus_id is None or focus_id not in pending_ids:
        state.approval_focus_id = pending_ids[-1]
        return
    idx = pending_ids.index(focus_id)
    next_idx = max(0, min(len(pending_ids) - 1, idx + step))
    state.approval_focus_id = pending_ids[next_idx]


def _toggle_approval_selection(state: AppState) -> None:
    state.approval_selected_action = (
        "deny" if state.approval_selected_action == "allow" else "allow"
    )


def _approval_toolbar_text(state: AppState) -> str:
    if not state.approval_ui_enabled or not _approval_mode_active(state):
        return ""
    focus_id = _focused_approval_id(state)
    if not focus_id:
        return ""
    record = state.pending_approvals.get(focus_id)
    if record is None:
        return ""
    total = len(_approval_pending_ids(state))
    selected_allow = state.approval_selected_action == "allow"
    target = (record.target or "-").strip() or "-"
    reason = " ".join(str(record.reason or "-").split())
    allow_label = "❯ 1. Yes" if selected_allow else "  1. Yes"
    deny_label = "❯ 2. No" if not selected_allow else "  2. No"
    lines = [
        _divider_line(),
        f" Approval Request ({total} pending)",
        f" id: {focus_id}",
        f" target: {target}",
        f" reason: {reason}",
        " Confirm this action?",
        f" {allow_label}",
        f" {deny_label}",
        " Tab/Up/Down switch · Enter confirm · Esc reject · /approve <id> y|n",
    ]
    return "\n".join(lines)


def _approval_toolbar_fragments(state: AppState) -> Any:
    text = _approval_toolbar_text(state)
    if not text:
        return ""
    return [("bg:default fg:default noreverse", text)]


def _build_prompt_key_bindings(client: OpenJaxAsyncClient, state: AppState) -> Any:
    if KeyBindings is None:
        return None
    kb = KeyBindings()

    @kb.add("tab", eager=True)
    def _toggle_action(event: object) -> None:
        if not _approval_mode_active(state):
            app = getattr(event, "app", None)
            current_buffer = getattr(app, "current_buffer", None)
            text = str(getattr(current_buffer, "text", "")).strip()
            if current_buffer is not None and text.startswith("/"):
                start_completion = getattr(current_buffer, "start_completion", None)
                if callable(start_completion):
                    start_completion(select_first=False)
            return
        _toggle_approval_selection(state)
        app = getattr(event, "app", None)
        if app is not None:
            app.invalidate()

    @kb.add("up", eager=True)
    def _toggle_action_up(event: object) -> None:
        app = getattr(event, "app", None)
        if not _approval_mode_active(state):
            return
        _toggle_approval_selection(state)
        if app is not None:
            app.invalidate()

    @kb.add("down", eager=True)
    def _toggle_action_down(event: object) -> None:
        app = getattr(event, "app", None)
        if not _approval_mode_active(state):
            return
        _toggle_approval_selection(state)
        if app is not None:
            app.invalidate()

    @kb.add("enter", eager=True)
    def _enter_resolve(event: object) -> None:
        app = getattr(event, "app", None)
        current_buffer = getattr(app, "current_buffer", None)
        current_text = getattr(current_buffer, "text", "")
        if not (_approval_mode_active(state) and not str(current_text).strip()):
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
        if not (_approval_mode_active(state) and not str(current_text).strip()):
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
        if not (_approval_mode_active(state) and not str(current_text).strip()):
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
        if not (_approval_mode_active(state) and not str(current_text).strip()):
            return
        if current_buffer is None:
            return
        state.approval_selected_action = "deny"
        current_buffer.text = ""
        validate_and_handle = getattr(current_buffer, "validate_and_handle", None)
        if callable(validate_and_handle):
            validate_and_handle()

    return kb


def _is_expired_approval_error(err: OpenJaxResponseError) -> bool:
    if err.code == "APPROVAL_NOT_FOUND":
        return True
    message = err.message.lower()
    return "timed out" in message or "timeout" in message


def _record_tool_started(turn: str, tool_name: str) -> None:
    state = _active_state
    if state is None:
        return
    key = (turn, tool_name)
    starts = state.active_tool_starts.setdefault(key, [])
    starts.append(time.monotonic())


def _record_tool_completed(turn: str, tool_name: str, ok: bool) -> None:
    state = _active_state
    if state is None:
        return
    stats = state.tool_turn_stats.setdefault(turn, ToolTurnStats())
    stats.calls += 1
    if ok:
        stats.ok_count += 1
    else:
        stats.fail_count += 1
    if tool_name:
        stats.tools.add(tool_name)

    key = (turn, tool_name)
    starts = state.active_tool_starts.get(key, [])
    if starts:
        elapsed_ms = max(int((time.monotonic() - starts.pop()) * 1000), 0)
        stats.known_duration_ms += elapsed_ms
    if not starts:
        state.active_tool_starts.pop(key, None)


def _print_tool_call_result_line(tool_name: str, ok: bool, output: str) -> None:
    state = _active_state
    if state is None:
        return
    bullet = _status_bullet(ok)
    label = _tool_result_label(tool_name, output)
    if not ok:
        label = f"{label} (failed)"
    _finalize_stream_line(state)
    _emit_ui_spacer()
    _emit_ui_line(f"{bullet} {label}")
    _emit_ui_spacer()


def _tool_result_label(tool_name: str, output: str) -> str:
    name = tool_name.strip().lower()
    if name == "read_file":
        return "Read 1 file"
    if name in {"apply_patch", "edit_file_range", "write_file"}:
        target = _extract_updated_target(output)
        if target:
            return f"Update({target})"
        return "Update file"
    if name == "list_dir":
        return "Read directory"
    if name == "grep_files":
        return "Search files"
    if name == "shell":
        return "Run shell command"
    if not name:
        return "Tool call"
    return name.replace("_", " ").title()


def _extract_updated_target(output: str) -> str | None:
    match = re.search(r"\bUPDATE\s+([^\s:]+)", output)
    if not match:
        return None
    return match.group(1).strip()


def _print_tool_summary_for_turn(turn: str) -> None:
    state = _active_state
    if state is None:
        return
    stats = state.tool_turn_stats.get(turn)
    if stats is None or stats.calls == 0:
        return

    tools = ", ".join(sorted(stats.tools)) if stats.tools else "-"
    duration = f"{stats.known_duration_ms}ms" if stats.known_duration_ms else "n/a"
    ok = stats.fail_count == 0
    bullet = _status_bullet(ok)
    _finalize_stream_line(state)
    _emit_ui_line(
        f"{bullet} tools: calls={stats.calls} ok={stats.ok_count} "
        f"fail={stats.fail_count} duration={duration} names=[{tools}]"
    )


def _emit_ui_spacer() -> None:
    state = _active_state
    if state is not None and state.input_backend == "prompt_toolkit":
        return
    print()


def _normalize_input(text: str) -> str:
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


def _configure_readline_keybindings() -> None:
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


_active_state: AppState | None = None


def _set_active_state(state: AppState | None) -> None:
    global _active_state
    _active_state = state


def _render_assistant_delta(turn: str, delta: str) -> None:
    state = _active_state
    if state is None:
        return
    if not delta:
        return
    if state.input_backend == "prompt_toolkit":
        if state.stream_turn_id != turn:
            _finalize_stream_line(state)
            state.stream_turn_id = turn
            state.stream_text_by_turn[turn] = ""
            state.stream_block_index = len(state.history_blocks)
            state.history_blocks.append(f"{_ASSISTANT_PREFIX} ")
        state.stream_text_by_turn[turn] = state.stream_text_by_turn.get(turn, "") + delta
        stream_text = state.stream_text_by_turn.get(turn, "")
        block = f"{_ASSISTANT_PREFIX} {_align_multiline(stream_text)}"
        idx = state.stream_block_index
        if idx is not None and 0 <= idx < len(state.history_blocks):
            state.history_blocks[idx] = block
        _refresh_history_view(state)
        return
    if state.stream_turn_id != turn:
        _finalize_stream_line()
        state.stream_turn_id = turn
        state.stream_text_by_turn[turn] = ""
        print(f"{_ASSISTANT_PREFIX} ", end="", flush=True)
    state.stream_text_by_turn[turn] = state.stream_text_by_turn.get(turn, "") + delta
    print(_align_multiline(delta), end="", flush=True)


def _render_assistant_message(turn: str, content: str) -> None:
    state = _active_state
    if state is None:
        _print_prefixed_block(_ASSISTANT_PREFIX, content)
        return

    if state.stream_turn_id == turn:
        streamed = state.stream_text_by_turn.get(turn, "")
        if streamed == content:
            _finalize_stream_line()
            return
        _finalize_stream_line()

    _print_prefixed_block(_ASSISTANT_PREFIX, content)


def _finalize_stream_line_if_turn(turn: str) -> None:
    state = _active_state
    if state is None:
        return
    if state.stream_turn_id == turn:
        _finalize_stream_line()


def _finalize_stream_line(state: AppState | None = None) -> None:
    current = state or _active_state
    if current is None:
        return
    if current.stream_turn_id is not None:
        if current.input_backend != "prompt_toolkit":
            print()
        current.stream_turn_id = None
        current.stream_block_index = None


def _align_multiline(text: str) -> str:
    if not text:
        return ""
    return text.replace("\n", f"\n{_PREFIX_CONTINUATION}")


def _print_prefixed_block(prefix: str, content: str) -> None:
    aligned = _align_multiline(content)
    _emit_ui_line(f"{prefix} {aligned}")


def _emit_ui_line(text: str) -> None:
    state = _active_state
    if state is not None and state.input_backend == "prompt_toolkit":
        state.history_blocks.append(text)
        _refresh_history_view(state)
        return
    print(text)


def _status_bullet(ok: bool) -> str:
    state = _active_state
    if state is not None and state.input_backend == "prompt_toolkit":
        color = _ANSI_GREEN if ok else _ANSI_RED
        return f"{color}{_ASSISTANT_PREFIX}{_ANSI_RESET}"
    if not _supports_ansi_color():
        return "🟢" if ok else "🔴"
    color = _ANSI_GREEN if ok else _ANSI_RED
    return f"{color}{_ASSISTANT_PREFIX}{_ANSI_RESET}"
