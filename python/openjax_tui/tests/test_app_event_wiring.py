from __future__ import annotations

import asyncio
import io
import unittest
from collections.abc import Awaitable, Callable
from contextlib import redirect_stdout
from typing import cast
from unittest.mock import AsyncMock, patch

from openjax_sdk.models import EventEnvelope
from openjax_tui import app
from openjax_tui.state import AppState, LiveViewportOwnership, ViewMode


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
        if stats is None:
            raise AssertionError("expected tool stats for turn t1")
        self.assertEqual(stats.calls, 1)
        self.assertEqual(stats.ok_count, 1)

    def test_apply_updates_tool_completion_without_dispatch_side_effect_dependency(self) -> None:
        state = AppState()
        state.waiting_turn_id = "t1"
        state.turn_phase = "thinking"
        apply_updates = cast(Callable[[AppState, EventEnvelope], None], getattr(app, "_apply_event_state_updates"))

        state.active_tool_starts[("t1", "shell")] = [1.0]
        apply_updates(state, _evt("t1", "tool_call_started", {"tool_name": "shell"}))
        self.assertEqual(state.turn_phase, "tool_wait")

        apply_updates(state, _evt("t1", "tool_call_completed", {"tool_name": "shell", "ok": True}))
        self.assertEqual(state.turn_phase, "thinking")

    def test_apply_updates_live_viewport_ownership_across_turn_lifecycle(self) -> None:
        state = AppState()
        state.view_mode = ViewMode.LIVE_VIEWPORT
        state.waiting_turn_id = "turn-1"
        state.input_ready = asyncio.Event()
        state.input_ready.clear()
        apply_updates = cast(Callable[[AppState, EventEnvelope], None], getattr(app, "_apply_event_state_updates"))

        apply_updates(state, _evt("turn-1", "assistant_delta", {"content_delta": "hi"}))
        self.assertEqual(state.turn_phase, "thinking")
        self.assertEqual(state.live_viewport_owner_turn_id, "turn-1")
        self.assertEqual(
            state.live_viewport_turn_ownership.get("turn-1"),
            LiveViewportOwnership.ACTIVE,
        )

        state.active_tool_starts[("turn-1", "shell")] = [1.0]
        apply_updates(state, _evt("turn-1", "tool_call_started", {"tool_name": "shell"}))
        self.assertEqual(state.turn_phase, "tool_wait")

        state.active_tool_starts[("turn-1", "shell")] = []
        apply_updates(state, _evt("turn-1", "tool_call_completed", {"tool_name": "shell", "ok": True}))
        self.assertEqual(state.turn_phase, "thinking")

        apply_updates(state, _evt("turn-1", "assistant_message", {"content": "done"}))
        self.assertEqual(
            state.live_viewport_turn_ownership.get("turn-1"),
            LiveViewportOwnership.RELEASED,
        )
        self.assertIsNone(state.live_viewport_owner_turn_id)

        apply_updates(state, _evt("turn-1", "turn_completed", {}))
        self.assertEqual(state.turn_phase, "idle")
        self.assertIsNone(state.waiting_turn_id)


class AppFallbackWiringTest(unittest.IsolatedAsyncioTestCase):
    async def test_fallback_to_basic_clears_live_viewport_and_stream_markers(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.input_backend_reason = ""
        state.view_mode = ViewMode.LIVE_VIEWPORT
        state.live_viewport_owner_turn_id = "turn-1"
        state.live_viewport_turn_ownership["turn-1"] = LiveViewportOwnership.ACTIVE
        state.stream_turn_id = "turn-1"
        state.stream_block_index = 0
        state.history_setter = lambda: None
        state.prompt_invalidator = lambda: None
        client = app.OpenJaxAsyncClient(daemon_cmd=["true"])
        fallback_to_basic = cast(Callable[..., Awaitable[None]], getattr(app, "_fallback_prompt_toolkit_to_basic"))

        with patch("openjax_tui.app._input_loop_basic", new=AsyncMock()) as mocked_basic:
            await fallback_to_basic(client=client, state=state, reason="prompt_toolkit_exited_early")

        self.assertEqual(state.input_backend, "basic")
        self.assertEqual(state.input_backend_reason, "prompt_toolkit_exited_early")
        self.assertIsNone(state.stream_turn_id)
        self.assertIsNone(state.stream_block_index)
        self.assertIsNone(state.live_viewport_owner_turn_id)
        self.assertEqual(state.live_viewport_turn_ownership, {})
        self.assertIsNone(state.history_setter)
        self.assertIsNone(state.prompt_invalidator)
        mocked_basic.assert_awaited_once()


if __name__ == "__main__":
    unittest.main()
