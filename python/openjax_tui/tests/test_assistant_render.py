from __future__ import annotations

import io
import unittest
from contextlib import redirect_stdout

from openjax_tui import assistant_render
from openjax_tui.state import AppState


class AssistantRenderModuleTest(unittest.TestCase):
    def test_finalize_stream_line_resets_state(self) -> None:
        state = AppState()
        state.stream_turn_id = "t1"
        state.input_backend = "prompt_toolkit"
        assistant_render.finalize_stream_line(state)
        self.assertIsNone(state.stream_turn_id)

    def test_emit_ui_line_appends_history_in_prompt_toolkit(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        called: list[str] = []
        assistant_render.emit_ui_line(
            state,
            "line",
            refresh_history_view_fn=lambda _s: called.append("refresh"),
        )
        self.assertEqual(state.history_blocks, ["line"])
        self.assertEqual(called, ["refresh"])

    def test_emit_ui_line_adds_blank_line_between_basic_blocks(self) -> None:
        state = AppState()
        state.input_backend = "basic"
        out = io.StringIO()
        with redirect_stdout(out):
            assistant_render.emit_ui_line(
                state,
                "first",
                refresh_history_view_fn=lambda _s: None,
            )
            assistant_render.emit_ui_line(
                state,
                "second",
                refresh_history_view_fn=lambda _s: None,
            )
        self.assertEqual(out.getvalue(), "first\n\nsecond\n")


if __name__ == "__main__":
    unittest.main()
