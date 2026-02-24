from __future__ import annotations

import asyncio
import io
import unittest
from contextlib import redirect_stdout
from typing import Optional

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxResponseError
from openjax_sdk.models import EventEnvelope
from openjax_tui.app import (
    AppState,
    ApprovalRecord,
    _apply_event_state_updates,
    _approval_toolbar_text,
    _drain_background_task,
    _handle_user_line,
    _shutdown_client_quietly,
)
from openjax_tui.approval import (
    input_prompt_prefix as _input_prompt_prefix,
    move_approval_focus as _move_approval_focus,
    resolve_approval_by_id,
    use_inline_approval_panel,
    pop_pending,
    is_expired_approval_error,
)
from openjax_tui.session_logging import tui_log_approval_event
from openjax_tui.tui_logging import _tui_log_info


class _StubClient(OpenJaxAsyncClient):
    def __init__(self) -> None:
        super().__init__(daemon_cmd=["true"])
        self.submitted: list[str] = []
        self.resolved: list[tuple[str, str, bool]] = []
        self.resolve_error: OpenJaxResponseError | None = None

    async def submit_turn(
        self, input_text: str, metadata: Optional[dict[str, object]] = None
    ) -> str:
        _ = metadata
        self.submitted.append(input_text)
        return "1"

    async def resolve_approval(
        self, turn_id: str, request_id: str, approved: bool, reason: str | None = None
    ) -> bool:
        _ = reason
        if self.resolve_error is not None:
            raise self.resolve_error
        self.resolved.append((turn_id, request_id, approved))
        return True


class _StubShutdownClient(OpenJaxAsyncClient):
    def __init__(self) -> None:
        super().__init__(daemon_cmd=["true"])
        self._session_id = "sess_1"
        self.shutdown_calls = 0
        self.stop_calls = 0

    async def shutdown_session(self) -> bool:
        self.shutdown_calls += 1
        self._session_id = None
        return True

    async def stop(self) -> None:
        self.stop_calls += 1


class _SlowStopShutdownClient(_StubShutdownClient):
    def __init__(self) -> None:
        super().__init__()
        self.stop_completed = False

    async def stop(self) -> None:
        self.stop_calls += 1
        await asyncio.sleep(1.1)
        self.stop_completed = True


class ApprovalFlowTest(unittest.IsolatedAsyncioTestCase):
    async def test_quiet_shutdown_skips_session_close_when_interrupted(self) -> None:
        client = _StubShutdownClient()

        await _shutdown_client_quietly(client, graceful=False)

        self.assertEqual(client.shutdown_calls, 0)
        self.assertEqual(client.stop_calls, 1)

    async def test_quiet_shutdown_closes_session_when_graceful(self) -> None:
        client = _StubShutdownClient()

        await _shutdown_client_quietly(client, graceful=True)

        self.assertEqual(client.shutdown_calls, 1)
        self.assertEqual(client.stop_calls, 1)

    async def test_quiet_shutdown_waits_for_slow_stop_completion(self) -> None:
        client = _SlowStopShutdownClient()

        await _shutdown_client_quietly(client, graceful=False)

        self.assertEqual(client.stop_calls, 1)
        self.assertTrue(client.stop_completed)

    async def test_drain_background_task_handles_cancelled_task(self) -> None:
        task: asyncio.Task[None] = asyncio.create_task(asyncio.sleep(10))
        await _drain_background_task(task)
        self.assertTrue(task.cancelled())

    async def test_drain_background_task_consumes_task_exception(self) -> None:
        async def boom() -> None:
            raise KeyboardInterrupt()

        task: asyncio.Task[None] = asyncio.create_task(boom())
        await _drain_background_task(task)
        self.assertTrue(task.done())

    def test_approval_requested_updates_state_and_triggers_prompt_redraw(self) -> None:
        state = AppState()
        state.input_ready = asyncio.Event()
        state.input_ready.clear()
        state.approval_interrupt = asyncio.Event()
        state.approval_interrupt.clear()
        redraw_calls: list[str] = []
        state.prompt_invalidator = lambda: redraw_calls.append("called")
        evt = EventEnvelope(
            protocol_version="1",
            kind="event",
            session_id="s1",
            turn_id="turn-evt",
            event_type="approval_requested",
            payload={
                "request_id": "ap-evt-1",
                "target": "apply_patch",
                "reason": "tool call requires approval by policy",
            },
        )

        out = io.StringIO()
        with redirect_stdout(out):
            _apply_event_state_updates(state, evt)

        self.assertIn("ap-evt-1", state.pending_approvals)
        self.assertEqual(state.approval_focus_id, "ap-evt-1")
        self.assertEqual(state.turn_phase, "approval")
        self.assertTrue(state.input_ready.is_set())
        self.assertTrue(state.approval_interrupt.is_set())
        self.assertEqual(redraw_calls, ["called"])
        self.assertIn("/approve ap-evt-1 y|n", out.getvalue())

    def test_approval_requested_hides_legacy_hint_in_prompt_toolkit(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.approval_ui_enabled = True
        state.input_ready = asyncio.Event()
        state.input_ready.clear()
        state.approval_interrupt = asyncio.Event()
        state.approval_interrupt.clear()
        evt = EventEnvelope(
            protocol_version="1",
            kind="event",
            session_id="s1",
            turn_id="turn-evt",
            event_type="approval_requested",
            payload={
                "request_id": "ap-evt-ptk",
                "target": "apply_patch",
                "reason": "tool call requires approval by policy",
            },
        )

        out = io.StringIO()
        with redirect_stdout(out):
            _apply_event_state_updates(state, evt)

        self.assertEqual(out.getvalue(), "")

    def test_approval_resolved_triggers_prompt_redraw(self) -> None:
        state = AppState()
        state.waiting_turn_id = "turn-keep-thinking"
        state.turn_phase = "approval"
        state.pending_approvals["ap-evt-2"] = ApprovalRecord(
            turn_id="turn-keep-thinking",
            target="apply_patch",
            reason="tool call requires approval by policy",
        )
        state.approval_order.append("ap-evt-2")
        redraw_calls: list[str] = []
        state.prompt_invalidator = lambda: redraw_calls.append("called")
        evt = EventEnvelope(
            protocol_version="1",
            kind="event",
            session_id="s1",
            turn_id="turn-keep-thinking",
            event_type="approval_resolved",
            payload={"request_id": "ap-evt-2", "approved": True},
        )

        _apply_event_state_updates(state, evt)

        self.assertNotIn("ap-evt-2", state.pending_approvals)
        self.assertEqual(state.turn_phase, "thinking")
        self.assertEqual(redraw_calls, ["called"])

    async def test_y_without_approval_submits_normal_turn(self) -> None:
        client = _StubClient()
        state = AppState()

        keep_running = await _handle_user_line(client, state, "y")

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, ["y"])
        self.assertEqual(client.resolved, [])

    async def test_y_with_active_approval_resolves_focused_request(self) -> None:
        client = _StubClient()
        state = AppState()
        state.pending_approvals["ap-1"] = ApprovalRecord(
            turn_id="turn-1",
            target="apply_patch",
            reason="tool call requires approval by policy",
        )
        state.approval_order.append("ap-1")
        state.approval_focus_id = "ap-1"
        state.turn_phase = "approval"

        keep_running = await _handle_user_line(client, state, "y")

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, [])
        self.assertEqual(client.resolved, [("turn-1", "ap-1", True)])
        self.assertNotIn("ap-1", state.pending_approvals)

    async def test_text_input_is_blocked_while_approval_pending(self) -> None:
        client = _StubClient()
        state = AppState()
        state.pending_approvals["ap-block"] = ApprovalRecord(
            turn_id="turn-block",
            target="apply_patch",
            reason="tool call requires approval by policy",
        )
        state.approval_order.append("ap-block")
        state.approval_focus_id = "ap-block"
        state.turn_phase = "approval"
        out = io.StringIO()

        with redirect_stdout(out):
            keep_running = await _handle_user_line(client, state, "请继续")

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, [])
        self.assertIn("pending request", out.getvalue())

    async def test_expired_approval_error_cleans_pending_request(self) -> None:
        client = _StubClient()
        client.resolve_error = OpenJaxResponseError(
            code="APPROVAL_NOT_FOUND",
            message="approval request not found or already resolved",
            retriable=False,
            details={},
        )
        state = AppState()
        state.pending_approvals["ap-2"] = ApprovalRecord(
            turn_id="turn-2",
            target="apply_patch",
            reason="tool call requires approval by policy",
        )
        state.approval_order.append("ap-2")
        state.approval_focus_id = "ap-2"
        state.turn_phase = "approval"
        out = io.StringIO()

        with redirect_stdout(out):
            await resolve_approval_by_id(
                client, state, "ap-2", approved=True,
                use_inline_approval_panel_fn=use_inline_approval_panel,
                pop_pending_fn=pop_pending,
                is_expired_approval_error_fn=is_expired_approval_error,
                log_approval_event_fn=lambda **kwargs: tui_log_approval_event(_tui_log_info, **kwargs),
            )

        self.assertNotIn("ap-2", state.pending_approvals)
        self.assertEqual(state.approval_focus_id, None)
        self.assertIn("auto-denied (expired): ap-2", out.getvalue())

    async def test_empty_input_confirms_selected_action_in_approval_ui(self) -> None:
        client = _StubClient()
        state = AppState()
        state.pending_approvals["ap-3"] = ApprovalRecord(
            turn_id="turn-3",
            target="apply_patch",
            reason="tool call requires approval by policy",
        )
        state.approval_order.append("ap-3")
        state.approval_focus_id = "ap-3"
        state.turn_phase = "approval"
        state.approval_ui_enabled = True
        state.approval_selected_action = "deny"

        keep_running = await _handle_user_line(client, state, "")

        self.assertTrue(keep_running)
        self.assertEqual(client.resolved, [("turn-3", "ap-3", False)])

    def test_toolbar_and_focus_switching(self) -> None:
        state = AppState()
        state.approval_ui_enabled = True
        state.turn_phase = "approval"
        state.pending_approvals["ap-4"] = ApprovalRecord(
            turn_id="turn-4",
            target="apply_patch",
            reason="r1",
        )
        state.pending_approvals["ap-5"] = ApprovalRecord(
            turn_id="turn-5",
            target="shell",
            reason="r2",
        )
        state.approval_order.append("ap-4")
        state.approval_order.append("ap-5")
        state.approval_focus_id = "ap-5"

        text = _approval_toolbar_text(state, "─" * 40)
        self.assertIn("Permission Request", text)
        self.assertIn("Action: shell", text)
        self.assertIn("Reason: r2", text)
        self.assertIn("❯ 1. Yes", text)

        _move_approval_focus(state, step=-1)
        self.assertEqual(state.approval_focus_id, "ap-4")

    def test_prompt_prefix_stays_normal_for_prompt_toolkit(self) -> None:
        state = AppState()
        self.assertEqual(_input_prompt_prefix(state, "❯"), "❯")

        state.pending_approvals["ap-6"] = ApprovalRecord(
            turn_id="turn-6",
            target="apply_patch",
            reason="r6",
        )
        state.approval_order.append("ap-6")
        state.approval_focus_id = "ap-6"
        state.turn_phase = "approval"
        state.input_backend = "prompt_toolkit"
        state.approval_ui_enabled = True
        self.assertEqual(_input_prompt_prefix(state, "❯"), "❯")

    def test_prompt_prefix_switches_in_approval_mode_for_basic(self) -> None:
        state = AppState()
        state.pending_approvals["ap-6"] = ApprovalRecord(
            turn_id="turn-6",
            target="apply_patch",
            reason="r6",
        )
        state.approval_order.append("ap-6")
        state.approval_focus_id = "ap-6"
        state.turn_phase = "approval"
        self.assertEqual(_input_prompt_prefix(state, "❯"), "approval>")


if __name__ == "__main__":
    unittest.main()
