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

    def test_force_prompt_toolkit_by_env(self) -> None:
        with (
            patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "prompt_toolkit"}, clear=False),
            patch.object(app, "PromptSession", object()),
            patch.object(app, "patch_stdout", object()),
            patch.object(app.sys, "stdin", MagicMock(isatty=lambda: False)),
            patch.object(app.sys, "stdout", MagicMock(isatty=lambda: False)),
        ):
            self.assertEqual(app._select_input_backend(), "prompt_toolkit")

    def test_force_prompt_toolkit_when_keybindings_unavailable(self) -> None:
        with (
            patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "prompt_toolkit"}, clear=False),
            patch.object(app, "PromptSession", object()),
            patch.object(app, "patch_stdout", object()),
            patch.object(app, "KeyBindings", None),
        ):
            self.assertEqual(app._select_input_backend(), "prompt_toolkit")

    def test_backend_reason_when_forced_basic(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "basic"}, clear=False):
            backend, reason = app._select_input_backend_with_reason()
            self.assertEqual(backend, "basic")
            self.assertIn("forced by OPENJAX_TUI_INPUT_BACKEND=basic", reason)


if __name__ == "__main__":
    unittest.main()
