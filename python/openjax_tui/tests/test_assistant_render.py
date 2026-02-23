from __future__ import annotations

import unittest

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


if __name__ == "__main__":
    unittest.main()
