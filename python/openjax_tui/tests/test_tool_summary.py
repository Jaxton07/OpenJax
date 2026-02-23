import io
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

from openjax_sdk.models import EventEnvelope
from openjax_tui.app import AppState, _print_event, _set_active_state, _status_bullet
from openjax_tui import event_dispatch
from openjax_tui import render_utils
from openjax_tui import tool_runtime


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

    def test_tool_line_appends_to_history_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        _set_active_state(state)

        with patch("openjax_tui.app.time.monotonic", side_effect=[1.0, 1.2]), patch(
            "openjax_tui.app._supports_ansi_color", return_value=True
        ):
            _print_event(_evt("1", "tool_call_started", {"tool_name": "shell"}))
            _print_event(_evt("1", "tool_call_completed", {"tool_name": "shell", "ok": True}))
            _print_event(_evt("1", "turn_completed", {}))

        self.assertEqual(len(state.history_blocks), 1)
        self.assertIn("Run shell command", state.history_blocks[0])

    def test_render_utils_module_helpers(self) -> None:
        self.assertEqual(render_utils.align_multiline("a\nb", "  "), "a\n  b")
        self.assertEqual(
            render_utils.tool_result_label("apply_patch", "UPDATE test.txt"),
            "Update(test.txt)",
        )

    def test_event_dispatch_ignores_turn_started(self) -> None:
        called: list[str] = []
        event_dispatch.print_event(
            _evt("1", "turn_started", {}),
            state=None,
            print_tool_turn_summary=False,
            render_assistant_delta_fn=lambda *_: called.append("delta"),
            render_assistant_message_fn=lambda *_: called.append("message"),
            finalize_stream_line_if_turn_fn=lambda *_: called.append("finalize"),
            record_tool_started_fn=lambda *_: called.append("tool_start"),
            record_tool_completed_fn=lambda *_: called.append("tool_done"),
            print_tool_call_result_line_fn=lambda *_: called.append("tool_line"),
            use_inline_approval_panel_fn=lambda *_: False,
            print_tool_summary_for_turn_fn=lambda *_: called.append("summary"),
        )
        self.assertEqual(called, [])

    def test_tool_runtime_status_bullet_without_ansi(self) -> None:
        state = AppState()
        self.assertEqual(
            tool_runtime.status_bullet(
                state=state,
                ok=True,
                assistant_prefix="⏺",
                ansi_green="\x1b[32m",
                ansi_red="\x1b[31m",
                ansi_reset="\x1b[0m",
                supports_ansi_color_fn=lambda: False,
            ),
            "🟢",
        )


if __name__ == "__main__":
    unittest.main()
