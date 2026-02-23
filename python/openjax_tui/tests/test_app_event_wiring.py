from __future__ import annotations

import io
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

from openjax_sdk.models import EventEnvelope
from openjax_tui import app


def _evt(turn_id: str, event_type: str, payload: dict[str, object]) -> EventEnvelope:
    return EventEnvelope(
        protocol_version="v1",
        kind="event",
        session_id="s1",
        turn_id=turn_id,
        event_type=event_type,
        payload=payload,
    )


class AppEventWiringTest(unittest.TestCase):
    def test_dispatch_event_tool_call_completed_uses_runtime_adapters(self) -> None:
        state = app.AppState()
        out = io.StringIO()

        with (
            redirect_stdout(out),
            patch("time.monotonic", side_effect=[1.0, 1.4]),
            patch("openjax_tui.startup_ui._supports_ansi_color", return_value=False),
        ):
            app._dispatch_event(_evt("t1", "tool_call_started", {"tool_name": "shell"}), state)
            app._dispatch_event(
                _evt(
                    "t1",
                    "tool_call_completed",
                    {"tool_name": "shell", "ok": True, "output": "done"},
                ),
                state,
            )

        self.assertIn("Run shell command", out.getvalue())
        stats = state.tool_turn_stats.get("t1")
        self.assertIsNotNone(stats)
        self.assertEqual(stats.calls, 1)
        self.assertEqual(stats.ok_count, 1)


if __name__ == "__main__":
    unittest.main()
