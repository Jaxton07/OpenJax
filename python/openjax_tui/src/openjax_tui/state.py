from __future__ import annotations

import asyncio
from collections import deque
from dataclasses import dataclass, field
from enum import Enum
from typing import Callable


class ViewMode(str, Enum):
    SESSION = "session"
    LIVE_VIEWPORT = "live_viewport"


class AnimationLifecycle(str, Enum):
    IDLE = "idle"
    PREPARING = "preparing"
    ACTIVE = "active"
    SETTLING = "settling"


class LiveViewportOwnership(str, Enum):
    ACTIVE = "active"
    RELEASED = "released"


class AppState:
    def __init__(self) -> None:
        self.running: bool = True
        self.pending_approvals: dict[str, ApprovalRecord] = {}
        self.approval_order: deque[str] = deque()
        self.approval_focus_id: str | None = None
        self.approval_ui_enabled: bool = False
        self.approval_selected_action: str = "allow"
        self.stream_turn_id: str | None = None
        self.stream_text_by_turn: dict[str, str] = {}
        self.turn_block_index: dict[str, int] = {}
        self.assistant_message_by_turn: dict[str, str] = {}
        self.waiting_turn_id: str | None = None
        self.input_ready: asyncio.Event | None = None
        self.approval_interrupt: asyncio.Event | None = None
        self.session_id: str | None = None
        self.input_backend: str = "basic"
        self.input_backend_reason: str = ""
        self.turn_phase: str = "idle"
        self.tool_turn_stats: dict[str, ToolTurnStats] = {}
        self.active_tool_starts: dict[tuple[str, str], list[float]] = {}
        self.active_tool_display_label_by_turn: dict[str, str] = {}
        self.prompt_invalidator: Callable[[], None] | None = None
        self.history_blocks: list[str] = []
        self.stream_block_index: int | None = None
        self.last_basic_ui_block_emitted: bool = False
        self.history_setter: Callable[[], None] | None = None
        self.history_auto_follow: bool = True
        self.history_manual_scroll: int = 0
        self.last_scrollback_flush_emitted: bool = False
        self.approval_flash_message: str = ""
        self.approval_flash_until: float = 0.0
        self.approval_flash_clear_handle: object | None = None
        self.view_mode: ViewMode = ViewMode.LIVE_VIEWPORT
        self.animation_lifecycle: AnimationLifecycle = AnimationLifecycle.IDLE
        self.animation_task: asyncio.Task[None] | None = None
        self.animation_frame_index: int = 0
        self.live_viewport_owner_turn_id: str | None = None
        self.live_viewport_turn_ownership: dict[str, LiveViewportOwnership] = {}

    @staticmethod
    def normalize_view_mode(mode: str | ViewMode | None) -> ViewMode:
        if isinstance(mode, ViewMode):
            return mode
        if mode is None:
            return ViewMode.LIVE_VIEWPORT
        normalized = str(mode).strip().lower()
        if normalized == "live":
            return ViewMode.LIVE_VIEWPORT
        for candidate in ViewMode:
            if candidate.value == normalized:
                return candidate
        return ViewMode.SESSION

    def set_view_mode(self, mode: str | ViewMode | None) -> ViewMode:
        self.view_mode = self.normalize_view_mode(mode)
        return self.view_mode


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
