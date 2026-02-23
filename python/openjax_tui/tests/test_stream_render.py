import io
import unittest
from contextlib import redirect_stdout

from openjax_tui.app import AppState
from openjax_tui import assistant_render


class StreamRenderTest(unittest.TestCase):
    def _render_assistant_delta(self, state: AppState, turn: str, delta: str) -> None:
        assistant_render.render_assistant_delta(
            state, turn, delta,
            assistant_prefix="⏺",
            align_multiline_fn=lambda t: t.replace("\n", "\n  "),
            finalize_stream_line_fn=lambda s: assistant_render.finalize_stream_line(s),
            refresh_history_view_fn=lambda s: None,
        )

    def _render_assistant_message(self, state: AppState, turn: str, content: str) -> None:
        assistant_render.render_assistant_message(
            state, turn, content,
            assistant_prefix="⏺",
            print_prefixed_block_fn=lambda s, p, c: print(f"{p} {c.replace(chr(10), chr(10)+'  ')}"),
            finalize_stream_line_fn=lambda s: assistant_render.finalize_stream_line(s),
        )

    def test_delta_renders_incrementally_and_message_dedupes(self) -> None:
        state = AppState()
        out = io.StringIO()

        with redirect_stdout(out):
            self._render_assistant_delta(state, "1", "你")
            self._render_assistant_delta(state, "1", "好")
            self._render_assistant_message(state, "1", "你好")

        self.assertEqual(out.getvalue(), "⏺ 你好\n")

    def test_multiline_assistant_alignment(self) -> None:
        state = AppState()
        out = io.StringIO()

        with redirect_stdout(out):
            self._render_assistant_message(state, "1", "第一行\n第二行")

        self.assertEqual(out.getvalue(), "⏺ 第一行\n  第二行\n")


if __name__ == "__main__":
    unittest.main()
