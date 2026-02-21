import unittest

from openjax_tui.app import _normalize_input


class NormalizeInputTest(unittest.TestCase):
    def test_strip_arrow_escape_sequences(self) -> None:
        raw = "abc\x1b[A\x1b[B\x1b[C\x1b[Dxyz"
        self.assertEqual(_normalize_input(raw), "abcxyz")

    def test_apply_backspace_semantics(self) -> None:
        raw = "ab\x08c"
        self.assertEqual(_normalize_input(raw), "ac")


if __name__ == "__main__":
    unittest.main()
