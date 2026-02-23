from __future__ import annotations

import asyncio
import unittest

from openjax_tui import prompt_runtime
from openjax_tui.state import AppState


class PromptRuntimeModuleTest(unittest.TestCase):
    def test_history_text_empty(self) -> None:
        state = AppState()
        self.assertEqual(prompt_runtime.history_text(state), "\n")

    def test_refresh_history_view_no_setter(self) -> None:
        state = AppState()
        prompt_runtime.refresh_history_view(state)

    def test_drain_background_task_none(self) -> None:
        asyncio.run(prompt_runtime.drain_background_task(None))


if __name__ == "__main__":
    unittest.main()
