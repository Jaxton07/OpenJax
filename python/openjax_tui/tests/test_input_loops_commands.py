from __future__ import annotations

import io
import unittest
from contextlib import redirect_stdout
from typing import Optional
from unittest.mock import AsyncMock, MagicMock

from openjax_sdk import OpenJaxAsyncClient
from openjax_tui.input_loops import InputLoopCallbacks, handle_user_line
from openjax_tui.state import AppState, ApprovalRecord


class _StubClient(OpenJaxAsyncClient):
    def __init__(self) -> None:
        super().__init__(daemon_cmd=["true"])
        self.submitted: list[str] = []

    async def submit_turn(
        self, input_text: str, metadata: Optional[dict[str, object]] = None
    ) -> str:
        _ = metadata
        self.submitted.append(input_text)
        return "turn-1"


def _callbacks() -> InputLoopCallbacks:
    return InputLoopCallbacks(
        approval_mode_active=lambda state: bool(state.pending_approvals),
        focused_approval_id=lambda state: state.approval_focus_id,
        resolve_approval_by_id=AsyncMock(),
        resolve_latest_approval=AsyncMock(),
        use_inline_approval_panel=lambda _: False,
        emit_ui_line=lambda state, text: state.history_blocks.append(text),
        print_pending=MagicMock(),
        tui_log_approval_event=lambda **kwargs: None,
        sync_status_animation_controller=lambda _: None,
    )


class InputLoopsCommandsTest(unittest.IsolatedAsyncioTestCase):
    async def test_help_prints_command_rows(self) -> None:
        state = AppState()
        client = _StubClient()
        callbacks = _callbacks()
        out = io.StringIO()

        with redirect_stdout(out):
            keep_running = await handle_user_line(
                client,
                state,
                "/help",
                callbacks,
                command_rows=("cmd1", "cmd2"),
            )

        self.assertTrue(keep_running)
        self.assertIn("commands:", out.getvalue())
        self.assertIn("cmd1", out.getvalue())

    async def test_pending_delegates_to_callback(self) -> None:
        state = AppState()
        client = _StubClient()
        callbacks = _callbacks()
        callbacks.print_pending = MagicMock()

        keep_running = await handle_user_line(client, state, "/pending", callbacks)

        self.assertTrue(keep_running)
        callbacks.print_pending.assert_called_once_with(state)

    async def test_approve_invalid_args_prints_usage(self) -> None:
        state = AppState()
        client = _StubClient()
        callbacks = _callbacks()
        out = io.StringIO()

        with redirect_stdout(out):
            keep_running = await handle_user_line(
                client,
                state,
                "/approve req-1 maybe",
                callbacks,
            )

        self.assertTrue(keep_running)
        self.assertIn("usage: /approve <approval_request_id> <y|n>", out.getvalue())

    async def test_quick_yes_in_approval_mode_resolves_latest(self) -> None:
        state = AppState()
        state.pending_approvals["req-1"] = ApprovalRecord(
            turn_id="turn-1", target="apply_patch", reason="test"
        )
        state.approval_focus_id = "req-1"
        client = _StubClient()
        callbacks = _callbacks()
        callbacks.resolve_latest_approval = AsyncMock()

        keep_running = await handle_user_line(client, state, "y", callbacks)

        self.assertTrue(keep_running)
        callbacks.resolve_latest_approval.assert_awaited_once()
        self.assertEqual(client.submitted, [])

    async def test_text_input_blocked_while_approval_pending(self) -> None:
        state = AppState()
        state.pending_approvals["req-2"] = ApprovalRecord(
            turn_id="turn-1", target="apply_patch", reason="test"
        )
        state.approval_focus_id = "req-2"
        client = _StubClient()
        callbacks = _callbacks()
        out = io.StringIO()

        with redirect_stdout(out):
            keep_running = await handle_user_line(client, state, "continue", callbacks)

        self.assertTrue(keep_running)
        self.assertIn("pending request", out.getvalue())
        self.assertEqual(client.submitted, [])


if __name__ == "__main__":
    unittest.main()
