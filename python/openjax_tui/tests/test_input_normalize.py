import unittest

from openjax_tui.app import _normalize_input
from openjax_tui.input_backend import normalize_input


class NormalizeInputTest(unittest.TestCase):
    def test_strip_arrow_escape_sequences(self) -> None:
        raw = "abc\x1b[A\x1b[B\x1b[C\x1b[Dxyz"
        self.assertEqual(_normalize_input(raw), "abcxyz")

    def test_apply_backspace_semantics(self) -> None:
        raw = "ab\x08c"
        self.assertEqual(_normalize_input(raw), "ac")

    def test_module_normalize_input(self) -> None:
        raw = "a\x1b[A\x08bc"
        self.assertEqual(normalize_input(raw), "bc")


if __name__ == "__main__":
    unittest.main()
