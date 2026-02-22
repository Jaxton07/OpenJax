import io
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

from openjax_sdk.models import EventEnvelope
from openjax_tui.app import AppState, _print_event, _set_active_state, _status_bullet


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

    def test_tool_events_print_immediate_lines(self) -> None:
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
        self.assertIn("🟢 Run shell command", text)
        self.assertIn("🔴 Search (failed)", text)
        self.assertNotIn("tools: calls=", text)

    def test_status_bullet_colored_dot_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        _set_active_state(state)

        with patch("openjax_tui.app._supports_ansi_color", return_value=True):
            bullet = _status_bullet(ok=True)

        self.assertEqual(bullet, "\x1b[32m⏺\x1b[0m")

    def test_tool_line_uses_prompt_toolkit_ansi_renderer(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        _set_active_state(state)
        captured: list[str] = []

        with patch("openjax_tui.app.time.monotonic", side_effect=[1.0, 1.2]), patch(
            "openjax_tui.app._supports_ansi_color", return_value=True
        ), patch("openjax_tui.app._prompt_toolkit_ansi", side_effect=lambda s: f"ANSI<{s}>"), patch(
            "openjax_tui.app._prompt_toolkit_print", side_effect=lambda s: captured.append(str(s))
        ):
            _print_event(_evt("1", "tool_call_started", {"tool_name": "shell"}))
            _print_event(_evt("1", "tool_call_completed", {"tool_name": "shell", "ok": True}))
            _print_event(_evt("1", "turn_completed", {}))

        self.assertEqual(len(captured), 1)
        self.assertIn("ANSI<", captured[0])
        self.assertIn("Run shell command", captured[0])


if __name__ == "__main__":
    unittest.main()
