import io
import unittest
from contextlib import redirect_stdout

from openjax_tui.app import AppState, _render_assistant_delta, _render_assistant_message, _set_active_state


class StreamRenderTest(unittest.TestCase):
    def tearDown(self) -> None:
        _set_active_state(None)

    def test_delta_renders_incrementally_and_message_dedupes(self) -> None:
        state = AppState()
        _set_active_state(state)
        out = io.StringIO()

        with redirect_stdout(out):
            _render_assistant_delta("1", "你")
            _render_assistant_delta("1", "好")
            _render_assistant_message("1", "你好")

        self.assertEqual(out.getvalue(), "⏺ 你好\n")


if __name__ == "__main__":
    unittest.main()
