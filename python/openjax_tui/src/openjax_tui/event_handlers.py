"""事件处理适配器模块。

提供事件处理和状态适配的包装函数，简化 app.py 中的事件分发逻辑。
"""

from __future__ import annotations

import time
from typing import TYPE_CHECKING, Any, Callable

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
from .prompt_ui import refresh_history_view as _refresh_history_view
from .startup_ui import _supports_ansi_color
from .tool_runtime import (
    emit_ui_spacer as _emit_ui_spacer,
    print_tool_call_result_line as _print_tool_call_result_line,
    print_tool_summary_for_turn as _print_tool_summary_for_turn,
    record_tool_completed as _record_tool_completed,
    record_tool_started as _record_tool_started,
    status_bullet as _status_bullet,
)

if TYPE_CHECKING:
    from .state import AppState, ToolTurnStats


# 常量
_ASSISTANT_PREFIX = "⏺"
_PREFIX_CONTINUATION = "  "
_ANSI_GREEN = "\x1b[32m"
_ANSI_RED = "\x1b[31m"
_ANSI_RESET = "\x1b[0m"


def create_emit_ui_line_fn(state: AppState) -> Callable[..., None]:
    """创建 emit_ui_line 的闭包函数。

    注意：函数签名兼容 tool_runtime 的调用方式，第一个 _state 参数被忽略。
    """
    def _emit_ui_line_fn(_state: Any, text: str) -> None:
        _ = _state  # _state 是调用者传递的，但我们使用闭包中的 state
        _emit_ui_line(
            state,
            text,
            refresh_history_view_fn=_refresh_history_view,
        )
    return _emit_ui_line_fn


def create_print_prefixed_block_fn(state: AppState) -> Callable[[str, str], None]:
    """创建 print_prefixed_block 的闭包函数。"""
    emit_ui_line_fn = create_emit_ui_line_fn(state)

    def _print_prefixed_block_fn(prefix: str, content: str) -> None:
        _print_prefixed_block(
            state,
            prefix,
            content,
            align_multiline_fn=lambda text: _align_multiline(text, _PREFIX_CONTINUATION),
            emit_ui_line_fn=emit_ui_line_fn,
        )
    return _print_prefixed_block_fn


def create_status_bullet_fn(state: AppState) -> Callable[[bool], str]:
    """创建 status_bullet 的闭包函数。"""
    def _status_bullet_fn(ok: bool) -> str:
        return _status_bullet(
            state=state,
            ok=ok,
            assistant_prefix=_ASSISTANT_PREFIX,
            ansi_green=_ANSI_GREEN,
            ansi_red=_ANSI_RED,
            ansi_reset=_ANSI_RESET,
            supports_ansi_color_fn=_supports_ansi_color,
        )
    return _status_bullet_fn


def create_render_assistant_delta_fn(state: AppState) -> Callable[[str, str], None]:
    """创建 render_assistant_delta 的闭包函数。"""
    def _render_assistant_delta_fn(turn: str, delta: str) -> None:
        _render_assistant_delta(
            state,
            turn,
            delta,
            assistant_prefix=_ASSISTANT_PREFIX,
            align_multiline_fn=lambda text: _align_multiline(text, _PREFIX_CONTINUATION),
            finalize_stream_line_fn=_finalize_stream_line,
            refresh_history_view_fn=_refresh_history_view,
        )
    return _render_assistant_delta_fn


def create_render_assistant_message_fn(state: AppState) -> Callable[[str, str], None]:
    """创建 render_assistant_message 的闭包函数。"""
    print_prefixed_block_fn = create_print_prefixed_block_fn(state)

    def _render_assistant_message_fn(turn: str, content: str) -> None:
        _render_assistant_message(
            state,
            turn,
            content,
            assistant_prefix=_ASSISTANT_PREFIX,
            print_prefixed_block_fn=print_prefixed_block_fn,
            finalize_stream_line_fn=_finalize_stream_line,
        )
    return _render_assistant_message_fn


def create_finalize_stream_line_if_turn_fn(state: AppState) -> Callable[[str], None]:
    """创建 finalize_stream_line_if_turn 的闭包函数。"""
    def _finalize_stream_line_if_turn_fn(turn: str) -> None:
        _finalize_stream_line_if_turn(
            state,
            turn,
            finalize_stream_line_fn=_finalize_stream_line,
        )
    return _finalize_stream_line_if_turn_fn


def create_record_tool_started_fn(state: AppState) -> Callable[[str, str], None]:
    """创建 record_tool_started 的闭包函数。"""
    def _record_tool_started_fn(turn: str, tool_name: str) -> None:
        _record_tool_started(
            state,
            turn,
            tool_name,
            monotonic_fn=time.monotonic,
        )
    return _record_tool_started_fn


def create_record_tool_completed_fn(
    state: AppState,
) -> Callable[[str, str, bool], int]:
    """创建 record_tool_completed 的闭包函数。"""
    def _record_tool_completed_fn(turn: str, tool_name: str, ok: bool) -> int:
        from .state import ToolTurnStats

        return _record_tool_completed(
            state,
            turn,
            tool_name,
            ok,
            monotonic_fn=time.monotonic,
            tool_turn_stats_cls=ToolTurnStats,
        )
    return _record_tool_completed_fn


def create_print_tool_call_result_line_fn(
    state: AppState,
) -> Callable[..., None]:
    """创建 print_tool_call_result_line 的闭包函数。

    注意：函数签名与 event_dispatch.py 兼容，第一个 _state 参数被忽略。
    """
    status_bullet_fn = create_status_bullet_fn(state)
    emit_ui_line_fn = create_emit_ui_line_fn(state)

    def _print_tool_call_result_line_fn(
        _state: Any, tool_name: str, ok: bool, output: str, *, elapsed_ms: int = 0, target_hint: str | None = None
    ) -> None:
        _ = _state  # _state 是 event_dispatch 传递的，但我们使用闭包中的 state
        _print_tool_call_result_line(
            state,
            tool_name,
            ok,
            output,
            status_bullet_fn=status_bullet_fn,
            tool_result_label_fn=_tool_result_label,
            finalize_stream_line_fn=_finalize_stream_line,
            emit_ui_spacer_fn=_emit_ui_spacer,
            emit_ui_line_fn=emit_ui_line_fn,
            elapsed_ms=elapsed_ms,
            target_hint=target_hint,
        )
    return _print_tool_call_result_line_fn


def create_print_tool_summary_for_turn_fn(
    state: AppState,
) -> Callable[[Any, str], None]:
    """创建 print_tool_summary_for_turn 的闭包函数。"""
    status_bullet_fn = create_status_bullet_fn(state)
    emit_ui_line_fn = create_emit_ui_line_fn(state)

    def _print_tool_summary_for_turn_fn(_state: Any, turn: str) -> None:
        _print_tool_summary_for_turn(
            state,
            turn,
            status_bullet_fn=status_bullet_fn,
            finalize_stream_line_fn=_finalize_stream_line,
            emit_ui_line_fn=emit_ui_line_fn,
        )
    return _print_tool_summary_for_turn_fn
