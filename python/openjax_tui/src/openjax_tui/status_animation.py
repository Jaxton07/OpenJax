from __future__ import annotations

import asyncio
from collections.abc import Awaitable
from typing import Callable

from .state import AnimationLifecycle, AppState

# 动画常量
STATUS_ANIMATION_INTERVAL_S: float = 1.0 / 7.0
THINKING_STATUS_FRAMES: tuple[str, ...] = ("", ".", "..", "...")
TOOL_WAIT_STATUS_FRAMES: tuple[str, ...] = ("|", "/", "-", "\\")


def _status_animation_phase(state: AppState) -> str | None:
    """获取当前状态动画阶段。

    Args:
        state: 应用状态

    Returns:
        "thinking", "tool_wait" 或 None
    """
    if state.turn_phase == "thinking":
        return "thinking"
    if state.turn_phase == "tool_wait":
        return "tool_wait"
    return None


def _status_animation_frame_count(state: AppState) -> int:
    """获取当前动画阶段的帧数。

    Args:
        state: 应用状态

    Returns:
        帧数
    """
    phase = _status_animation_phase(state)
    if phase == "tool_wait":
        return len(TOOL_WAIT_STATUS_FRAMES)
    return len(THINKING_STATUS_FRAMES)


def get_status_indicator_text(state: AppState) -> str:
    """获取当前状态指示器文本。

    Args:
        state: 应用状态

    Returns:
        状态指示器文本
    """
    phase = _status_animation_phase(state)
    if phase is None:
        return ""
    if phase == "tool_wait":
        frame = TOOL_WAIT_STATUS_FRAMES[
            state.animation_frame_index % len(TOOL_WAIT_STATUS_FRAMES)
        ]
        return f" status: waiting for tool results {frame}"
    frame = THINKING_STATUS_FRAMES[
        state.animation_frame_index % len(THINKING_STATUS_FRAMES)
    ]
    return f" status: thinking{frame}"


def should_run_animation(state: AppState) -> bool:
    """检查是否应该运行动画。

    Args:
        state: 应用状态

    Returns:
        是否应该运行动画
    """
    return (
        state.running
        and state.input_backend == "prompt_toolkit"
        and state.prompt_invalidator is not None
        and _status_animation_phase(state) is not None
    )


async def run_animation_ticker(
    state: AppState,
    *,
    sleep_fn: Callable[[float], Awaitable[None]] = asyncio.sleep,
    request_prompt_redraw_fn: Callable[[], None] | None = None,
) -> None:
    """运行状态动画 tick 循环。

    Args:
        state: 应用状态
        sleep_fn: 睡眠函数（用于测试注入）
        request_prompt_redraw_fn: 请求重绘的回调函数（无参数）
    """
    state.animation_lifecycle = AnimationLifecycle.ACTIVE
    try:
        while should_run_animation(state):
            await sleep_fn(STATUS_ANIMATION_INTERVAL_S)
            if not should_run_animation(state):
                break
            frame_count = _status_animation_frame_count(state)
            state.animation_frame_index = (state.animation_frame_index + 1) % frame_count
            if request_prompt_redraw_fn is not None:
                request_prompt_redraw_fn()
    finally:
        if state.animation_task is asyncio.current_task():
            state.animation_task = None
        if not should_run_animation(state):
            state.animation_lifecycle = AnimationLifecycle.IDLE
            state.animation_frame_index = 0


def start_animation(
    state: AppState,
    *,
    request_prompt_redraw_fn: Callable[[], None] | None = None,
) -> None:
    """启动状态动画。

    Args:
        state: 应用状态
        request_prompt_redraw_fn: 请求重绘的回调函数（无参数）
    """
    if not should_run_animation(state):
        return
    task = state.animation_task
    if task is not None and not task.done():
        return
    state.animation_lifecycle = AnimationLifecycle.PREPARING
    state.animation_frame_index = 0
    state.animation_task = asyncio.create_task(
        run_animation_ticker(state, request_prompt_redraw_fn=request_prompt_redraw_fn)
    )
    if request_prompt_redraw_fn is not None:
        request_prompt_redraw_fn()


def stop_animation(
    state: AppState,
    *,
    request_prompt_redraw_fn: Callable[[], None] | None = None,
) -> None:
    """停止状态动画。

    Args:
        state: 应用状态
        request_prompt_redraw_fn: 请求重绘的回调函数（无参数）
    """
    task = state.animation_task
    previous_lifecycle = state.animation_lifecycle
    had_frame = state.animation_frame_index != 0
    state.animation_task = None
    if task is not None and not task.done():
        state.animation_lifecycle = AnimationLifecycle.SETTLING
        task.cancel()
    else:
        state.animation_lifecycle = AnimationLifecycle.IDLE
    if had_frame:
        state.animation_frame_index = 0
    if (
        (task is not None and not task.done())
        or had_frame
        or previous_lifecycle != state.animation_lifecycle
    ):
        if request_prompt_redraw_fn is not None:
            request_prompt_redraw_fn()


def sync_animation_controller(
    state: AppState,
    *,
    request_prompt_redraw_fn: Callable[[], None] | None = None,
) -> None:
    """同步状态动画控制器。

    根据当前状态决定是否启动或停止动画。

    Args:
        state: 应用状态
        request_prompt_redraw_fn: 请求重绘的回调函数（无参数）
    """
    if should_run_animation(state):
        start_animation(state, request_prompt_redraw_fn=request_prompt_redraw_fn)
        return
    stop_animation(state, request_prompt_redraw_fn=request_prompt_redraw_fn)
