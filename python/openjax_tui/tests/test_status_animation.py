from __future__ import annotations

import asyncio
import unittest
from collections.abc import Awaitable, Callable
from typing import cast
from unittest.mock import AsyncMock, patch

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.models import EventEnvelope
from openjax_tui import app
from openjax_tui.state import AnimationLifecycle, AppState


def _evt(turn_id: str, event_type: str, payload: dict[str, object] | None = None) -> EventEnvelope:
    return EventEnvelope(
        protocol_version="v1",
        kind="event",
        session_id="s1",
        turn_id=turn_id,
        event_type=event_type,
        payload=payload or {},
    )


class StatusAnimationTest(unittest.IsolatedAsyncioTestCase):
    async def test_ticker_advances_with_bounded_cadence_only_while_active(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.turn_phase = "thinking"
        redraw_calls: list[str] = []
        state.prompt_invalidator = lambda: redraw_calls.append("redraw")
        delays: list[float] = []
        run_ticker = cast(Callable[..., Awaitable[None]], getattr(app, "_run_status_animation_ticker"))
        animation_interval = cast(float, getattr(app, "_STATUS_ANIMATION_INTERVAL_S"))

        async def fake_sleep(delay: float) -> None:
            delays.append(delay)
            if len(delays) >= 3:
                state.turn_phase = "idle"
            await asyncio.sleep(0)

        await run_ticker(state, sleep_fn=fake_sleep)

        self.assertGreaterEqual(animation_interval, 1.0 / 8.0)
        self.assertTrue(delays)
        self.assertTrue(all(delay == animation_interval for delay in delays))
        self.assertEqual(redraw_calls, ["redraw", "redraw"])
        self.assertEqual(state.animation_lifecycle, AnimationLifecycle.IDLE)
        self.assertEqual(state.animation_frame_index, 0)

    async def test_single_ticker_task_source_of_truth(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.turn_phase = "thinking"
        state.prompt_invalidator = lambda: None
        start_ticker = cast(Callable[[AppState], None], getattr(app, "_start_status_animation"))
        sync_ticker = cast(Callable[[AppState], None], getattr(app, "_sync_status_animation_controller"))

        start_ticker(state)
        first_task = state.animation_task
        self.assertIsNotNone(first_task)
        start_ticker(state)
        self.assertIs(state.animation_task, first_task)

        state.turn_phase = "idle"
        sync_ticker(state)
        await asyncio.sleep(0)

        self.assertIsNone(state.animation_task)
        assert first_task is not None
        self.assertTrue(first_task.cancelled() or first_task.done())

    async def test_sync_controller_does_not_spawn_ticker_for_basic_backend(self) -> None:
        state = AppState()
        state.input_backend = "basic"
        state.turn_phase = "thinking"
        state.prompt_invalidator = lambda: None
        sync_ticker = cast(Callable[[AppState], None], getattr(app, "_sync_status_animation_controller"))

        sync_ticker(state)

        self.assertIsNone(state.animation_task)
        self.assertEqual(state.animation_lifecycle, AnimationLifecycle.IDLE)

    async def test_tool_completion_with_other_active_calls_stays_in_tool_wait(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.waiting_turn_id = "turn-1"
        state.turn_phase = "thinking"
        state.prompt_invalidator = lambda: None
        apply_updates = cast(Callable[[AppState, EventEnvelope], None], getattr(app, "_apply_event_state_updates"))

        state.active_tool_starts[("turn-1", "shell")] = [1.0]
        state.active_tool_starts[("turn-1", "grep")] = [1.1]
        apply_updates(state, _evt("turn-1", "tool_call_completed", {"tool_name": "shell"}))
        self.assertEqual(state.turn_phase, "tool_wait")

        state.active_tool_starts[("turn-1", "shell")] = []
        state.active_tool_starts[("turn-1", "grep")] = []
        apply_updates(state, _evt("turn-1", "tool_call_completed", {"tool_name": "grep"}))
        self.assertEqual(state.turn_phase, "thinking")

    async def test_tool_event_for_non_waiting_turn_does_not_flip_phase(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.waiting_turn_id = "turn-1"
        state.turn_phase = "idle"
        state.prompt_invalidator = lambda: None
        apply_updates = cast(Callable[[AppState, EventEnvelope], None], getattr(app, "_apply_event_state_updates"))

        apply_updates(state, _evt("turn-2", "tool_call_started", {"tool_name": "shell"}))

        self.assertEqual(state.turn_phase, "idle")
        self.assertIsNone(state.animation_task)

    async def test_turn_completion_cancels_animation(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.turn_phase = "thinking"
        state.waiting_turn_id = "turn-1"
        state.prompt_invalidator = lambda: None
        start_ticker = cast(Callable[[AppState], None], getattr(app, "_start_status_animation"))
        apply_updates = cast(Callable[[AppState, EventEnvelope], None], getattr(app, "_apply_event_state_updates"))

        start_ticker(state)
        task = state.animation_task
        self.assertIsNotNone(task)

        apply_updates(state, _evt("turn-1", "turn_completed"))
        await asyncio.sleep(0)

        self.assertEqual(state.turn_phase, "idle")
        self.assertIsNone(state.animation_task)
        assert task is not None
        self.assertTrue(task.cancelled() or task.done())

    async def test_fallback_to_basic_cancels_animation_ticker(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.turn_phase = "thinking"
        state.prompt_invalidator = lambda: None
        start_ticker = cast(Callable[[AppState], None], getattr(app, "_start_status_animation"))
        fallback_to_basic = cast(Callable[..., Awaitable[None]], getattr(app, "_fallback_prompt_toolkit_to_basic"))

        start_ticker(state)
        task = state.animation_task
        self.assertIsNotNone(task)
        client = OpenJaxAsyncClient(daemon_cmd=["true"])

        with patch("openjax_tui.app._input_loop_basic", new=AsyncMock()) as mocked_basic:
            await fallback_to_basic(
                client=client,
                state=state,
                reason="prompt_toolkit_exited_early",
            )

        await asyncio.sleep(0)
        self.assertEqual(state.input_backend, "basic")
        self.assertEqual(state.input_backend_reason, "prompt_toolkit_exited_early")
        self.assertIsNone(state.animation_task)
        assert task is not None
        self.assertTrue(task.cancelled() or task.done())
        mocked_basic.assert_awaited_once()


if __name__ == "__main__":
    _ = unittest.main()
