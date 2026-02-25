"""Tests for state management module."""

from __future__ import annotations

import unittest

from openjax_tui.state import AppState, ApprovalRequest, Message, TurnPhase


class TestTurnPhase(unittest.TestCase):
    """Test TurnPhase enum."""

    def test_turn_phase_values(self) -> None:
        """Test that TurnPhase has expected values."""
        self.assertEqual(TurnPhase.IDLE.name, "IDLE")
        self.assertEqual(TurnPhase.THINKING.name, "THINKING")
        self.assertEqual(TurnPhase.STREAMING.name, "STREAMING")
        self.assertEqual(TurnPhase.ERROR.name, "ERROR")


class TestMessage(unittest.TestCase):
    """Test Message dataclass."""

    def test_message_creation(self) -> None:
        """Test creating a Message."""
        msg = Message(role="user", content="Hello")
        self.assertEqual(msg.role, "user")
        self.assertEqual(msg.content, "Hello")
        self.assertIsNone(msg.timestamp)
        self.assertEqual(msg.metadata, {})

    def test_message_with_metadata(self) -> None:
        """Test creating a Message with metadata."""
        msg = Message(
            role="assistant",
            content="Hi",
            timestamp="2024-01-01T00:00:00",
            metadata={"model": "gpt-4"},
        )
        self.assertEqual(msg.metadata["model"], "gpt-4")


class TestApprovalRequest(unittest.TestCase):
    """Test ApprovalRequest dataclass."""

    def test_approval_creation(self) -> None:
        """Test creating an ApprovalRequest."""
        approval = ApprovalRequest(id="test-1", action="write_file")
        self.assertEqual(approval.id, "test-1")
        self.assertEqual(approval.action, "write_file")
        self.assertIsNone(approval.reason)

    def test_approval_with_reason(self) -> None:
        """Test creating an ApprovalRequest with reason."""
        approval = ApprovalRequest(
            id="test-2",
            action="delete_file",
            reason="User requested deletion",
        )
        self.assertEqual(approval.reason, "User requested deletion")


class TestAppState(unittest.TestCase):
    """Test AppState dataclass."""

    def test_default_state(self) -> None:
        """Test default AppState initialization."""
        state = AppState()
        self.assertIsNone(state.session_id)
        self.assertEqual(state.turn_phase, TurnPhase.IDLE)
        self.assertEqual(state.messages, [])
        self.assertEqual(state.pending_approvals, {})
        self.assertEqual(state.current_input, "")
        self.assertFalse(state.command_palette_open)

    def test_add_message(self) -> None:
        """Test adding a message."""
        state = AppState()
        msg = state.add_message("user", "Hello")

        self.assertEqual(len(state.messages), 1)
        self.assertEqual(msg.role, "user")
        self.assertEqual(msg.content, "Hello")
        self.assertIsNotNone(msg.timestamp)

    def test_add_message_with_metadata(self) -> None:
        """Test adding a message with metadata."""
        state = AppState()
        msg = state.add_message("assistant", "Hi", model="gpt-4")

        self.assertEqual(msg.metadata["model"], "gpt-4")

    def test_clear_messages(self) -> None:
        """Test clearing all messages."""
        state = AppState()
        state.add_message("user", "Hello")
        state.add_message("assistant", "Hi")

        self.assertEqual(len(state.messages), 2)

        state.clear_messages()
        self.assertEqual(len(state.messages), 0)

    def test_add_approval(self) -> None:
        """Test adding an approval request."""
        state = AppState()
        approval = state.add_approval("app-1", "write_file")

        self.assertEqual(len(state.pending_approvals), 1)
        self.assertEqual(approval.id, "app-1")
        self.assertEqual(approval.action, "write_file")
        self.assertIn("app-1", state.pending_approvals)

    def test_add_approval_with_reason(self) -> None:
        """Test adding an approval request with reason."""
        state = AppState()
        approval = state.add_approval("app-2", "delete_file", "Test reason")

        self.assertEqual(approval.reason, "Test reason")

    def test_resolve_approval(self) -> None:
        """Test resolving an approval request."""
        state = AppState()
        state.add_approval("app-1", "write_file")

        resolved = state.resolve_approval("app-1")

        self.assertIsNotNone(resolved)
        self.assertEqual(resolved.id, "app-1")
        self.assertEqual(len(state.pending_approvals), 0)

    def test_resolve_nonexistent_approval(self) -> None:
        """Test resolving a non-existent approval."""
        state = AppState()

        resolved = state.resolve_approval("nonexistent")

        self.assertIsNone(resolved)

    def test_get_pending_approval_count(self) -> None:
        """Test getting pending approval count."""
        state = AppState()
        self.assertEqual(state.get_pending_approval_count(), 0)

        state.add_approval("app-1", "action1")
        self.assertEqual(state.get_pending_approval_count(), 1)

        state.add_approval("app-2", "action2")
        self.assertEqual(state.get_pending_approval_count(), 2)

        state.resolve_approval("app-1")
        self.assertEqual(state.get_pending_approval_count(), 1)

    def test_set_turn_phase(self) -> None:
        """Test setting turn phase."""
        state = AppState()
        self.assertEqual(state.turn_phase, TurnPhase.IDLE)

        state.set_turn_phase(TurnPhase.THINKING)
        self.assertEqual(state.turn_phase, TurnPhase.THINKING)

        state.set_turn_phase(TurnPhase.STREAMING)
        self.assertEqual(state.turn_phase, TurnPhase.STREAMING)

    def test_is_processing_idle(self) -> None:
        """Test is_processing returns False when idle."""
        state = AppState()
        state.set_turn_phase(TurnPhase.IDLE)
        self.assertFalse(state.is_processing())

    def test_is_processing_thinking(self) -> None:
        """Test is_processing returns True when thinking."""
        state = AppState()
        state.set_turn_phase(TurnPhase.THINKING)
        self.assertTrue(state.is_processing())

    def test_is_processing_streaming(self) -> None:
        """Test is_processing returns True when streaming."""
        state = AppState()
        state.set_turn_phase(TurnPhase.STREAMING)
        self.assertTrue(state.is_processing())

    def test_is_processing_error(self) -> None:
        """Test is_processing returns False when error."""
        state = AppState()
        state.set_turn_phase(TurnPhase.ERROR)
        self.assertFalse(state.is_processing())


if __name__ == "__main__":
    unittest.main()
