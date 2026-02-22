from __future__ import annotations

import os
import unittest
from unittest.mock import patch

from openjax_tui import app


class StartupConfigTest(unittest.TestCase):
    def test_resolve_openjax_version_from_env(self) -> None:
        with patch.dict(os.environ, {"OPENJAX_VERSION": "9.9.9-test"}, clear=False):
            self.assertEqual(app._resolve_openjax_version(), "9.9.9-test")

    def test_slash_command_candidates(self) -> None:
        self.assertEqual(
            app._slash_command_candidates("/"),
            ["/approve", "/pending", "/help", "/exit"],
        )
        self.assertEqual(app._slash_command_candidates("/he"), ["/help"])
        self.assertEqual(app._slash_command_candidates("hello"), [])

    def test_slash_hint_text(self) -> None:
        self.assertIn("/approve", app._slash_hint_text("/"))
        self.assertEqual(app._slash_hint_text("hello"), "")

    def test_format_display_directory_uses_tilde(self) -> None:
        home = os.path.expanduser("~")
        self.assertEqual(app._format_display_directory(home), "~")
        self.assertEqual(
            app._format_display_directory(os.path.join(home, "work", "repo")),
            "~/work/repo",
        )


if __name__ == "__main__":
    unittest.main()
