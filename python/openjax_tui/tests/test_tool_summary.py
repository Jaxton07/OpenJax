import io
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

from openjax_sdk.models import EventEnvelope
from openjax_tui.app import AppState, _print_event, _set_active_state


def _evt(turn_id: str, event_type: str, payload: dict[str, object]) -> EventEnvelope:
    return EventEnvelope(
        protocol_version="v1",
        kind="event",
        session_id="s1",
        turn_id=turn_id,
        event_type=event_type,
        payload=payload,
    )


class ToolSummaryTest(unittest.TestCase):
    def tearDown(self) -> None:
        _set_active_state(None)

    def test_tool_events_fold_into_single_summary(self) -> None:
        state = AppState()
        _set_active_state(state)
        out = io.StringIO()

        with redirect_stdout(out), patch(
            "openjax_tui.app.time.monotonic", side_effect=[1.0, 1.3, 2.0, 2.25]
        ), patch("openjax_tui.app._supports_ansi_color", return_value=False):
            _print_event(_evt("1", "tool_call_started", {"tool_name": "shell"}))
            _print_event(_evt("1", "tool_call_completed", {"tool_name": "shell", "ok": True}))
            _print_event(_evt("1", "tool_call_started", {"tool_name": "search"}))
            _print_event(_evt("1", "tool_call_completed", {"tool_name": "search", "ok": False}))
            _print_event(_evt("1", "turn_completed", {}))

        text = out.getvalue()
        self.assertIn("⏺ tools: calls=2 ok=1 fail=1 duration=550ms names=[search, shell]", text)
        self.assertNotIn("tool> shell ...", text)
        self.assertNotIn("tool> search ...", text)


if __name__ == "__main__":
    unittest.main()
