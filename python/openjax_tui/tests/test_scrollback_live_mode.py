from __future__ import annotations

import unittest

from openjax_tui import assistant_render
from openjax_tui.app import retain_live_viewport_blocks
from openjax_tui.state import AppState, ViewMode


def _noop_refresh(_state: AppState) -> None:
    return None


class ScrollbackLiveModeTest(unittest.TestCase):
    def test_live_mode_flushes_all_when_no_active_stream(self) -> None:
        state = AppState()
        state.view_mode = ViewMode.LIVE_VIEWPORT
        state.history_blocks = ["first", "second"]
        state.turn_block_index = {"t1": 0, "t2": 1}
        state.stream_block_index = 1
        state.stream_turn_id = None

        dropped = retain_live_viewport_blocks(state)

        self.assertEqual(dropped, ["first", "second"])
        self.assertEqual(state.history_blocks, [])
        self.assertEqual(state.turn_block_index, {})
        self.assertIsNone(state.stream_block_index)

    def test_live_mode_keeps_only_in_progress_turn_block(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.view_mode = ViewMode.LIVE_VIEWPORT

        assistant_render.render_assistant_delta(
            state,
            "t1",
            "hello",
            assistant_prefix="⏺",
            align_multiline_fn=lambda text: text.replace("\n", "\n  "),
            finalize_stream_line_fn=assistant_render.finalize_stream_line,
            refresh_history_view_fn=_noop_refresh,
        )
        assistant_render.render_assistant_delta(
            state,
            "t2",
            "live",
            assistant_prefix="⏺",
            align_multiline_fn=lambda text: text.replace("\n", "\n  "),
            finalize_stream_line_fn=assistant_render.finalize_stream_line,
            refresh_history_view_fn=_noop_refresh,
        )

        dropped = retain_live_viewport_blocks(state)

        self.assertEqual(dropped, ["⏺ hello"])
        self.assertEqual(state.history_blocks, ["⏺ live"])
        self.assertEqual(state.turn_block_index, {"t2": 0})
        self.assertEqual(state.stream_block_index, 0)

    def test_live_mode_flushes_all_when_stream_index_is_out_of_range(self) -> None:
        state = AppState()
        state.view_mode = ViewMode.LIVE_VIEWPORT
        state.history_blocks = ["first", "second"]
        state.turn_block_index = {"t1": 0, "t2": 1}
        state.stream_turn_id = "t2"
        state.stream_block_index = 9

        dropped = retain_live_viewport_blocks(state)

        self.assertEqual(dropped, ["first", "second"])
        self.assertEqual(state.history_blocks, [])
        self.assertEqual(state.turn_block_index, {})
        self.assertIsNone(state.stream_block_index)

    def test_live_mode_flushes_all_when_stream_turn_missing_even_with_index(self) -> None:
        state = AppState()
        state.view_mode = ViewMode.LIVE_VIEWPORT
        state.history_blocks = ["first", "second"]
        state.turn_block_index = {"t1": 0, "t2": 1}
        state.stream_turn_id = None
        state.stream_block_index = 1

        dropped = retain_live_viewport_blocks(state)

        self.assertEqual(dropped, ["first", "second"])
        self.assertEqual(state.history_blocks, [])
        self.assertEqual(state.turn_block_index, {})
        self.assertIsNone(state.stream_block_index)

    def test_session_mode_does_not_retain_or_flush(self) -> None:
        state = AppState()
        state.view_mode = ViewMode.SESSION
        state.stream_turn_id = "t1"
        state.stream_block_index = 1
        state.history_blocks = ["one", "two"]

        dropped = retain_live_viewport_blocks(state)

        self.assertEqual(dropped, [])
        self.assertEqual(state.history_blocks, ["one", "two"])
        self.assertEqual(state.stream_turn_id, "t1")
        self.assertEqual(state.stream_block_index, 1)

    def test_live_mode_long_mixed_width_tail_keeps_active_block_lossless(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.view_mode = ViewMode.LIVE_VIEWPORT

        long_cjk_multiline = "\n".join(
            [
                "前置段落 line-0",
                "混合宽度 Cafe\u0301 与 emoji 👩\u200d💻🚀",
                "尾部行 " + "数据块" * 80,
            ]
        )

        assistant_render.render_assistant_delta(
            state,
            "turn-old",
            "legacy",
            assistant_prefix="⏺",
            align_multiline_fn=lambda text: text.replace("\n", "\n  "),
            finalize_stream_line_fn=assistant_render.finalize_stream_line,
            refresh_history_view_fn=_noop_refresh,
        )
        assistant_render.render_assistant_delta(
            state,
            "turn-live",
            long_cjk_multiline,
            assistant_prefix="⏺",
            align_multiline_fn=lambda text: text.replace("\n", "\n  "),
            finalize_stream_line_fn=assistant_render.finalize_stream_line,
            refresh_history_view_fn=_noop_refresh,
        )

        dropped = retain_live_viewport_blocks(state)

        self.assertEqual(len(dropped), 1)
        self.assertEqual(dropped[0], "⏺ legacy")
        self.assertEqual(state.history_blocks, [f"⏺ {long_cjk_multiline.replace(chr(10), chr(10) + '  ')}"])
        self.assertEqual(state.turn_block_index, {"turn-live": 0})
        self.assertEqual(state.stream_turn_id, "turn-live")
        self.assertEqual(state.stream_text_by_turn["turn-live"], long_cjk_multiline)

    def test_live_mode_rapid_turn_burst_only_keeps_latest_active_turn(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.view_mode = ViewMode.LIVE_VIEWPORT

        for idx in range(40):
            turn = f"t{idx}"
            assistant_render.render_assistant_delta(
                state,
                turn,
                f"chunk-{idx}",
                assistant_prefix="⏺",
                align_multiline_fn=lambda text: text.replace("\n", "\n  "),
                finalize_stream_line_fn=assistant_render.finalize_stream_line,
                refresh_history_view_fn=_noop_refresh,
            )

        dropped = retain_live_viewport_blocks(state)

        self.assertEqual(len(dropped), 39)
        self.assertEqual(state.history_blocks, ["⏺ chunk-39"])
        self.assertEqual(state.turn_block_index, {"t39": 0})
        self.assertEqual(state.stream_turn_id, "t39")
        self.assertEqual(state.stream_block_index, 0)


if __name__ == "__main__":
    _ = unittest.main()
