"""事件状态管理器测试。"""

from __future__ import annotations

import asyncio
import unittest
from unittest.mock import MagicMock

from openjax_sdk.models import EventEnvelope
from openjax_tui.event_state_manager import (
    EventStateCallbacks,
    EventStateManager,
    get_active_tool_call_count,
    has_active_tool_calls_after_event,
    update_live_viewport_ownership,
)
from openjax_tui.state import AppState, LiveViewportOwnership, ViewMode


def _evt(turn_id: str, event_type: str, payload: dict[str, object]) -> EventEnvelope:
    """创建测试事件。"""
    return EventEnvelope(
        protocol_version="v1",
        kind="event",
        session_id="s1",
        turn_id=turn_id,
        event_type=event_type,
        payload=payload,
    )


class GetActiveToolCallCountTest(unittest.TestCase):
    """测试 get_active_tool_call_count 函数。"""

    def test_empty_state_returns_zero(self) -> None:
        """空状态应返回 0。"""
        state = AppState()
        result = get_active_tool_call_count(state, "turn1")
        self.assertEqual(result, 0)

    def test_counts_active_tools_for_turn(self) -> None:
        """应正确计算指定 turn 的活动工具。"""
        state = AppState()
        state.active_tool_starts[("turn1", "tool1")] = [1.0, 2.0]
        state.active_tool_starts[("turn1", "tool2")] = [3.0]
        state.active_tool_starts[("turn2", "tool1")] = [4.0]

        result = get_active_tool_call_count(state, "turn1")
        self.assertEqual(result, 3)


class HasActiveToolCallsAfterEventTest(unittest.TestCase):
    """测试 has_active_tool_calls_after_event 函数。"""

    def test_no_turn_id_returns_false(self) -> None:
        """无 turn_id 应返回 False。"""
        state = AppState()
        evt = _evt("", "tool_call_started", {"tool_name": "test"})
        result = has_active_tool_calls_after_event(state, evt)
        self.assertFalse(result)

    def test_tool_call_started_increases_count(self) -> None:
        """tool_call_started 应增加计数。"""
        state = AppState()
        state.active_tool_starts[("turn1", "existing")] = [1.0]

        evt = _evt("turn1", "tool_call_started", {"tool_name": "new_tool"})
        result = has_active_tool_calls_after_event(state, evt)
        self.assertTrue(result)

    def test_tool_call_completed_decreases_count(self) -> None:
        """tool_call_completed 应减少计数。"""
        state = AppState()
        state.active_tool_starts[("turn1", "test_tool")] = [1.0]

        evt = _evt("turn1", "tool_call_completed", {"tool_name": "test_tool", "ok": True})
        result = has_active_tool_calls_after_event(state, evt)
        self.assertFalse(result)


class UpdateLiveViewportOwnershipTest(unittest.TestCase):
    """测试 update_live_viewport_ownership 函数。"""

    def test_non_live_viewport_mode_does_nothing(self) -> None:
        """非 live viewport 模式不应修改状态。"""
        state = AppState()
        state.view_mode = ViewMode.SESSION

        evt = _evt("turn1", "assistant_delta", {"content_delta": "hi"})
        update_live_viewport_ownership(state, evt)

        self.assertIsNone(state.live_viewport_owner_turn_id)

    def test_assistant_delta_sets_ownership(self) -> None:
        """assistant_delta 应设置所有权。"""
        state = AppState()
        state.view_mode = ViewMode.LIVE_VIEWPORT

        evt = _evt("turn1", "assistant_delta", {"content_delta": "hi"})
        update_live_viewport_ownership(state, evt)

        self.assertEqual(state.live_viewport_owner_turn_id, "turn1")
        self.assertEqual(
            state.live_viewport_turn_ownership.get("turn1"),
            LiveViewportOwnership.ACTIVE,
        )

    def test_turn_completed_releases_ownership(self) -> None:
        """turn_completed 应释放所有权。"""
        state = AppState()
        state.view_mode = ViewMode.LIVE_VIEWPORT
        state.live_viewport_owner_turn_id = "turn1"
        state.live_viewport_turn_ownership["turn1"] = LiveViewportOwnership.ACTIVE

        evt = _evt("turn1", "turn_completed", {})
        update_live_viewport_ownership(state, evt)

        self.assertIsNone(state.live_viewport_owner_turn_id)
        self.assertEqual(
            state.live_viewport_turn_ownership.get("turn1"),
            LiveViewportOwnership.RELEASED,
        )


class EventStateManagerTest(unittest.TestCase):
    """测试 EventStateManager 类。"""

    def _create_manager(self, state: AppState | None = None) -> tuple[EventStateManager, MagicMock]:
        """创建管理器和模拟回调。"""
        if state is None:
            state = AppState()

        callbacks = MagicMock()
        callbacks.is_live_viewport_mode.return_value = False

        manager = EventStateManager(state, callbacks)
        return manager, callbacks

    def test_apply_event_updates_turn_phase_to_tool_wait(self) -> None:
        """应用 tool_call_started 应将 phase 设为 tool_wait。"""
        state = AppState()
        state.waiting_turn_id = "turn1"
        state.turn_phase = "thinking"
        state.active_tool_starts[("turn1", "shell")] = [1.0]

        manager, callbacks = self._create_manager(state)
        evt = _evt("turn1", "tool_call_started", {"tool_name": "shell"})
        manager.apply_event_updates(evt)

        self.assertEqual(state.turn_phase, "tool_wait")
        callbacks.sync_animation.assert_called_once()

    def test_apply_event_updates_handles_approval_requested(self) -> None:
        """应用 approval_requested 应创建审批记录。"""
        state = AppState()
        state.waiting_turn_id = "turn1"
        try:
            state.input_ready = asyncio.Event()
        except RuntimeError:
            # 无事件循环时跳过
            self.skipTest("No event loop available")

        manager, callbacks = self._create_manager(state)
        evt = _evt("turn1", "approval_requested", {
            "request_id": "req1",
            "target": "test_target",
            "reason": "test_reason",
        })
        manager.apply_event_updates(evt)

        self.assertIn("req1", state.pending_approvals)
        self.assertEqual(state.turn_phase, "approval")
        callbacks.log_approval_event.assert_called_once()

    def test_apply_event_updates_handles_approval_resolved(self) -> None:
        """应用 approval_resolved 应调用 pop_pending 回调。"""
        state = AppState()
        state.waiting_turn_id = "turn1"
        from openjax_tui.state import ApprovalRecord
        state.pending_approvals["req1"] = ApprovalRecord(
            turn_id="turn1", target="test", reason="test"
        )
        state.approval_order.append("req1")

        manager, callbacks = self._create_manager(state)
        evt = _evt("turn1", "approval_resolved", {"request_id": "req1", "approved": True})
        manager.apply_event_updates(evt)

        # pop_pending 回调应被调用（实际移除由回调处理）
        callbacks.pop_pending.assert_called_once_with("req1")

    def test_apply_event_updates_handles_turn_completed(self) -> None:
        """应用 turn_completed 应重置状态。"""
        state = AppState()
        state.waiting_turn_id = "turn1"
        state.turn_phase = "thinking"
        try:
            state.input_ready = asyncio.Event()
            state.input_ready.clear()
        except RuntimeError:
            # 无事件循环时跳过
            self.skipTest("No event loop available")

        manager, callbacks = self._create_manager(state)
        evt = _evt("turn1", "turn_completed", {})
        manager.apply_event_updates(evt)

        self.assertIsNone(state.waiting_turn_id)
        self.assertEqual(state.turn_phase, "idle")
        self.assertTrue(state.input_ready.is_set())


if __name__ == "__main__":
    unittest.main()
