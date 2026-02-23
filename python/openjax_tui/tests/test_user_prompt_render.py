import io
import unittest
from contextlib import redirect_stdout
from typing import Optional

from openjax_sdk import OpenJaxAsyncClient
from openjax_tui.app import AppState, _handle_user_line


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


class UserPromptRenderTest(unittest.IsolatedAsyncioTestCase):
    async def test_submit_turn_renders_user_prefix(self) -> None:
        state = AppState()
        client = _StubClient()
        out = io.StringIO()

        with redirect_stdout(out):
            keep_running = await _handle_user_line(client, state, "你好")

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, ["你好"])
        self.assertEqual(out.getvalue(), "")

    async def test_submit_turn_appends_user_line_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.history_setter = lambda _: None
        client = _StubClient()

        keep_running = await _handle_user_line(client, state, "hello")

        self.assertTrue(keep_running)
        self.assertEqual(client.submitted, ["hello"])
        self.assertEqual(state.history_blocks, ["❯ hello"])


if __name__ == "__main__":
    unittest.main()
