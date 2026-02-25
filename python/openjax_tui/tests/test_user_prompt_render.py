import io
import unittest
from contextlib import redirect_stdout
from typing import Optional

from openjax_sdk import OpenJaxAsyncClient
from openjax_tui.app import AppState
from openjax_tui.input_loops import InputLoopCallbacks, handle_user_line as _handle_user_line


class _StubClient(OpenJaxAsyncClient):
    def __init__(self) -> None:
        super().__init__(daemon_cmd=["true"])
        self.submitted: list[str] = []

    async def submit_turn(
        self, input_text: str, metadata: Optional[dict[str, object]] = None
    ) -> str:
        _ = metadata
        self.submitted.append(input_text)
        return "1"


def _create_test_callbacks() -> InputLoopCallbacks:
    """Create test callbacks for input loops."""
    return InputLoopCallbacks(
        approval_mode_active=lambda _: False,
        focused_approval_id=lambda _: None,
        resolve_approval_by_id=lambda *args, **kwargs: None,
        resolve_latest_approval=lambda *args, **kwargs: None,
        use_inline_approval_panel=lambda _: False,
        emit_ui_line=lambda state, text: setattr(state, "history_blocks", state.history_blocks + [text]),
        print_pending=lambda _: None,
        tui_log_approval_event=lambda **kwargs: None,
        sync_status_animation_controller=lambda _: None,
    )


class UserPromptRenderTest(unittest.IsolatedAsyncioTestCase):
    async def test_submit_turn_renders_user_prefix(self) -> None:
        state = AppState()
        client = _StubClient()
        callbacks = _create_test_callbacks()
        out = io.StringIO()

        with redirect_stdout(out):
            keep_running = await _handle_user_line(client, state, "你好", callbacks)

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, ["你好"])
        self.assertEqual(out.getvalue(), "")

    async def test_submit_turn_appends_user_line_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.history_setter = lambda _: None
        client = _StubClient()
        callbacks = _create_test_callbacks()

        keep_running = await _handle_user_line(client, state, "hello", callbacks)

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, ["hello"])
        self.assertEqual(
            state.history_blocks,
            [
                "╭─────╮\n│hello│\n╰─────╯",
            ],
        )

    async def test_submit_turn_appends_cjk_line_with_aligned_border(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.history_setter = lambda _: None
        client = _StubClient()
        callbacks = _create_test_callbacks()

        keep_running = await _handle_user_line(client, state, "你好", callbacks)

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, ["你好"])
        self.assertEqual(
            state.history_blocks,
            [
                "╭────╮\n│你好│\n╰────╯",
            ],
        )


if __name__ == "__main__":
    unittest.main()
