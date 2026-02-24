from __future__ import annotations

import asyncio
from collections import deque
from dataclasses import dataclass, field
from typing import Callable


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
        self.prompt_invalidator: Callable[[], None] | None = None
        self.history_blocks: list[str] = []
        self.stream_block_index: int | None = None
        self.history_setter: Callable[[], None] | None = None
        self.history_auto_follow: bool = True
        self.history_manual_scroll: int = 0


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
