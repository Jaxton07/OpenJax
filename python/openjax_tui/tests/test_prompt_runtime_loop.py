from __future__ import annotations

import asyncio
import time
import unittest
from unittest.mock import AsyncMock

from openjax_sdk import OpenJaxAsyncClient
from openjax_tui.prompt_runtime_loop import (
    PromptToolkitComponents,
    compact_history_window,
    fallback_prompt_toolkit_to_basic,
    run_prompt_toolkit_loop,
    _status_line_text,
)
from openjax_tui.state import AppState, ViewMode


class PromptRuntimeLoopTest(unittest.IsolatedAsyncioTestCase):
    def test_status_line_uses_approval_flash_message_when_active(self) -> None:
        state = AppState()
        state.turn_phase = "thinking"
        state.approval_flash_message = "Approved"
        state.approval_flash_until = time.monotonic() + 10
        self.assertEqual(_status_line_text(state), "Approved")

    async def test_fallback_to_basic_resets_prompt_runtime_state(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.stream_turn_id = "turn-1"
        state.stream_block_index = 0
        state.live_viewport_owner_turn_id = "turn-1"
        state.live_viewport_turn_ownership["turn-1"] = "active"  # type: ignore[assignment]
        state.history_setter = lambda: None
        state.prompt_invalidator = lambda: None
        client = OpenJaxAsyncClient(daemon_cmd=["true"])
        run_basic = AsyncMock()

        await fallback_prompt_toolkit_to_basic(
            client,
            state,
            reason="prompt_toolkit_exited_early",
            run_input_loop_basic_fn=run_basic,
            request_prompt_redraw_fn=lambda _: None,
            finalize_stream_line_fn=lambda s: setattr(s, "stream_turn_id", None),
        )

        self.assertEqual(state.input_backend, "basic")
        self.assertEqual(state.input_backend_reason, "prompt_toolkit_exited_early")
        self.assertIsNone(state.stream_turn_id)
        self.assertEqual(state.live_viewport_turn_ownership, {})
        self.assertIsNone(state.history_setter)
        self.assertIsNone(state.prompt_invalidator)
        run_basic.assert_awaited_once()

    async def test_unavailable_runtime_falls_back_without_prompt_bootstrap(self) -> None:
        state = AppState()
        state.input_ready = asyncio.Event()
        state.input_ready.set()
        client = OpenJaxAsyncClient(daemon_cmd=["true"])
        fallback = AsyncMock()

        await run_prompt_toolkit_loop(
            client,
            state,
            components=PromptToolkitComponents(
                prompt_session_cls=None,
                patch_stdout=None,
                application_cls=None,
                text_area_cls=None,
                document_cls=None,
                layout_cls=None,
                hsplit_cls=None,
                vsplit_cls=None,
                window_cls=None,
                formatted_text_control_cls=None,
                condition_cls=None,
                conditional_container_cls=None,
                dimension_cls=None,
                completer_cls=None,
                completion_cls=None,
                run_in_terminal_fn=None,
            ),
            key_bindings=None,
            prompt_style=None,
            slash_commands=("/help",),
            user_prompt_prefix="❯",
            divider_line_fn=lambda: "---",
            handle_user_line_fn=AsyncMock(return_value=True),
            fallback_to_basic_fn=fallback,
            request_prompt_redraw_fn=lambda _: None,
            drain_background_task_fn=AsyncMock(),
            tui_log_info_fn=lambda _: None,
            tui_debug_fn=lambda _: None,
        )

        fallback.assert_awaited_once_with(client, state, "prompt_toolkit_unavailable")

    def test_compact_history_window_drops_old_blocks_and_reindexes(self) -> None:
        state = AppState()
        state.view_mode = ViewMode.SESSION
        state.history_blocks = [
            "line-1",
            "line-2",
            "line-3",
            "line-4",
        ]
        state.stream_block_index = 2
        state.turn_block_index = {"t1": 1, "t2": 3}
        state.history_manual_scroll = 10
        debug_lines: list[str] = []

        dropped = compact_history_window(
            state,
            max_history_window_lines=6,
            tui_debug_fn=debug_lines.append,
        )

        self.assertTrue(dropped)
        self.assertEqual(len(state.history_blocks), 3)
        self.assertEqual(state.stream_block_index, 1)
        self.assertEqual(state.turn_block_index, {"t1": 0, "t2": 2})
        self.assertLessEqual(state.history_manual_scroll, 10)
        self.assertTrue(debug_lines)


if __name__ == "__main__":
    unittest.main()
