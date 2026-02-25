from __future__ import annotations

import asyncio
import os
import queue
import unicodedata
from dataclasses import dataclass
from typing import Any, Callable

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxResponseError

from .input_backend import normalize_input as _normalize_input, start_basic_input_worker as _start_basic_input_worker


# 输入循环常量
INPUT_REQUEST = object()
INPUT_STOP = object()
USER_PROMPT_PREFIX = "❯"


@dataclass
class InputLoopCallbacks:
    """输入循环所需的回调函数集合。"""

    # 审批相关
    approval_mode_active: Callable[[Any], bool]
    focused_approval_id: Callable[[Any], str | None]
    resolve_approval_by_id: Callable[..., Any]
    resolve_latest_approval: Callable[..., Any]
    use_inline_approval_panel: Callable[[Any], bool]
    # UI 相关
    emit_ui_line: Callable[..., None]
    print_pending: Callable[[Any], None]
    # 日志相关
    tui_log_approval_event: Callable[..., None]
    # 动画相关
    sync_status_animation_controller: Callable[[Any], None]


async def run_input_loop_basic(
    client: OpenJaxAsyncClient,
    state: Any,
    callbacks: InputLoopCallbacks,
    *,
    handle_user_line_fn: Callable[[OpenJaxAsyncClient, Any, str], Any],
    input_request: object = INPUT_REQUEST,
    input_stop: object = INPUT_STOP,
    user_prompt_prefix: str = USER_PROMPT_PREFIX,
    active_state_getter: Callable[[], Any] | None = None,
) -> None:
    """运行 basic 输入循环。

    Args:
        client: OpenJax 异步客户端
        state: 应用状态
        callbacks: 回调函数集合
        handle_user_line_fn: 处理用户输入行的函数
        input_request: 输入请求信号对象
        input_stop: 输入停止信号对象
        user_prompt_prefix: 用户提示前缀
        active_state_getter: 获取活动状态的函数
    """
    if state.input_ready is None:
        raise RuntimeError("input gate is not initialized")

    line_queue: asyncio.Queue[str | None] = asyncio.Queue()
    request_queue: queue.Queue[object] = queue.Queue()
    _start_basic_input_worker(
        asyncio.get_running_loop(),
        request_queue,
        line_queue,
        input_request=input_request,
        input_stop=input_stop,
        user_prompt_prefix=user_prompt_prefix,
        active_state_getter=active_state_getter,
        approval_mode_active=callbacks.approval_mode_active,
    )

    while state.running:
        try:
            await state.input_ready.wait()
            request_queue.put_nowait(input_request)
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
        if not await handle_user_line_fn(client, state, line):
            return


async def handle_user_line(
    client: OpenJaxAsyncClient,
    state: Any,
    line: str,
    callbacks: InputLoopCallbacks,
    *,
    command_rows: tuple[str, ...] = (),
    slash_commands: tuple[str, ...] = (),
) -> bool:
    """处理用户输入的一行文本。

    Args:
        client: OpenJax 异步客户端
        state: 应用状态
        line: 用户输入的文本行
        callbacks: 回调函数集合
        command_rows: 命令帮助行
        slash_commands: 斜杠命令列表

    Returns:
        是否继续运行（False 表示退出）
    """
    text = _normalize_input(line).strip()
    if not text:
        if callbacks.approval_mode_active(state):
            approved = state.approval_selected_action == "allow"
            await callbacks.resolve_latest_approval(
                client, state, approved=approved,
                focused_approval_id_fn=callbacks.focused_approval_id,
                resolve_approval_by_id_fn=callbacks.resolve_approval_by_id,
            )
        return True
    if text == "/exit":
        state.running = False
        return False
    if text == "/help":
        _print_help(command_rows)
        return True
    if text == "/pending":
        callbacks.print_pending(state)
        return True

    if text in ("y", "n") and callbacks.approval_mode_active(state):
        await callbacks.resolve_latest_approval(
            client, state, approved=(text == "y"),
            focused_approval_id_fn=callbacks.focused_approval_id,
            resolve_approval_by_id_fn=callbacks.resolve_approval_by_id,
        )
        return True

    if text.startswith("/approve "):
        parts = text.split()
        if len(parts) != 3 or parts[2] not in ("y", "n"):
            print("usage: /approve <approval_request_id> <y|n>")
            return True
        await callbacks.resolve_approval_by_id(
            client,
            state,
            parts[1],
            parts[2] == "y",
        )
        return True

    if callbacks.approval_mode_active(state):
        if callbacks.use_inline_approval_panel(state):
            focus_id = callbacks.focused_approval_id(state)
            record = state.pending_approvals.get(focus_id or "")
            callbacks.tui_log_approval_event(
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
        callbacks.emit_ui_line(state, _format_user_message_bubble(text))

    try:
        turn_id = await client.submit_turn(text)
        state.waiting_turn_id = turn_id
        state.turn_phase = "thinking"
        callbacks.sync_status_animation_controller(state)
        if state.input_ready is not None:
            state.input_ready.clear()
    except OpenJaxResponseError as err:
        print(f"[error] submit failed: {err.code} {err.message}")

    return True


def _print_help(command_rows: tuple[str, ...]) -> None:
    """打印帮助信息。"""
    print("commands:")
    for row in command_rows:
        print(f"  {row}")


def daemon_cmd_from_env() -> list[str]:
    """从环境变量获取守护进程命令。

    Returns:
        守护进程命令列表
    """
    cmd = os.environ.get("OPENJAX_DAEMON_CMD")
    if not cmd:
        return ["cargo", "run", "-q", "-p", "openjaxd"]
    return cmd.split()


def _format_user_message_bubble(text: str) -> str:
    lines = text.splitlines() or [text]
    width = max((_display_width(line) for line in lines), default=0)
    width = max(width, 1)
    top = f"╭{'─' * width}╮"
    middle = [
        f"│{line}{' ' * max(0, width - _display_width(line))}│"
        for line in lines
    ]
    bottom = f"╰{'─' * width}╯"
    return "\n".join([top, *middle, bottom])


def _display_width(text: str) -> int:
    return sum(_char_display_width(ch) for ch in text)


def _char_display_width(ch: str) -> int:
    if _is_zero_width(ch):
        return 0
    codepoint = ord(ch)
    if (
        0x1F300 <= codepoint <= 0x1FAFF
        or 0x1F000 <= codepoint <= 0x1F02F
        or 0x2600 <= codepoint <= 0x27BF
    ):
        return 2
    return 2 if unicodedata.east_asian_width(ch) in {"W", "F"} else 1


def _is_zero_width(ch: str) -> bool:
    codepoint = ord(ch)
    if codepoint == 0x200D or 0xFE00 <= codepoint <= 0xFE0F:
        return True
    if unicodedata.combining(ch):
        return True
    category = unicodedata.category(ch)
    return category in {"Cf", "Cc", "Cs"}
