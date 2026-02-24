"""事件状态管理模块。

管理事件驱动的状态更新，包括 turn phase 转换、审批状态管理和 live viewport 所有权。
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Callable

if TYPE_CHECKING:
    from openjax_sdk.models import EventEnvelope
    from .state import AppState


def get_active_tool_call_count(state: AppState, turn_id: str) -> int:
    """获取指定 turn 的活动工具调用计数。

    Args:
        state: 应用状态
        turn_id: Turn ID

    Returns:
        活动工具调用数量
    """
    count = 0
    for key, starts in state.active_tool_starts.items():
        if key[0] == turn_id:
            count += len(starts)
    return count


def has_active_tool_calls_after_event(state: AppState, evt: EventEnvelope) -> bool:
    """检查事件处理后是否仍有活动工具调用。

    Args:
        state: 应用状态
        evt: 事件信封

    Returns:
        是否仍有活动工具调用
    """
    turn_id = evt.turn_id
    if not turn_id:
        return False

    active_count = get_active_tool_call_count(state, turn_id)
    if evt.event_type == "tool_call_started":
        tool_name = str(evt.payload.get("tool_name", ""))
        starts = state.active_tool_starts.get((turn_id, tool_name))
        if not starts:
            active_count += 1
    elif evt.event_type == "tool_call_completed":
        tool_name = str(evt.payload.get("tool_name", ""))
        starts = state.active_tool_starts.get((turn_id, tool_name))
        if starts:
            active_count = max(0, active_count - 1)
    return active_count > 0


def tool_wait_display_label(tool_name: str) -> str:
    normalized = tool_name.strip().lower()
    if normalized == "read_file":
        return "Reading"
    if normalized == "list_dir":
        return "Scanning"
    if normalized == "grep_files":
        return "Searching"
    if normalized == "shell":
        return "Running"
    return "Working"


def update_live_viewport_ownership(state: AppState, evt: EventEnvelope) -> None:
    """更新 live viewport 所有权状态。

    Args:
        state: 应用状态
        evt: 事件信封
    """
    from .state import LiveViewportOwnership, ViewMode

    if state.view_mode != ViewMode.LIVE_VIEWPORT:
        return

    turn_id = evt.turn_id
    if not turn_id:
        return

    if evt.event_type == "assistant_delta":
        state.live_viewport_owner_turn_id = turn_id
        state.live_viewport_turn_ownership[turn_id] = LiveViewportOwnership.ACTIVE
        return

    if evt.event_type in {"assistant_message", "turn_completed"}:
        state.live_viewport_turn_ownership[turn_id] = LiveViewportOwnership.RELEASED
        if state.live_viewport_owner_turn_id == turn_id:
            state.live_viewport_owner_turn_id = None


class EventStateManager:
    """事件状态管理器。

    负责应用事件驱动的状态更新。
    """

    def __init__(
        self,
        state: AppState,
        callbacks: EventStateCallbacks,
    ) -> None:
        """初始化事件状态管理器。

        Args:
            state: 应用状态
            callbacks: 回调函数集合
        """
        self.state = state
        self.callbacks = callbacks

    def apply_event_updates(self, evt: EventEnvelope) -> None:
        """应用事件到状态。

        Args:
            evt: 事件信封
        """
        update_live_viewport_ownership(self.state, evt)
        self._update_tool_display_state(evt)
        self._update_turn_phase(evt)
        self._handle_approval_events(evt)
        self._handle_turn_completion(evt)

    def _update_tool_display_state(self, evt: EventEnvelope) -> None:
        turn_id = evt.turn_id
        if not turn_id:
            return
        if evt.event_type == "tool_call_started":
            tool_name = str(evt.payload.get("tool_name", ""))
            self.state.active_tool_display_label_by_turn[turn_id] = tool_wait_display_label(tool_name)
            return
        if evt.event_type == "tool_call_completed":
            if not has_active_tool_calls_after_event(self.state, evt):
                self.state.active_tool_display_label_by_turn.pop(turn_id, None)
            return
        if evt.event_type == "turn_completed":
            self.state.active_tool_display_label_by_turn.pop(turn_id, None)

    def _update_turn_phase(self, evt: EventEnvelope) -> None:
        """更新 turn phase 状态。

        Args:
            evt: 事件信封
        """
        if not evt.turn_id or evt.turn_id != self.state.waiting_turn_id:
            return

        if evt.event_type == "tool_call_started":
            self.state.turn_phase = "tool_wait"
            self.callbacks.sync_animation()
        elif evt.event_type == "tool_call_completed":
            self.state.turn_phase = (
                "tool_wait" if has_active_tool_calls_after_event(self.state, evt) else "thinking"
            )
            self.callbacks.sync_animation()
        elif evt.event_type in {"assistant_delta", "assistant_message"}:
            if self.state.turn_phase != "approval":
                self.state.turn_phase = "thinking"
            self.callbacks.sync_animation()

    def _handle_approval_events(self, evt: EventEnvelope) -> None:
        """处理审批相关事件。

        Args:
            evt: 事件信封
        """
        if evt.event_type == "approval_requested" and evt.turn_id:
            self._handle_approval_requested(evt)
        elif evt.event_type == "approval_resolved":
            self._handle_approval_resolved(evt)

    def _handle_approval_requested(self, evt: EventEnvelope) -> None:
        """处理审批请求事件。

        Args:
            evt: 事件信封
        """
        from .state import ApprovalRecord

        request_id = str(evt.payload.get("request_id", ""))
        if not request_id:
            return

        record = ApprovalRecord(
            turn_id=evt.turn_id,
            target=str(evt.payload.get("target", "")),
            reason=str(evt.payload.get("reason", "")),
        )
        self.state.pending_approvals[request_id] = record
        if request_id not in self.state.approval_order:
            self.state.approval_order.append(request_id)
        self.state.approval_focus_id = request_id
        self.state.approval_selected_action = "allow"

        self.callbacks.log_approval_event(
            action="requested",
            request_id=request_id,
            turn_id=evt.turn_id,
            target=record.target,
            approved=None,
            resolved=None,
            detail="event_received",
        )

        if not self.callbacks.use_inline_approval_panel(self.state):
            print(
                f"[approval] use /approve {request_id} y|n, quick y/n, or press Enter to confirm default allow"
            )

        self.state.turn_phase = "approval"
        self.callbacks.sync_animation()

        if self.state.input_ready is not None:
            self.state.input_ready.set()
        if self.state.approval_interrupt is not None:
            self.state.approval_interrupt.set()

        self.callbacks.debug_log(
            f"approval state updated request_id={request_id} pending={len(self.state.pending_approvals)}"
        )
        self.callbacks.request_redraw()

    def _handle_approval_resolved(self, evt: EventEnvelope) -> None:
        """处理审批解决事件。

        Args:
            evt: 事件信封
        """
        request_id = str(evt.payload.get("request_id", ""))
        record = self.state.pending_approvals.get(request_id)
        approved = evt.payload.get("approved")
        approved_bool = approved if isinstance(approved, bool) else None

        self.callbacks.log_approval_event(
            action="resolved_event",
            request_id=request_id,
            turn_id=record.turn_id if record else evt.turn_id,
            target=record.target if record else None,
            approved=approved_bool,
            resolved=True,
            detail="event_received",
        )

        self.callbacks.pop_pending(request_id)

        if not self.state.pending_approvals:
            self.state.turn_phase = "thinking" if self.state.waiting_turn_id else "idle"

        self.callbacks.sync_animation()

        if self.state.waiting_turn_id and not self.state.pending_approvals and self.state.input_ready is not None:
            self.state.input_ready.clear()

        self.callbacks.debug_log(
            f"approval resolved request_id={request_id} pending={len(self.state.pending_approvals)} phase={self.state.turn_phase}"
        )
        self.callbacks.request_redraw()

    def _handle_turn_completion(self, evt: EventEnvelope) -> None:
        """处理 turn 完成事件。

        Args:
            evt: 事件信封
        """
        if evt.event_type != "turn_completed" or evt.turn_id != self.state.waiting_turn_id:
            return

        self.state.active_tool_display_label_by_turn.pop(evt.turn_id, None)
        self.state.waiting_turn_id = None
        self.state.turn_phase = "idle"
        self.callbacks.sync_animation()

        if self.state.input_ready is not None:
            self.state.input_ready.set()

        self.callbacks.debug_log("turn completed; input gate reopened")

        # Final safety net for prompt_toolkit: ensure completed turn content is
        if self.state.history_setter is not None and (
            self.state.history_auto_follow or self.callbacks.is_live_viewport_mode()
        ):
            self.state.history_setter()
        self.callbacks.request_redraw()

        self.callbacks.request_redraw()


class EventStateCallbacks:
    """事件状态管理器回调函数集合。"""

    def __init__(
        self,
        sync_animation: Callable[[], None],
        request_redraw: Callable[[], None],
        log_approval_event: Callable[..., None],
        pop_pending: Callable[[str], None],
        use_inline_approval_panel: Callable[[AppState], bool],
        debug_log: Callable[[str], None],
        is_live_viewport_mode: Callable[[], bool],
    ) -> None:
        """初始化回调集合。

        Args:
            sync_animation: 同步动画控制器
            request_redraw: 请求重绘
            log_approval_event: 记录审批事件
            pop_pending: 移除待处理审批
            use_inline_approval_panel: 是否使用内联审批面板
            debug_log: 调试日志
            is_live_viewport_mode: 是否为 live viewport 模式
        """
        self.sync_animation = sync_animation
        self.request_redraw = request_redraw
        self.log_approval_event = log_approval_event
        self.pop_pending = pop_pending
        self.use_inline_approval_panel = use_inline_approval_panel
        self.debug_log = debug_log
        self.is_live_viewport_mode = is_live_viewport_mode
