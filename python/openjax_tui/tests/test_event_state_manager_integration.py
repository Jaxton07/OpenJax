from __future__ import annotations

import asyncio
import unittest

from openjax_sdk.models import EventEnvelope
from openjax_tui.event_state_manager import EventStateCallbacks, EventStateManager
from openjax_tui.state import AppState, ViewMode


def _evt(turn_id: str, event_type: str, payload: dict[str, object] | None = None) -> EventEnvelope:
    return EventEnvelope(
        protocol_version="v1",
        kind="event",
        session_id="s1",
        turn_id=turn_id,
        event_type=event_type,
        payload=payload or {},
    )


class EventStateManagerIntegrationTest(unittest.IsolatedAsyncioTestCase):
    async def test_turn_phase_and_gate_transition(self) -> None:
        state = AppState()
        state.waiting_turn_id = "turn-1"
        state.turn_phase = "thinking"
        state.input_ready = asyncio.Event()
        state.input_ready.clear()
        redraw_calls: list[str] = []
        popped: list[str] = []

        callbacks = EventStateCallbacks(
            sync_animation=lambda: redraw_calls.append("sync"),
            request_redraw=lambda: redraw_calls.append("redraw"),
            log_approval_event=lambda **kwargs: None,
            pop_pending=lambda request_id: popped.append(request_id),
            use_inline_approval_panel=lambda _: False,
            debug_log=lambda _: None,
            is_live_viewport_mode=lambda: state.view_mode == ViewMode.LIVE_VIEWPORT,
        )
        manager = EventStateManager(state, callbacks)

        state.active_tool_starts[("turn-1", "shell")] = [1.0]
        manager.apply_event_updates(_evt("turn-1", "tool_call_started", {"tool_name": "shell"}))
        self.assertEqual(state.turn_phase, "tool_wait")

        state.active_tool_starts[("turn-1", "shell")] = []
        manager.apply_event_updates(
            _evt("turn-1", "tool_call_completed", {"tool_name": "shell", "ok": True})
        )
        self.assertEqual(state.turn_phase, "thinking")

        manager.apply_event_updates(_evt("turn-1", "turn_completed"))
        self.assertEqual(state.turn_phase, "idle")
        self.assertIsNone(state.waiting_turn_id)
        self.assertTrue(state.input_ready.is_set())
        self.assertIn("redraw", redraw_calls)
        self.assertEqual(popped, [])

    async def test_approval_requested_then_resolved_populates_and_cleans_queue(self) -> None:
        state = AppState()
        state.waiting_turn_id = "turn-1"
        state.input_ready = asyncio.Event()
        state.input_ready.clear()
        state.approval_interrupt = asyncio.Event()
        state.approval_interrupt.clear()
        pop_requests: list[str] = []

        def _pop_pending(request_id: str) -> None:
            pop_requests.append(request_id)
            state.pending_approvals.pop(request_id, None)
            if request_id in state.approval_order:
                state.approval_order.remove(request_id)

        callbacks = EventStateCallbacks(
            sync_animation=lambda: None,
            request_redraw=lambda: None,
            log_approval_event=lambda **kwargs: None,
            pop_pending=_pop_pending,
            use_inline_approval_panel=lambda _: True,
            debug_log=lambda _: None,
            is_live_viewport_mode=lambda: False,
        )
        manager = EventStateManager(state, callbacks)

        manager.apply_event_updates(
            _evt(
                "turn-1",
                "approval_requested",
                {"request_id": "req-1", "target": "apply_patch", "reason": "policy"},
            )
        )
        self.assertIn("req-1", state.pending_approvals)
        self.assertEqual(state.turn_phase, "approval")
        self.assertTrue(state.input_ready.is_set())
        self.assertTrue(state.approval_interrupt.is_set())

        manager.apply_event_updates(
            _evt("turn-1", "approval_resolved", {"request_id": "req-1", "approved": True})
        )
        self.assertIn("req-1", pop_requests)


if __name__ == "__main__":
    unittest.main()
