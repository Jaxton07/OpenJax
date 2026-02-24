from __future__ import annotations

import contextlib
import os
from abc import ABC, abstractmethod
from typing import Any, Callable

from .state import AppState, ViewMode


class HistoryViewportAdapter(ABC):
    """历史视口适配器基类，提供统一的视口操作接口。"""

    def __init__(self, state: AppState) -> None:
        self.state = state

    @property
    @abstractmethod
    def container(self) -> Any:
        """返回视口容器对象。"""
        raise NotImplementedError

    def append_block(self, block: str) -> int:
        """添加一个新的历史块，返回块索引。"""
        self.state.history_blocks.append(block)
        return len(self.state.history_blocks) - 1

    def update_block(self, index: int, block: str) -> None:
        """更新指定索引的历史块内容。"""
        if 0 <= index < len(self.state.history_blocks):
            self.state.history_blocks[index] = block

    @abstractmethod
    def set_text(self, text: str) -> None:
        """设置视口显示的文本内容。"""
        raise NotImplementedError

    @abstractmethod
    def current_scroll(self) -> int:
        """获取当前滚动位置。"""
        raise NotImplementedError

    @abstractmethod
    def max_scroll(self) -> int | None:
        """获取最大滚动位置，返回 None 表示无法确定。"""
        raise NotImplementedError

    @abstractmethod
    def apply_scroll(self) -> None:
        """应用当前滚动状态到视口。"""
        raise NotImplementedError

    def refresh(self, text: str) -> None:
        """刷新视口内容并应用滚动。"""
        self.set_text(text)
        self.apply_scroll()

    def set_manual_scroll(self, value: int) -> None:
        """设置手动滚动位置并禁用自动跟随。"""
        self.state.history_manual_scroll = max(0, int(value))
        self.state.history_auto_follow = False
        self.apply_scroll()

    def follow_tail(self) -> None:
        """启用自动跟随模式（滚动到末尾）。"""
        self.state.history_auto_follow = True
        self.state.history_manual_scroll = max(0, int(self.current_scroll()))
        self.apply_scroll()

    def sync_manual_scroll_from_render(self) -> None:
        """从渲染状态同步手动滚动位置。"""
        self.state.history_manual_scroll = self.current_scroll()
        if self.state.history_auto_follow:
            return
        window_max = self.max_scroll()
        if window_max is None:
            return
        if self.state.history_manual_scroll >= window_max:
            self.follow_tail()


class TextAreaHistoryViewportAdapter(HistoryViewportAdapter):
    """基于 prompt_toolkit TextArea 的视口适配器实现。"""

    def __init__(self, state: AppState, history_view: Any, *, document_cls: Any) -> None:
        super().__init__(state)
        self._history_view = history_view
        self._history_window = getattr(history_view, "window", None)
        self._document_cls = document_cls

    @property
    def container(self) -> Any:
        return self._history_view

    def set_text(self, text: str) -> None:
        if self._document_cls is None:
            return
        history_buffer = getattr(self._history_view, "buffer", None)
        if history_buffer is None:
            return
        if self.state.history_auto_follow:
            cursor_position = len(text)
        else:
            current_cursor = int(getattr(history_buffer, "cursor_position", 0))
            cursor_position = min(max(current_cursor, 0), len(text))
        with contextlib.suppress(Exception):
            history_buffer.set_document(
                self._document_cls(text=text, cursor_position=cursor_position),
                bypass_readonly=True,
            )

    def current_scroll(self) -> int:
        if self._history_window is None:
            return max(0, int(self.state.history_manual_scroll))
        render_info = getattr(self._history_window, "render_info", None)
        if render_info is not None:
            with contextlib.suppress(Exception):
                return max(0, int(getattr(render_info, "vertical_scroll", 0)))
        return max(0, int(self.state.history_manual_scroll))

    def max_scroll(self) -> int | None:
        if self._history_window is None:
            return None
        render_info = getattr(self._history_window, "render_info", None)
        if render_info is None:
            return None
        with contextlib.suppress(Exception):
            content_height = int(getattr(render_info, "content_height", 0))
            window_height = int(getattr(render_info, "window_height", 0))
            return max(content_height - window_height, 0)
        return None

    def apply_scroll(self) -> None:
        if self._history_window is None:
            return
        with contextlib.suppress(Exception):
            if self.state.history_auto_follow:
                self._history_window.vertical_scroll = 10**9
            else:
                self._history_window.vertical_scroll = max(0, int(self.state.history_manual_scroll))


class PilotHistoryViewportAdapter(HistoryViewportAdapter):
    """基于 Pilot 模式的视口适配器实现（用于 LIVE_VIEWPORT 模式）。"""

    def __init__(
        self,
        state: AppState,
        history_window: Any,
        *,
        set_text_fn: Callable[[str], None],
    ) -> None:
        super().__init__(state)
        self._history_window = history_window
        self._set_text_fn = set_text_fn

    @property
    def container(self) -> Any:
        return self._history_window

    def set_text(self, text: str) -> None:
        self._set_text_fn(text)

    def current_scroll(self) -> int:
        render_info = getattr(self._history_window, "render_info", None)
        if render_info is not None:
            with contextlib.suppress(Exception):
                return max(0, int(getattr(render_info, "vertical_scroll", 0)))
        return max(0, int(self.state.history_manual_scroll))

    def max_scroll(self) -> int | None:
        render_info = getattr(self._history_window, "render_info", None)
        if render_info is None:
            return None
        with contextlib.suppress(Exception):
            content_height = int(getattr(render_info, "content_height", 0))
            window_height = int(getattr(render_info, "window_height", 0))
            return max(content_height - window_height, 0)
        return None

    def apply_scroll(self) -> None:
        with contextlib.suppress(Exception):
            if self.state.history_auto_follow:
                self._history_window.vertical_scroll = 10**9
            else:
                self._history_window.vertical_scroll = max(0, int(self.state.history_manual_scroll))


def resolve_history_viewport_impl() -> str:
    """解析历史视口实现类型（从环境变量或默认值）。

    Returns:
        "textarea" 或 "pilot"
    """
    requested = os.environ.get("OPENJAX_TUI_HISTORY_VIEWPORT_IMPL", "pilot")
    normalized = requested.strip().lower()
    if normalized == "textarea":
        return "textarea"
    return "pilot"


def _retain_live_viewport_blocks(state: AppState) -> list[str]:
    """在 LIVE_VIEWPORT 模式下，仅保留当前活动 turn 的历史块。

    Args:
        state: 应用状态

    Returns:
        被移除的历史块列表
    """
    if state.view_mode != ViewMode.LIVE_VIEWPORT:
        return []
    if not state.history_blocks:
        return []

    keep_index: int | None = None
    if (
        state.stream_turn_id is not None
        and state.stream_block_index is not None
        and 0 <= state.stream_block_index < len(state.history_blocks)
    ):
        keep_index = state.stream_block_index

    dropped_blocks: list[str] = []
    if keep_index is None:
        dropped_blocks = list(state.history_blocks)
        state.history_blocks = []
        state.turn_block_index = {}
        state.stream_block_index = None
        state.history_manual_scroll = 0
        return dropped_blocks

    retained_block = state.history_blocks[keep_index]
    for idx, block in enumerate(state.history_blocks):
        if idx != keep_index:
            dropped_blocks.append(block)
    state.history_blocks = [retained_block]

    if state.stream_turn_id is not None:
        state.turn_block_index = {state.stream_turn_id: 0}
        state.stream_block_index = 0
    else:
        state.turn_block_index = {}
        state.stream_block_index = None
    state.history_manual_scroll = 0
    return dropped_blocks


def retain_live_viewport_blocks(state: AppState) -> list[str]:
    """公共接口：在 LIVE_VIEWPORT 模式下保留活动视口块。

    Args:
        state: 应用状态

    Returns:
        被移除的历史块列表
    """
    return _retain_live_viewport_blocks(state)
