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
        return Markdown(_with_hard_linebreaks(self.content), code_theme=self.code_theme)


def _with_hard_linebreaks(content: str) -> str:
    """Preserve single line breaks for terminal markdown rendering."""
    lines = content.split("\n")
    in_fenced_code = False
    output: list[str] = []

    for line in lines:
        stripped = line.lstrip()
        if stripped.startswith("```"):
            in_fenced_code = not in_fenced_code
            output.append(line)
            continue

        if in_fenced_code or not line or line.endswith("  "):
            output.append(line)
            continue

        output.append(f"{line}  ")

    return "\n".join(output)
