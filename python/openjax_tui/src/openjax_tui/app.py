from __future__ import annotations

import asyncio
import contextlib
import os
import re
from collections import deque

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxProtocolError, OpenJaxResponseError
from openjax_sdk.models import EventEnvelope


class AppState:
    def __init__(self) -> None:
        self.running = True
        self.pending_approvals: dict[str, str] = {}
        self.approval_order: deque[str] = deque()
        self.is_typing = False
        self.buffered_events: deque[EventEnvelope] = deque()
        self.stream_turn_id: str | None = None
        self.stream_text_by_turn: dict[str, str] = {}


async def run() -> None:
    _configure_readline_keybindings()
    daemon_cmd = _daemon_cmd_from_env()
    client = OpenJaxAsyncClient(daemon_cmd=daemon_cmd)
    state = AppState()
    _set_active_state(state)

    await client.start()
    try:
        session_id = await client.start_session()
        await client.stream_events()
        print(f"OpenJax TUI  session={session_id}")
        print(f"cwd={os.getcwd()}")
        _print_help()

        event_task = asyncio.create_task(_event_loop(client, state))
        try:
            await _input_loop(client, state)
        finally:
            state.running = False
            event_task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await event_task
    finally:
        if client.session_id:
            await client.shutdown_session()
        await client.stop()
        _finalize_stream_line(state)
        _set_active_state(None)
        print("openjax_tui exited")


async def _input_loop(client: OpenJaxAsyncClient, state: AppState) -> None:
    while state.running:
        try:
            state.is_typing = True
            line = await asyncio.to_thread(input, "> ")
        except EOFError:
            state.is_typing = False
            state.running = False
            return
        except KeyboardInterrupt:
            state.is_typing = False
            print("^C")
            state.running = False
            return
        finally:
            state.is_typing = False

        text = _normalize_input(line).strip()
        _flush_buffered_events(state)
        if not text:
            continue
        if text == "/exit":
            state.running = False
            return
        if text == "/help":
            _print_help()
            continue
        if text == "/pending":
            _print_pending(state)
            continue

        if text in ("y", "n"):
            await _resolve_latest_approval(client, state, approved=(text == "y"))
            continue

        if text.startswith("/approve "):
            parts = text.split()
            if len(parts) != 3 or parts[2] not in ("y", "n"):
                print("usage: /approve <approval_request_id> <y|n>")
                continue
            await _resolve_approval_by_id(
                client=client,
                state=state,
                approval_request_id=parts[1],
                approved=parts[2] == "y",
            )
            continue

        try:
            turn_id = await client.submit_turn(text)
            print(f"\nyou> {text}")
            print(f"[turn:{turn_id}] thinking...")
        except OpenJaxResponseError as err:
            print(f"[error] submit failed: {err.code} {err.message}")


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
        if state.is_typing:
            state.buffered_events.append(evt)
        else:
            _print_event(evt)
        if evt.event_type == "approval_requested" and evt.turn_id:
            request_id = str(evt.payload.get("request_id", ""))
            if request_id:
                state.pending_approvals[request_id] = evt.turn_id
                state.approval_order.append(request_id)
                print(f"[approval] use /approve {request_id} y|n, or quick y/n")
        if evt.event_type == "approval_resolved":
            request_id = str(evt.payload.get("request_id", ""))
            _pop_pending(state, request_id)


def _print_event(evt: EventEnvelope) -> None:
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
        print(f"[turn:{turn}] tool> {evt.payload.get('tool_name')} ...")
        return
    if t == "tool_call_completed":
        _finalize_stream_line_if_turn(turn)
        print(
            f"[turn:{turn}] tool> {evt.payload.get('tool_name')} ok={evt.payload.get('ok')}"
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
        print(f"[turn:{turn}] done")
        return
    if t == "turn_started":
        return
    print(f"[turn:{turn}] {t}")


def _daemon_cmd_from_env() -> list[str]:
    cmd = os.environ.get("OPENJAX_DAEMON_CMD")
    if not cmd:
        return ["cargo", "run", "-q", "-p", "openjaxd"]
    return cmd.split()


def _print_help() -> None:
    print("-" * 64)
    print("commands:")
    print("  text                submit turn")
    print("  /approve <id> y|n   resolve a specific approval")
    print("  y | n               resolve latest pending approval")
    print("  /pending            show pending approvals")
    print("  /help               show help")
    print("  /exit               exit")
    print("-" * 64)


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


def _flush_buffered_events(state: AppState) -> None:
    while state.buffered_events:
        _print_event(state.buffered_events.popleft())


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


_ACTIVE_STATE: AppState | None = None


def _set_active_state(state: AppState | None) -> None:
    global _ACTIVE_STATE
    _ACTIVE_STATE = state


def _render_assistant_delta(turn: str, delta: str) -> None:
    state = _ACTIVE_STATE
    if state is None:
        return
    if not delta:
        return
    if state.stream_turn_id != turn:
        _finalize_stream_line()
        state.stream_turn_id = turn
        state.stream_text_by_turn[turn] = ""
    state.stream_text_by_turn[turn] = state.stream_text_by_turn.get(turn, "") + delta
    text = state.stream_text_by_turn[turn]
    print(f"\rassistant> {text}", end="", flush=True)


def _render_assistant_message(turn: str, content: str) -> None:
    state = _ACTIVE_STATE
    if state is None:
        print(f"assistant> {content}")
        return

    if state.stream_turn_id == turn:
        streamed = state.stream_text_by_turn.get(turn, "")
        if streamed == content:
            _finalize_stream_line()
            return
        _finalize_stream_line()

    print(f"assistant> {content}")


def _finalize_stream_line_if_turn(turn: str) -> None:
    state = _ACTIVE_STATE
    if state is None:
        return
    if state.stream_turn_id == turn:
        _finalize_stream_line()


def _finalize_stream_line(state: AppState | None = None) -> None:
    current = state or _ACTIVE_STATE
    if current is None:
        return
    if current.stream_turn_id is not None:
        print()
        current.stream_turn_id = None
