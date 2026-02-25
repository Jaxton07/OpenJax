"""Tests for state management module."""

from __future__ import annotations

import unittest

from openjax_tui.state import AppState, ApprovalRequest, Message, TurnPhase


class TestTurnPhase(unittest.TestCase):
    """Test TurnPhase enum."""

    def test_turn_phase_values(self) -> None:
        self.assertEqual(TurnPhase.IDLE.name, "IDLE")
        self.assertEqual(TurnPhase.THINKING.name, "THINKING")
        self.assertEqual(TurnPhase.STREAMING.name, "STREAMING")
        self.assertEqual(TurnPhase.ERROR.name, "ERROR")


class TestMessage(unittest.TestCase):
    """Test Message dataclass."""

    def test_message_creation(self) -> None:
        msg = Message(role="user", content="Hello")
        self.assertEqual(msg.role, "user")
        self.assertEqual(msg.content, "Hello")
        self.assertIsNone(msg.timestamp)
        self.assertEqual(msg.metadata, {})


class TestApprovalRequest(unittest.TestCase):
    """Test ApprovalRequest dataclass."""

    def test_approval_creation(self) -> None:
        approval = ApprovalRequest(id="test-1", turn_id="turn-1", action="write_file")
        self.assertEqual(approval.id, "test-1")
        self.assertEqual(approval.turn_id, "turn-1")
        self.assertEqual(approval.action, "write_file")
        self.assertIsNone(approval.reason)


class TestAppState(unittest.TestCase):
    """Test AppState dataclass."""

    def test_default_state(self) -> None:
        state = AppState()
        self.assertIsNone(state.session_id)
        self.assertEqual(state.turn_phase, TurnPhase.IDLE)
        self.assertEqual(state.messages, [])
        self.assertEqual(state.pending_approvals, {})
        self.assertEqual(state.approval_order, [])
        self.assertIsNone(state.approval_focus_id)
        self.assertEqual(state.current_input, "")
        self.assertFalse(state.command_palette_open)
        self.assertIsNone(state.active_turn_id)
        self.assertEqual(state.stream_text_by_turn, {})
        self.assertIsNone(state.last_error)

    def test_add_message(self) -> None:
        state = AppState()
        msg = state.add_message("user", "Hello")

        self.assertEqual(len(state.messages), 1)
        self.assertEqual(msg.role, "user")
        self.assertEqual(msg.content, "Hello")
        self.assertIsNotNone(msg.timestamp)

    def test_clear_messages(self) -> None:
        state = AppState()
        state.add_message("user", "Hello")
        state.add_message("assistant", "Hi")

        state.clear_messages()
        self.assertEqual(len(state.messages), 0)

    def test_add_approval_tracks_order_and_focus(self) -> None:
        state = AppState()
        approval = state.add_approval("app-1", "write_file", turn_id="turn-1")

        self.assertEqual(len(state.pending_approvals), 1)
        self.assertEqual(approval.id, "app-1")
        self.assertEqual(approval.turn_id, "turn-1")
        self.assertIn("app-1", state.pending_approvals)
        self.assertEqual(state.approval_order, ["app-1"])
        self.assertEqual(state.approval_focus_id, "app-1")

    def test_resolve_approval_updates_focus(self) -> None:
        state = AppState()
        state.add_approval("app-1", "write_file", turn_id="turn-1")
        state.add_approval("app-2", "delete_file", turn_id="turn-2")

        resolved = state.resolve_approval("app-2")

        self.assertIsNotNone(resolved)
        self.assertEqual(resolved.id, "app-2")
        self.assertEqual(state.approval_focus_id, "app-1")
        self.assertEqual(state.approval_order, ["app-1"])

    def test_get_pending_approval_count(self) -> None:
        state = AppState()
        self.assertEqual(state.get_pending_approval_count(), 0)

        state.add_approval("app-1", "action1", turn_id="turn-1")
        state.add_approval("app-2", "action2", turn_id="turn-2")
        self.assertEqual(state.get_pending_approval_count(), 2)

        state.resolve_approval("app-1")
        self.assertEqual(state.get_pending_approval_count(), 1)

    def test_start_turn_sets_state(self) -> None:
        state = AppState()
        state.start_turn("turn-1")

        self.assertEqual(state.active_turn_id, "turn-1")
        self.assertEqual(state.stream_text_by_turn["turn-1"], "")
        self.assertEqual(state.turn_phase, TurnPhase.THINKING)

    def test_append_delta_aggregates_content(self) -> None:
        state = AppState()
        text = state.append_delta("turn-1", "hello")
        text = state.append_delta("turn-1", " world")

        self.assertEqual(text, "hello world")
        self.assertEqual(state.stream_text_by_turn["turn-1"], "hello world")
        self.assertEqual(state.turn_phase, TurnPhase.STREAMING)

    def test_finalize_turn_uses_final_content(self) -> None:
        state = AppState()
        state.append_delta("turn-1", "draft")

        final_text = state.finalize_turn("turn-1", "final")

        self.assertEqual(final_text, "final")
        self.assertIsNone(state.active_turn_id)
        self.assertNotIn("turn-1", state.stream_text_by_turn)
        self.assertEqual(state.turn_phase, TurnPhase.IDLE)

    def test_set_error(self) -> None:
        state = AppState()

        state.set_error("boom")

        self.assertEqual(state.last_error, "boom")
        self.assertEqual(state.turn_phase, TurnPhase.ERROR)

    def test_latest_pending_approval(self) -> None:
        state = AppState()
        state.add_approval("app-1", "action1", turn_id="turn-1")
        state.add_approval("app-2", "action2", turn_id="turn-2")

        latest = state.latest_pending_approval()

        self.assertIsNotNone(latest)
        assert latest is not None
        self.assertEqual(latest.id, "app-2")

    def test_add_tool_call_result(self) -> None:
        state = AppState()

        msg = state.add_tool_call_result("read_file", True, "ok")

        self.assertEqual(msg.role, "tool")
        self.assertEqual(msg.content, "Read 1 file")
        self.assertTrue(msg.metadata["ok"])
        self.assertEqual(msg.metadata["tool_name"], "read_file")


if __name__ == "__main__":
    unittest.main()
