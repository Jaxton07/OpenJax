"""Chat screen for OpenJax TUI."""

from __future__ import annotations

import sys

from textual.app import ComposeResult
from textual.containers import Vertical
from textual.screen import Screen
from textual.widgets import Footer, Header, Input, RichLog

from ..commands import create_commands
from ..widgets.command_palette import CommandPalette


def get_quit_key() -> str:
    """Get platform-specific quit key."""
    return "ctrl+c" if sys.platform == "darwin" else "ctrl+q"


class ChatScreen(Screen):
    """Main chat interface screen."""

    # Platform-specific bindings
    BINDINGS = [
        (get_quit_key(), "quit", "退出"),
    ]

    def compose(self) -> ComposeResult:
        """Compose the chat screen layout."""
        yield Header()
        with Vertical(id="chat-container"):
            yield RichLog(id="chat-log", markup=True)
            yield Input(
                placeholder="输入消息按回车发送，/ 打开命令面板...",
                id="chat-input",
            )
        yield Footer()

    def on_mount(self) -> None:
        """Called when screen is mounted."""
        log = self.query_one("#chat-log", RichLog)
        log.write("[bold green]欢迎使用 OpenJax TUI![/bold green]")
        # Show platform-specific quit key
        quit_key = "Ctrl+C" if sys.platform == "darwin" else "Ctrl+Q"
        log.write(f"输入消息按回车发送，输入 / 打开命令面板，{quit_key} 退出。\n")

    def on_input_changed(self, event: Input.Changed) -> None:
        """Handle input changes to detect '/' for command palette."""
        # Check if input is just "/" to open command palette
        if event.value == "/":
            # Clear the input and open command palette
            self.query_one("#chat-input", Input).value = ""
            self.show_command_palette()

    def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle input submission."""
        if event.value.strip():
            # Submit message to app
            self.app.submit_message(event.value.strip())
            # Clear input after submission
            self.query_one("#chat-input", Input).value = ""

    def show_command_palette(self) -> None:
        """Show the command palette overlay."""
        # Create commands with reference to app
        commands = create_commands(self.app)
        palette = CommandPalette(commands=commands, id="command-palette")
        self.mount(palette)

    def on_command_palette_dismissed(self, event: CommandPalette.Dismissed) -> None:
        """Handle command palette dismissal."""
        # Remove the palette from the screen
        palette = self.query_one("#command-palette", CommandPalette)
        if palette:
            palette.remove()
        # Focus back to input
        self.query_one("#chat-input", Input).focus()

    def add_user_message(self, text: str) -> None:
        """Add a user message to the chat log.

        Args:
            text: The message text
        """
        log = self.query_one("#chat-log", RichLog)
        log.write(f"[bold blue]你:[/bold blue] {text}")

    def add_assistant_message(self, text: str) -> None:
        """Add an assistant message to the chat log.

        Args:
            text: The message text
        """
        log = self.query_one("#chat-log", RichLog)
        log.write(f"[bold green]助手:[/bold green] {text}")

    def add_system_message(self, text: str) -> None:
        """Add a system message to the chat log.

        Args:
            text: The message text (can include Rich markup)
        """
        log = self.query_one("#chat-log", RichLog)
        log.write(text)

    def clear_messages(self) -> None:
        """Clear all messages from the chat log."""
        log = self.query_one("#chat-log", RichLog)
        log.clear()
