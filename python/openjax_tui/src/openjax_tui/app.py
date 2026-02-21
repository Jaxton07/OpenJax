from __future__ import annotations

import asyncio
import contextlib
from dataclasses import dataclass, field
import os
import queue
import re
import shutil
import signal
import sys
import threading
import time
from collections import deque

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxProtocolError, OpenJaxResponseError
from openjax_sdk.models import EventEnvelope

try:
    from prompt_toolkit import PromptSession
    from prompt_toolkit.patch_stdout import patch_stdout
except Exception:  # pragma: no cover - optional dependency fallback
    PromptSession = None  # type: ignore[assignment]
    patch_stdout = None  # type: ignore[assignment]

_INPUT_REQUEST = object()
_INPUT_STOP = object()
_USER_PROMPT_PREFIX = "❯"
_ASSISTANT_PREFIX = "⏺"


class AppState:
    def __init__(self) -> None:
        self.running = True
        self.pending_approvals: dict[str, str] = {}
        self.approval_order: deque[str] = deque()
        self.stream_turn_id: str | None = None
        self.stream_text_by_turn: dict[str, str] = {}
        self.waiting_turn_id: str | None = None
        self.input_ready: asyncio.Event | None = None
        self.session_id: str | None = None
        self.input_backend: str = "basic"
        self.turn_phase: str = "idle"
        self.tool_turn_stats: dict[str, ToolTurnStats] = {}
        self.active_tool_starts: dict[tuple[str, str], list[float]] = {}


@dataclass
class ToolTurnStats:
    calls: int = 0
    ok_count: int = 0
    fail_count: int = 0
    known_duration_ms: int = 0
    tools: set[str] = field(default_factory=set)


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
    input_backend = _select_input_backend()
    if input_backend == "basic":
        _configure_readline_keybindings()
    daemon_cmd = _daemon_cmd_from_env()
    client = OpenJaxAsyncClient(daemon_cmd=daemon_cmd)
    state = AppState()
    state.input_ready = asyncio.Event()
    state.input_ready.set()
    _set_active_state(state)

    await client.start()
    try:
        session_id = await client.start_session()
        state.session_id = session_id
        state.input_backend = input_backend
        await client.stream_events()
        _print_logo()
        print("OpenJax TUI")
        print(f"cwd={os.getcwd()}")
        _print_status_bar(state)
        _print_help()

        event_task = asyncio.create_task(_event_loop(client, state))
        try:
            if input_backend == "prompt_toolkit":
                await _input_loop_prompt_toolkit(client, state)
            else:
                await _input_loop_basic(client, state)
        except (KeyboardInterrupt, asyncio.CancelledError):
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
        await _shutdown_client_quietly(client)
        _finalize_stream_line(state)
        _set_active_state(None)
        print("openjax_tui exited")


async def _shutdown_client_quietly(client: OpenJaxAsyncClient) -> None:
    with contextlib.suppress(
        OpenJaxProtocolError,
        OpenJaxResponseError,
        ConnectionError,
        BrokenPipeError,
        RuntimeError,
        asyncio.CancelledError,
    ):
        if client.session_id:
            await client.shutdown_session()
    with contextlib.suppress(
        OpenJaxProtocolError,
        OpenJaxResponseError,
        ConnectionError,
        BrokenPipeError,
        RuntimeError,
        asyncio.CancelledError,
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
            return
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
    if PromptSession is None or patch_stdout is None:
        await _input_loop_basic(client, state)
        return

    session = PromptSession("> ")
    with patch_stdout():
        while state.running:
            try:
                await state.input_ready.wait()
                line = await session.prompt_async()
            except (EOFError, KeyboardInterrupt):
                state.running = False
                return
            except asyncio.CancelledError:
                state.running = False
                return

            if not await _handle_user_line(client, state, line):
                return


async def _handle_user_line(client: OpenJaxAsyncClient, state: AppState, line: str) -> bool:
    text = _normalize_input(line).strip()
    if not text:
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

    if text in ("y", "n"):
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

    try:
        turn_id = await client.submit_turn(text)
        print(f"\n{_USER_PROMPT_PREFIX} {text}")
        print(f"[turn:{turn_id}] thinking...")
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
        _print_event(evt)
        if evt.event_type == "approval_requested" and evt.turn_id:
            request_id = str(evt.payload.get("request_id", ""))
            if request_id:
                state.pending_approvals[request_id] = evt.turn_id
                state.approval_order.append(request_id)
                print(f"[approval] use /approve {request_id} y|n, or quick y/n")
                state.turn_phase = "approval"
                if state.input_ready is not None:
                    state.input_ready.set()
        if evt.event_type == "approval_resolved":
            request_id = str(evt.payload.get("request_id", ""))
            _pop_pending(state, request_id)
            if not state.pending_approvals:
                state.turn_phase = "thinking" if state.waiting_turn_id else "idle"
            if state.waiting_turn_id and not state.pending_approvals and state.input_ready is not None:
                state.input_ready.clear()
        if evt.event_type == "turn_completed" and evt.turn_id == state.waiting_turn_id:
            state.waiting_turn_id = None
            state.turn_phase = "idle"
            if state.input_ready is not None:
                state.input_ready.set()


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
        _record_tool_completed(
            turn=turn,
            tool_name=str(evt.payload.get("tool_name", "")),
            ok=bool(evt.payload.get("ok")),
        )
        return
    if t == "approval_requested":
        _finalize_stream_line_if_turn(turn)
        print(
            "[turn:{turn}] approval> id={rid} target={target} reason={reason}".format(
                turn=turn,
                rid=evt.payload.get("request_id"),
                target=evt.payload.get("target"),
                reason=evt.payload.get("reason"),
            )
        )
        return
    if t == "approval_resolved":
        _finalize_stream_line_if_turn(turn)
        print(
            f"[turn:{turn}] approval> id={evt.payload.get('request_id')} approved={evt.payload.get('approved')}"
        )
        return
    if t == "turn_completed":
        _finalize_stream_line_if_turn(turn)
        _print_tool_summary_for_turn(turn)
        print(f"[turn:{turn}] done")
        if state is not None:
            state.tool_turn_stats.pop(turn, None)
        return
    if t == "turn_started":
        return
    print(f"[turn:{turn}] {t}")


def _daemon_cmd_from_env() -> list[str]:
    cmd = os.environ.get("OPENJAX_DAEMON_CMD")
    if not cmd:
        return ["cargo", "run", "-q", "-p", "openjaxd"]
    return cmd.split()


def _select_input_backend() -> str:
    if os.environ.get("OPENJAX_TUI_INPUT_BACKEND", "").lower() == "basic":
        return "basic"
    if (
        PromptSession is not None
        and patch_stdout is not None
        and sys.stdin.isatty()
        and sys.stdout.isatty()
    ):
        return "prompt_toolkit"
    return "basic"


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
                line = input(f"{_USER_PROMPT_PREFIX} ")
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
    print("  text                submit turn")
    print("  /approve <id> y|n   resolve a specific approval")
    print("  y | n               resolve latest pending approval")
    print("  /pending            show pending approvals")
    print("  /help               show help")
    print("  /exit               exit")


def _print_status_bar(state: AppState) -> None:
    line = (
        f"[status] session={state.session_id or '-'}  backend={state.input_backend}  "
        f"phase={state.turn_phase}  approvals={len(state.pending_approvals)}"
    )
    print(line)
    print(_divider_line())


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
        turn_id = state.pending_approvals.get(request_id)
        if turn_id:
            print(f"  {request_id} (turn:{turn_id})")


async def _resolve_latest_approval(
    client: OpenJaxAsyncClient, state: AppState, approved: bool
) -> None:
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
    turn_id = state.pending_approvals.get(approval_request_id)
    if not turn_id:
        print(f"[approval] request not found: {approval_request_id}")
        return
    try:
        ok = await client.resolve_approval(
            turn_id=turn_id,
            request_id=approval_request_id,
            approved=approved,
            reason="approved_by_tui" if approved else "rejected_by_tui",
        )
        print(f"[approval] resolved: {approval_request_id} ok={ok}")
        _pop_pending(state, approval_request_id)
    except OpenJaxResponseError as err:
        print(f"[approval] resolve failed: {err.code} {err.message}")


def _pop_pending(state: AppState, approval_request_id: str) -> None:
    state.pending_approvals.pop(approval_request_id, None)
    with contextlib.suppress(ValueError):
        state.approval_order.remove(approval_request_id)


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


def _print_tool_summary_for_turn(turn: str) -> None:
    state = _active_state
    if state is None:
        return
    stats = state.tool_turn_stats.get(turn)
    if stats is None or stats.calls == 0:
        return

    tools = ", ".join(sorted(stats.tools)) if stats.tools else "-"
    duration = f"{stats.known_duration_ms}ms" if stats.known_duration_ms else "n/a"
    print(
        f"[turn:{turn}] tool> calls={stats.calls} ok={stats.ok_count} "
        f"fail={stats.fail_count} duration={duration} tools=[{tools}]"
    )


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
    if state.stream_turn_id != turn:
        _finalize_stream_line()
        state.stream_turn_id = turn
        state.stream_text_by_turn[turn] = ""
        print(f"{_ASSISTANT_PREFIX} ", end="", flush=True)
    state.stream_text_by_turn[turn] = state.stream_text_by_turn.get(turn, "") + delta
    print(delta, end="", flush=True)


def _render_assistant_message(turn: str, content: str) -> None:
    state = _active_state
    if state is None:
        print(f"{_ASSISTANT_PREFIX} {content}")
        return

    if state.stream_turn_id == turn:
        streamed = state.stream_text_by_turn.get(turn, "")
        if streamed == content:
            _finalize_stream_line()
            return
        _finalize_stream_line()

    print(f"{_ASSISTANT_PREFIX} {content}")


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
        print()
        current.stream_turn_id = None
