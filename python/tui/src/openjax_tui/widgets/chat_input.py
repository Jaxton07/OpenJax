"""Transparent input widget for OpenJax TUI."""

from __future__ import annotations

from textual.widgets import Input


class ChatInput(Input):
    """Input widget with explicit transparent ANSI styling."""

    DEFAULT_CSS = """
    ChatInput {
        background: transparent;
        color: $foreground;
        padding: 0 1;
        border: tall $border-blurred;
        width: 100%;
        height: 3;
        background-tint: 0%;
    }

    ChatInput:focus {
        background: transparent;
        border: tall $border;
        background-tint: 0%;
    }

    ChatInput > .input--cursor {
        background: $primary;
        color: $background;
    }

    ChatInput > .input--selection {
        background: $primary 35%;
    }

    ChatInput > .input--placeholder,
    ChatInput > .input--suggestion {
        color: $text-disabled;
    }

    ChatInput:ansi {
        background: transparent;
        color: ansi_default;
        background-tint: 0%;
    }

    ChatInput:ansi > .input--cursor {
        background: ansi_cyan;
        color: ansi_black;
    }

    ChatInput:ansi > .input--placeholder,
    ChatInput:ansi > .input--suggestion {
        text-style: dim;
        color: ansi_default;
    }
    """
