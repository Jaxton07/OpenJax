"""Markdown renderable wrapper for assistant messages."""

from __future__ import annotations

from dataclasses import dataclass

from rich.markdown import Markdown


@dataclass
class MarkdownMessage:
    """Assistant message markdown renderable."""

    content: str
    code_theme: str = "monokai"

    def to_renderable(self) -> Markdown:
        return Markdown(self.content, code_theme=self.code_theme)

