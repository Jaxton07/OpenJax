"""Tests for markdown message renderable."""

from __future__ import annotations

import unittest

from openjax_tui.widgets.markdown_message import MarkdownMessage
from rich.markdown import Markdown


class TestMarkdownMessage(unittest.TestCase):
    def test_markdown_renderable_created(self) -> None:
        msg = MarkdownMessage("# Title\n\n- item\n\n> quote")

        renderable = msg.to_renderable()

        self.assertIsInstance(renderable, Markdown)
        self.assertIn("Title", renderable.markup)
        self.assertIn("- item", renderable.markup)
        self.assertIn("> quote", renderable.markup)

    def test_fenced_code_block_uses_theme(self) -> None:
        msg = MarkdownMessage("```python\nprint('hi')\n```", code_theme="monokai")

        renderable = msg.to_renderable()

        self.assertEqual(renderable.code_theme, "monokai")
        self.assertIn("print('hi')", renderable.markup)

    def test_invalid_markdown_does_not_raise(self) -> None:
        msg = MarkdownMessage("```python\nunterminated")

        renderable = msg.to_renderable()

        self.assertIsInstance(renderable, Markdown)


if __name__ == "__main__":
    unittest.main()

