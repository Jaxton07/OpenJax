import unittest

from openjax_tui.app import (
    _OPENJAX_LOGO_LONG,
    _OPENJAX_LOGO_SHORT,
    _OPENJAX_LOGO_TINY,
    _normalize_logo_block,
    _select_logo,
    _text_block_width,
)


class LogoSelectTest(unittest.TestCase):
    def test_select_long_when_terminal_is_wide(self) -> None:
        width = _text_block_width(_normalize_logo_block(_OPENJAX_LOGO_LONG))
        self.assertEqual(_select_logo(width), _OPENJAX_LOGO_LONG)

    def test_select_short_when_terminal_fits_short_only(self) -> None:
        width = _text_block_width(_normalize_logo_block(_OPENJAX_LOGO_SHORT))
        self.assertEqual(_select_logo(width), _OPENJAX_LOGO_SHORT)

    def test_select_tiny_when_terminal_is_narrow(self) -> None:
        self.assertEqual(_select_logo(10), _OPENJAX_LOGO_TINY)


if __name__ == "__main__":
    unittest.main()
