import io
import unittest
from contextlib import redirect_stdout

from openjax_tui import assistant_render
from openjax_tui.state import AppState


def _align_multiline(text: str) -> str:
    return text.replace("\n", "\n  ")


def _noop_refresh(_state: AppState) -> None:
    return None


def _print_prefixed_block(_state: AppState, prefix: str, content: str) -> None:
    print(f"{prefix} {content.replace(chr(10), chr(10) + '  ')}")


class StreamRenderTest(unittest.TestCase):
    def _render_assistant_delta(self, state: AppState, turn: str, delta: str) -> None:
        assistant_render.render_assistant_delta(
            state, turn, delta,
            assistant_prefix="⏺",
            align_multiline_fn=_align_multiline,
            finalize_stream_line_fn=assistant_render.finalize_stream_line,
            refresh_history_view_fn=_noop_refresh,
        )

    def _render_assistant_message(self, state: AppState, turn: str, content: str) -> None:
        assistant_render.render_assistant_message(
            state, turn, content,
            assistant_prefix="⏺",
            print_prefixed_block_fn=_print_prefixed_block,
            finalize_stream_line_fn=assistant_render.finalize_stream_line,
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

    def test_rapid_delta_burst_final_message_is_authoritative_in_prompt_toolkit(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"

        burst_chunks = [f"片段{idx:03d}|" for idx in range(120)]
        burst_stream = "".join(burst_chunks)
        final_content = "最终结论\n" + burst_stream + "收尾✅"

        for chunk in burst_chunks:
            self._render_assistant_delta(state, "burst-1", chunk)

        self.assertEqual(state.stream_text_by_turn["burst-1"], burst_stream)
        self.assertEqual(len(state.history_blocks), 1)

        self._render_assistant_message(state, "burst-1", final_content)

        self.assertEqual(state.assistant_message_by_turn["burst-1"], final_content)
        self.assertEqual(state.stream_text_by_turn["burst-1"], final_content)
        self.assertEqual(state.history_blocks, [f"⏺ {final_content.replace(chr(10), chr(10) + '  ')}"])
        self.assertIsNone(state.stream_turn_id)
        self.assertIsNone(state.stream_block_index)

    def test_final_message_overrides_mixed_width_stream_without_duplication(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"

        deltas = ["Cafe\u0301 ", "数据流 ", "👩\u200d💻\n", "第二行草稿"]
        for delta in deltas:
            self._render_assistant_delta(state, "mix-1", delta)

        self._render_assistant_message(state, "mix-1", "Cafe\u0301 数据流 👩\u200d💻\n第二行定稿")

        self.assertEqual(len(state.history_blocks), 1)
        self.assertEqual(state.history_blocks[0], "⏺ Cafe\u0301 数据流 👩\u200d💻\n  第二行定稿")
        self.assertEqual(state.assistant_message_by_turn["mix-1"], "Cafe\u0301 数据流 👩\u200d💻\n第二行定稿")


if __name__ == "__main__":
    _ = unittest.main()
