from __future__ import annotations

import io
import unittest
from unittest.mock import patch

from openjax_tui import app


class _FakeBuffer:
    def __init__(self) -> None:
        self.data = bytearray()
        self.flushed = 0

    def write(self, payload: bytes) -> int:
        self.data.extend(payload)
        return len(payload)

    def flush(self) -> None:
        self.flushed += 1


class _FakeStream:
    def __init__(self, *, is_tty: bool, with_buffer: bool = True) -> None:
        self._is_tty = is_tty
        self.buffer = _FakeBuffer() if with_buffer else None
        self.text = io.StringIO()
        self.flushed = 0

    def isatty(self) -> bool:
        return self._is_tty

    def write(self, text: str) -> int:
        return self.text.write(text)

    def flush(self) -> None:
        self.flushed += 1


class KeyboardEnhancementTest(unittest.TestCase):
    def test_env_toggle_defaults_to_disabled(self) -> None:
        with patch.dict("os.environ", {}, clear=False):
            self.assertFalse(app._keyboard_enhancement_enabled_by_env())

    def test_env_toggle_accepts_truthy_values(self) -> None:
        with patch.dict(
            "os.environ",
            {"OPENJAX_TUI_ENABLE_KEYBOARD_ENHANCEMENT": "true"},
            clear=False,
        ):
            self.assertTrue(app._keyboard_enhancement_enabled_by_env())

    def test_enable_writes_push_sequence_for_tty_stream(self) -> None:
        stream = _FakeStream(is_tty=True, with_buffer=True)
        ok = app._enable_keyboard_enhancement(stream=stream)

        self.assertTrue(ok)
        assert stream.buffer is not None
        self.assertEqual(stream.buffer.data.decode("utf-8"), "\x1b[>7u")
        self.assertEqual(stream.buffer.flushed, 1)

    def test_disable_writes_pop_sequence_for_tty_stream(self) -> None:
        stream = _FakeStream(is_tty=True, with_buffer=True)
        ok = app._disable_keyboard_enhancement(stream=stream)

        self.assertTrue(ok)
        assert stream.buffer is not None
        self.assertEqual(stream.buffer.data.decode("utf-8"), "\x1b[<1u")
        self.assertEqual(stream.buffer.flushed, 1)

    def test_enable_skips_non_tty_stream(self) -> None:
        stream = _FakeStream(is_tty=False, with_buffer=True)
        ok = app._enable_keyboard_enhancement(stream=stream)

        self.assertFalse(ok)
        assert stream.buffer is not None
        self.assertEqual(stream.buffer.data.decode("utf-8"), "")
        self.assertEqual(stream.buffer.flushed, 0)


if __name__ == "__main__":
    unittest.main()
