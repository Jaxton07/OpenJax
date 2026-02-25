"""Tests for event mapper."""

from __future__ import annotations

from dataclasses import dataclass
import unittest

from openjax_tui.event_mapper import map_event
from openjax_tui.state import AppState, TurnPhase


@dataclass
class DummyEvent:
    event_type: str
    turn_id: str | None
    payload: dict


class TestEventMapper(unittest.TestCase):
    """Test mapping daemon events to state/UI ops."""

    def test_assistant_delta_accumulates_stream(self) -> None:
        state = AppState()

        ops = map_event(
            DummyEvent(event_type="assistant_delta", turn_id="t1", payload={"content_delta": "Hello"}),
            state,
        )

        self.assertEqual(state.stream_text_by_turn["t1"], "Hello")
        self.assertEqual(state.turn_phase, TurnPhase.STREAMING)
        self.assertEqual(ops[0].kind, "stream_updated")
        self.assertEqual(ops[0].text, "Hello")

    def test_assistant_message_overrides_stream(self) -> None:
        state = AppState()
        state.append_delta("t1", "draft")

        ops = map_event(
            DummyEvent(event_type="assistant_message", turn_id="t1", payload={"content": "final"}),
            state,
        )

        self.assertEqual(state.stream_text_by_turn["t1"], "final")
        self.assertEqual(state.turn_render_kind_by_turn["t1"], "markdown")
        self.assertEqual(ops[0].text, "final")

    def test_turn_completed_clears_stream(self) -> None:
        state = AppState()
        state.append_delta("t1", "hello")

        ops = map_event(DummyEvent(event_type="turn_completed", turn_id="t1", payload={}), state)

        self.assertEqual(ops[0].kind, "turn_completed")
        self.assertEqual(ops[0].text, "hello")
        self.assertNotIn("t1", state.stream_text_by_turn)
        self.assertEqual(state.turn_phase, TurnPhase.IDLE)

    def test_approval_events_sync_pending(self) -> None:
        state = AppState()

        map_event(
            DummyEvent(
                event_type="approval_requested",
                turn_id="turn-1",
                payload={"request_id": "req-1", "target": "shell", "reason": "need permission"},
            ),
            state,
        )
        self.assertEqual(state.get_pending_approval_count(), 1)

        map_event(
            DummyEvent(
                event_type="approval_resolved",
                turn_id="turn-1",
                payload={"request_id": "req-1"},
            ),
            state,
        )
        self.assertEqual(state.get_pending_approval_count(), 0)

    def test_tool_call_completed_adds_tool_line(self) -> None:
        state = AppState()

        ops = map_event(
            DummyEvent(
                event_type="tool_call_completed",
                turn_id="turn-1",
                payload={"tool_name": "read_file", "ok": True, "output": "READ test.txt"},
            ),
            state,
        )

        self.assertEqual(ops[0].kind, "tool_call_completed")
        self.assertEqual(state.messages[-1].role, "tool")
        self.assertEqual(state.messages[-1].content, "Read 1 file")
        self.assertTrue(state.messages[-1].metadata["ok"])
        self.assertEqual(state.messages[-1].metadata["render_kind"], "plain")
        self.assertEqual(state.messages[-1].metadata["target"], "test.txt")
        self.assertEqual(state.messages[-1].metadata["output_preview"], "READ test.txt")


if __name__ == "__main__":
    unittest.main()
