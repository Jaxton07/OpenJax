"""Tests for logging setup."""

from __future__ import annotations

import logging
import os
from pathlib import Path
import tempfile
import unittest

from openjax_tui.logging_setup import _reset_logging_for_tests, setup_logging


class TestLoggingSetup(unittest.TestCase):
    """Test logging setup behavior."""

    def setUp(self) -> None:
        self._old_log_dir = os.environ.get("OPENJAX_TUI_LOG_DIR")
        self._old_max_bytes = os.environ.get("OPENJAX_TUI_LOG_MAX_BYTES")
        self._old_debug = os.environ.get("OPENJAX_TUI_DEBUG")
        _reset_logging_for_tests()

    def tearDown(self) -> None:
        self._restore_env("OPENJAX_TUI_LOG_DIR", self._old_log_dir)
        self._restore_env("OPENJAX_TUI_LOG_MAX_BYTES", self._old_max_bytes)
        self._restore_env("OPENJAX_TUI_DEBUG", self._old_debug)
        _reset_logging_for_tests()

    @staticmethod
    def _restore_env(name: str, value: str | None) -> None:
        if value is None:
            os.environ.pop(name, None)
        else:
            os.environ[name] = value

    def test_setup_logging_writes_file(self) -> None:
        """Logger should create and write to the configured file."""
        with tempfile.TemporaryDirectory() as tmpdir:
            os.environ["OPENJAX_TUI_LOG_DIR"] = tmpdir
            os.environ["OPENJAX_TUI_LOG_MAX_BYTES"] = "1024"
            os.environ["OPENJAX_TUI_DEBUG"] = "1"

            logger = setup_logging()
            logger.info("hello from test")
            for handler in logger.handlers:
                handler.flush()

            log_path = Path(tmpdir) / "openjax_tui.log"
            self.assertTrue(log_path.exists())
            content = log_path.read_text(encoding="utf-8")
            self.assertIn("hello from test", content)
            self.assertEqual(logger.level, logging.DEBUG)

    def test_setup_logging_invalid_max_bytes_uses_default(self) -> None:
        """Invalid max bytes should not break logger setup."""
        with tempfile.TemporaryDirectory() as tmpdir:
            os.environ["OPENJAX_TUI_LOG_DIR"] = tmpdir
            os.environ["OPENJAX_TUI_LOG_MAX_BYTES"] = "bad-value"
            os.environ.pop("OPENJAX_TUI_DEBUG", None)

            logger = setup_logging()
            self.assertEqual(logger.level, logging.INFO)
            self.assertGreaterEqual(len(logger.handlers), 1)


if __name__ == "__main__":
    unittest.main()
