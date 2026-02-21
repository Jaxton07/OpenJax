import os
import unittest
from unittest.mock import MagicMock, patch

from openjax_tui import app


class InputBackendTest(unittest.TestCase):
    def test_force_basic_by_env(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "basic"}, clear=False):
            self.assertEqual(app._select_input_backend(), "basic")

    def test_prompt_toolkit_when_tty_and_available(self) -> None:
        with (
            patch.dict(os.environ, {}, clear=False),
            patch.object(app, "PromptSession", object()),
            patch.object(app, "patch_stdout", object()),
            patch.object(app.sys, "stdin", MagicMock(isatty=lambda: True)),
            patch.object(app.sys, "stdout", MagicMock(isatty=lambda: True)),
        ):
            self.assertEqual(app._select_input_backend(), "prompt_toolkit")


if __name__ == "__main__":
    unittest.main()
