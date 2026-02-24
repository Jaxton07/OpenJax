from __future__ import annotations

import os
import tempfile
import unittest

from openjax_tui.tui_logging import (
    _reset_tui_logger_for_tests,
    _setup_tui_logger,
    _tui_debug,
)
from openjax_tui import tui_logging
from openjax_tui import session_logging


class LoggingConfigTest(unittest.TestCase):
    _old_log_dir: str | None = None
    _old_max_bytes: str | None = None
    _old_debug: str | None = None

    def setUp(self) -> None:
        _reset_tui_logger_for_tests()
        self._old_log_dir = os.environ.get("OPENJAX_TUI_LOG_DIR")
        self._old_max_bytes = os.environ.get("OPENJAX_TUI_LOG_MAX_BYTES")
        self._old_debug = os.environ.get("OPENJAX_TUI_DEBUG")

    def tearDown(self) -> None:
        _reset_tui_logger_for_tests()
        if self._old_log_dir is None:
            os.environ.pop("OPENJAX_TUI_LOG_DIR", None)
        else:
            os.environ["OPENJAX_TUI_LOG_DIR"] = self._old_log_dir
        if self._old_max_bytes is None:
            os.environ.pop("OPENJAX_TUI_LOG_MAX_BYTES", None)
        else:
            os.environ["OPENJAX_TUI_LOG_MAX_BYTES"] = self._old_max_bytes
        if self._old_debug is None:
            os.environ.pop("OPENJAX_TUI_DEBUG", None)
        else:
            os.environ["OPENJAX_TUI_DEBUG"] = self._old_debug

    def test_setup_tui_logger_writes_to_expected_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            os.environ["OPENJAX_TUI_LOG_DIR"] = tmpdir
            os.environ["OPENJAX_TUI_LOG_MAX_BYTES"] = "1024"
            os.environ["OPENJAX_TUI_DEBUG"] = "1"

            logger = _setup_tui_logger()
            self.assertIsNotNone(logger)

            _tui_debug("log-probe-debug-message")
            assert logger is not None
            for handler in logger.handlers:
                handler.flush()

            log_path = os.path.join(tmpdir, "openjax_tui.log")
            self.assertTrue(os.path.exists(log_path))
            with open(log_path, "r", encoding="utf-8") as fh:
                content = fh.read()

            self.assertIn("log-probe-debug-message", content)
            self.assertIn("tui logger initialized", content)

    def test_setup_tui_logger_respects_rotation_config(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            os.environ["OPENJAX_TUI_LOG_DIR"] = tmpdir
            os.environ["OPENJAX_TUI_LOG_MAX_BYTES"] = "2048"

            logger = _setup_tui_logger()
            assert logger is not None
            rotating_handlers = [h for h in logger.handlers if hasattr(h, "backupCount")]
            self.assertEqual(len(rotating_handlers), 1)

            handler = rotating_handlers[0]
            self.assertEqual(getattr(handler, "maxBytes"), 2048)
            self.assertEqual(getattr(handler, "backupCount"), 5)

    def test_parse_log_max_bytes_in_module(self) -> None:
        self.assertEqual(tui_logging._parse_log_max_bytes("4096", 100), 4096)
        self.assertEqual(tui_logging._parse_log_max_bytes("invalid", 100), 100)

    def test_session_logging_field_helpers(self) -> None:
        self.assertEqual(session_logging.approval_text_field(" a b "), "a_b")
        self.assertEqual(session_logging.approval_bool_field(None), "-")


if __name__ == "__main__":
    unittest.main()
