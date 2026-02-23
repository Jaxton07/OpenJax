import os
import unittest
from unittest.mock import MagicMock, patch

from openjax_tui import input_backend


class InputBackendTest(unittest.TestCase):
    def test_force_basic_by_env(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "basic"}, clear=False):
            backend, _ = input_backend.select_input_backend_with_reason(
                prompt_session=object(),
                patch_stdout=object(),
                key_bindings=object(),
                prompt_toolkit_import_error=None,
                stdin_is_tty=True,
                stdout_is_tty=True,
            )
            self.assertEqual(backend, "basic")

    def test_prompt_toolkit_when_tty_and_available(self) -> None:
        with patch.dict(os.environ, {}, clear=False):
            backend, _ = input_backend.select_input_backend_with_reason(
                prompt_session=object(),
                patch_stdout=object(),
                key_bindings=object(),
                prompt_toolkit_import_error=None,
                stdin_is_tty=True,
                stdout_is_tty=True,
            )
            self.assertEqual(backend, "prompt_toolkit")

    def test_force_prompt_toolkit_by_env(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "prompt_toolkit"}, clear=False):
            backend, _ = input_backend.select_input_backend_with_reason(
                prompt_session=object(),
                patch_stdout=object(),
                key_bindings=object(),
                prompt_toolkit_import_error=None,
                stdin_is_tty=False,
                stdout_is_tty=False,
            )
            self.assertEqual(backend, "prompt_toolkit")

    def test_force_prompt_toolkit_when_keybindings_unavailable(self) -> None:
        with (
            patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "prompt_toolkit"}, clear=False),
        ):
            backend, _ = input_backend.select_input_backend_with_reason(
                prompt_session=object(),
                patch_stdout=object(),
                key_bindings=None,
                prompt_toolkit_import_error=None,
                stdin_is_tty=True,
                stdout_is_tty=True,
            )
            self.assertEqual(backend, "prompt_toolkit")

    def test_backend_reason_when_forced_basic(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "basic"}, clear=False):
            backend, reason = input_backend.select_input_backend_with_reason(
                prompt_session=object(),
                patch_stdout=object(),
                key_bindings=object(),
                prompt_toolkit_import_error=None,
                stdin_is_tty=True,
                stdout_is_tty=True,
            )
            self.assertEqual(backend, "basic")
            self.assertIn("forced by OPENJAX_TUI_INPUT_BACKEND=basic", reason)

    def test_module_select_input_backend_with_reason(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_TUI_INPUT_BACKEND": "basic"}, clear=False):
            backend, reason = input_backend.select_input_backend_with_reason(
                prompt_session=object(),
                patch_stdout=object(),
                key_bindings=object(),
                prompt_toolkit_import_error=None,
                stdin_is_tty=True,
                stdout_is_tty=True,
            )
            self.assertEqual(backend, "basic")
            self.assertIn("forced by OPENJAX_TUI_INPUT_BACKEND=basic", reason)


if __name__ == "__main__":
    unittest.main()
