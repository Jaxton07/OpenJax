from __future__ import annotations

import contextlib
import os
import re
import shutil
import sys
from pathlib import Path


_LOGO_GLYPHS: dict[str, tuple[str, ...]] = {
    "O": (
        " █████ ",
        "██   ██",
        "██   ██",
        "██   ██",
        "██   ██",
        " █████ ",
    ),
    "P": (
        "██████ ",
        "██   ██",
        "██████ ",
        "██     ",
        "██     ",
        "██     ",
    ),
    "E": (
        "███████",
        "██     ",
        "█████  ",
        "██     ",
        "██     ",
        "███████",
    ),
    "N": (
        "██   ██",
        "███  ██",
        "████ ██",
        "██ ████",
        "██  ███",
        "██   ██",
    ),
    "J": (
        "   ████",
        "    ██ ",
        "    ██ ",
        "    ██ ",
        "██  ██ ",
        " ████  ",
    ),
    "A": (
        "  ███  ",
        " ██ ██ ",
        "██   ██",
        "███████",
        "██   ██",
        "██   ██",
    ),
    "X": (
        "██   ██",
        " ██ ██ ",
        "  ███  ",
        "  ███  ",
        " ██ ██ ",
        "██   ██",
    ),
}


def _compose_logo(word: str, letter_spacing: int) -> str:
    glyphs = [_LOGO_GLYPHS[ch] for ch in word if ch in _LOGO_GLYPHS]
    if not glyphs:
        return ""

    height = max((len(glyph) for glyph in glyphs), default=0)
    normalized_glyphs: list[list[str]] = []
    for glyph in glyphs:
        glyph_width = max((len(row) for row in glyph), default=0)
        rows = [row.ljust(glyph_width) for row in glyph]
        if len(rows) < height:
            rows.extend([" " * glyph_width] * (height - len(rows)))
        normalized_glyphs.append(rows)

    spacer = " " * max(letter_spacing, 1)
    lines: list[str] = []
    for row_idx in range(height):
        lines.append(spacer.join(rows[row_idx] for rows in normalized_glyphs).rstrip())
    return "\n".join(lines)


_OPENJAX_LOGO_LONG = _compose_logo("OPENJAX", letter_spacing=2)
_OPENJAX_LOGO_SHORT = _compose_logo("OPENJAX", letter_spacing=1)
_OPENJAX_LOGO_TINY = "OPENJAX"


def _resolve_openjax_version() -> str:
    env_version = os.environ.get("OPENJAX_VERSION", "").strip()
    if env_version:
        return env_version

    cargo_path = Path(__file__).resolve().parents[4] / "Cargo.toml"
    if not cargo_path.exists():
        return "dev"

    in_workspace_package = False
    with contextlib.suppress(OSError):
        with cargo_path.open("r", encoding="utf-8") as fh:
            for raw_line in fh:
                line = raw_line.strip()
                if line.startswith("[") and line.endswith("]"):
                    in_workspace_package = line == "[workspace.package]"
                    continue
                if not in_workspace_package:
                    continue
                match = re.match(r'^version\s*=\s*"([^"]+)"$', line)
                if match:
                    return match.group(1)
    return "dev"


def _format_display_directory(path: str) -> str:
    home = os.path.expanduser("~")
    if path == home:
        return "~"
    prefix = home + os.sep
    if path.startswith(prefix):
        return "~/" + path[len(prefix) :]
    return path


def _print_startup_card(version: str) -> None:
    model = os.environ.get("OPENJAX_MODEL", "(default)")
    directory = _format_display_directory(os.getcwd())
    rows = [
        f">_ OpenJax TUI (v{version})",
        "",
        f"model:     {model}",
        f"directory: {directory}",
    ]
    content_width = max((len(row) for row in rows), default=0) + 2
    top = "╭" + ("─" * content_width) + "╮"
    bottom = "╰" + ("─" * content_width) + "╯"
    print(top)
    for row in rows:
        print(f"│ {row.ljust(content_width - 1)}│")
    print(bottom)
    print()


def _text_block_width(text: str) -> int:
    return max((len(line) for line in text.splitlines()), default=0)


def _normalize_logo_block(text: str) -> str:
    lines = text.splitlines()
    while lines and not lines[0].strip():
        _ = lines.pop(0)
    while lines and not lines[-1].strip():
        _ = lines.pop()

    if not lines:
        return ""

    non_empty = [line for line in lines if line.strip()]
    common_indent = min(
        (len(line) - len(line.lstrip(" ")) for line in non_empty), default=0
    )
    normalized = [line[common_indent:].rstrip() for line in lines]
    return "\n".join(normalized)


def _select_logo(columns: int) -> str:
    long_width = _text_block_width(_normalize_logo_block(_OPENJAX_LOGO_LONG))
    short_width = _text_block_width(_normalize_logo_block(_OPENJAX_LOGO_SHORT))
    if columns >= long_width:
        return _OPENJAX_LOGO_LONG
    if columns >= short_width:
        return _OPENJAX_LOGO_SHORT
    return _OPENJAX_LOGO_TINY


def _print_logo() -> None:
    columns = shutil.get_terminal_size(fallback=(100, 24)).columns
    plain_logo = _normalize_logo_block(_select_logo(columns))
    logo = plain_logo
    if _supports_ansi_color():
        logo = _apply_horizontal_gradient(logo)
    print(logo)
    subtitle = "OPENJAX TERMINAL"
    subtitle_padding = max((_text_block_width(plain_logo) - len(subtitle)) // 2, 0)
    print(" " * subtitle_padding + subtitle)
    print()


def _supports_ansi_color() -> bool:
    if os.environ.get("NO_COLOR"):
        return False
    if not sys.stdout.isatty():
        return False
    term = os.environ.get("TERM", "")
    if term == "dumb":
        return False
    return True


def _apply_horizontal_gradient(text: str) -> str:
    lines = text.splitlines()
    if not lines:
        return text

    width = max((len(line) for line in lines), default=0)
    if width <= 1:
        return text

    start = (98, 157, 255)
    end = (255, 120, 180)

    def lerp(a: int, b: int, t: float) -> int:
        return int(round(a + (b - a) * t))

    rendered_lines: list[str] = []
    for line in lines:
        rendered_chars: list[str] = []
        for idx, ch in enumerate(line):
            if ch.isspace():
                rendered_chars.append(ch)
                continue
            t = idx / (width - 1)
            r = lerp(start[0], end[0], t)
            g = lerp(start[1], end[1], t)
            b = lerp(start[2], end[2], t)
            rendered_chars.append(f"\x1b[38;2;{r};{g};{b}m{ch}")
        rendered_chars.append("\x1b[0m")
        rendered_lines.append("".join(rendered_chars))

    return "\n".join(rendered_lines)
