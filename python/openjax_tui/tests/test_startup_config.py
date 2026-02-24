from __future__ import annotations

import os
import unittest
from unittest.mock import patch

from openjax_tui import app
from openjax_tui import slash_commands
from openjax_tui import startup_ui


class StartupConfigTest(unittest.TestCase):
    def test_resolve_openjax_version_from_env(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_VERSION": "9.9.9-test"}, clear=False):
            self.assertEqual(app._resolve_openjax_version(), "9.9.9-test")

    def test_slash_command_candidates(self) -> None:
        cmds = ("/approve", "/pending", "/help", "/exit")
        self.assertEqual(
            slash_commands.slash_command_candidates("/", cmds),
            ["/approve", "/pending", "/help", "/exit"],
        )
        self.assertEqual(slash_commands.slash_command_candidates("/he", cmds), ["/help"])
        self.assertEqual(slash_commands.slash_command_candidates("hello", cmds), [])

    def test_slash_hint_text(self) -> None:
        cmds = ("/approve", "/pending", "/help", "/exit")
        self.assertIn("/approve", slash_commands.slash_hint_text("/", cmds))
        self.assertEqual(slash_commands.slash_hint_text("hello", cmds), "")

    def test_format_display_directory_uses_tilde(self) -> None:
        home = os.path.expanduser("~")
        self.assertEqual(startup_ui._format_display_directory(home), "~")
        self.assertEqual(
            startup_ui._format_display_directory(os.path.join(home, "work", "repo")),
            "~/work/repo",
        )

    def test_startup_ui_module_exports_compatible_helpers(self) -> None:
        # 验证 startup_ui 模块正确导出常量
        self.assertIsNotNone(startup_ui._OPENJAX_LOGO_TINY)
        with patch.dict(os.environ, {"OPENJAX_VERSION": "1.2.3-mod"}, clear=False):
            self.assertEqual(startup_ui._resolve_openjax_version(), "1.2.3-mod")

    def test_slash_commands_module_helpers(self) -> None:
        cmds = ("/approve", "/pending", "/help", "/exit")
        self.assertEqual(
            slash_commands.slash_command_candidates("/he", cmds),
            ["/help"],
        )
        self.assertIn("/approve", slash_commands.slash_hint_text("/", cmds))

    def test_slash_completer_handles_empty_cursor_text(self) -> None:
        cmds = ("/approve", "/pending", "/help", "/exit")

        class DummyCompletion:
            def __init__(self, text: str, start_position: int) -> None:
                self.text = text
                self.start_position = start_position

        class DummyCompleter:
            pass

        completer = slash_commands.build_slash_command_completer(
            cmds,
            DummyCompleter,
            DummyCompletion,
        )
        self.assertIsNotNone(completer)

        empty_doc = type("Doc", (), {"text_before_cursor": ""})()
        self.assertEqual(list(completer.get_completions(empty_doc, None)), [])

        slash_doc = type("Doc", (), {"text_before_cursor": "/"})()
        completions = list(completer.get_completions(slash_doc, None))
        self.assertEqual([c.text for c in completions], list(cmds))


if __name__ == "__main__":
    unittest.main()
