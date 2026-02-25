"""Main application for OpenJax TUI."""

from __future__ import annotations

import sys

from textual.app import App

from .screens.chat import ChatScreen


class OpenJaxApp(App):
    """OpenJax TUI main application."""

    TITLE = "OpenJax"
    SUB_TITLE = "AI Agent Framework"
    CSS_PATH = "styles.tcss"

    def on_mount(self) -> None:
        """Push the chat screen when app mounts."""
        self.push_screen(ChatScreen())

    def action_help_quit(self) -> None:
        """Override to quit immediately without confirmation."""
        self.exit()
